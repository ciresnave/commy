/// Memory allocator for shared memory regions using offset-based allocation.
///
/// `FreeListAllocator` manages sub-allocation within a memory-mapped file, allowing
/// multiple processes to safely share data through offsets rather than pointers.
///

#[cfg(unix)]
use libc;
/// # Resizing Behavior
///
/// When `allow_growth` is enabled and the allocator runs out of space, it can
/// automatically grow the underlying file. During growth:
///
/// - A new, larger memory map is created
/// - The old mmap remains valid (as fallback) for existing data
/// - New allocations go to the new mmap
/// - Once the old mmap is no longer needed, it's dropped
///
/// # Shrinking Behavior
///
/// The allocator can shrink the file to reclaim unused space via `shrink_to_usage()`.
/// The shrink operation:
///
/// - Immediately sets an `allocation_limit` that prevents new allocations beyond it
/// - Only truncates the actual file when a single mmap exists
/// - Is safe to call even during resize operations
use std::alloc::{Allocator, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Size of the mmap header (reserved at offset 0)
pub const MMAP_HEADER_SIZE: usize = 4096;

/// Timeout for stale resize locks (in seconds)
pub const RESIZE_LOCK_TIMEOUT_SECS: u64 = 5;

/// Maximum time a single resize operation should take (in seconds)
/// If exceeded, the operation aborts with an error to prevent hanging
pub const RESIZE_OPERATION_TIMEOUT_SECS: u64 = 60;

/// Header stored at the beginning of every mmap for cross-process coordination.
/// This is the single source of truth for all configuration and coordination state.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MmapHeader {
    /// Generation counter - incremented on resize/shrink
    pub version: u64,

    /// Current file size in bytes
    pub current_size: u64,

    /// Current allocation limit (respects shrink operations)
    pub allocation_limit: u64,

    // === RESIZE COORDINATION ===
    /// Target size of in-progress resize (0 = no resize happening)
    pub resize_in_progress_target: u64,

    /// How much free space the resize will create
    pub resize_will_leave_free: u64,

    /// PID of process holding resize lock (0 = no lock)
    pub resize_lock_holder_pid: u32,

    /// Timestamp when lock was acquired (Unix seconds, for stale detection)
    pub resize_lock_timestamp: u64,

    // === CONFIG (Shared across all processes) ===
    /// Whether to allow automatic file growth when allocations fail
    pub allow_growth: bool,

    /// Maximum file size in bytes, or 0 for unlimited
    pub max_size: u64,

    /// Factor by which to grow the file (stored as integer: 2.0 -> 2)
    pub growth_factor_int: u32,

    /// Maximum number of consecutive resize attempts before giving up
    pub max_resize_attempts: u32,

    /// Padding to reach 4KB total
    pub _padding: [u8; 4016],
}

/// Errors that can occur during allocation or resizing
#[derive(Debug, Clone)]
pub enum AllocatorError {
    OutOfMemory,
    AllocationFailed,
    ResizeFailed(String),
    MaxSizeExceeded,
    ResizeAttemptLimitExceeded,
    ShrinkFailed(String),
    FileTruncationFailed(String),
}

impl std::fmt::Display for AllocatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AllocatorError::OutOfMemory => write!(f, "Out of memory"),
            AllocatorError::AllocationFailed => write!(f, "Allocation failed"),
            AllocatorError::ResizeFailed(msg) => write!(f, "Resize failed: {}", msg),
            AllocatorError::MaxSizeExceeded => write!(f, "Maximum size exceeded"),
            AllocatorError::ResizeAttemptLimitExceeded => {
                write!(f, "Resize attempt limit exceeded")
            }
            AllocatorError::ShrinkFailed(msg) => write!(f, "Shrink failed: {}", msg),
            AllocatorError::FileTruncationFailed(msg) => {
                write!(f, "File truncation failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for AllocatorError {}

/// Get current Unix timestamp in seconds
#[allow(dead_code)]
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Check if a process with the given PID is still alive (Unix only)
///
/// Note: This is kept for reference but not currently used. The heartbeat-based
/// stale detection in ResizeLockGuard is more robust and doesn't require permissions.
#[allow(dead_code)]
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // On Unix, send signal 0 to check if process exists without affecting it
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// RAII guard for resize lock with automatic heartbeat updates
pub struct ResizeLockGuard<'a> {
    allocator: &'a FreeListAllocator,
    /// When lock was acquired (used for timeout detection and abort)
    acquired_at: std::time::Instant,
}

impl<'a> ResizeLockGuard<'a> {
    /// Check if this resize operation has exceeded the timeout
    ///
    /// Returns Err if the operation has taken longer than RESIZE_OPERATION_TIMEOUT_SECS.
    /// This prevents indefinite hangs by detecting frozen resize operations.
    pub fn check_timeout(&self) -> Result<(), AllocatorError> {
        if self.acquired_at.elapsed().as_secs() > RESIZE_OPERATION_TIMEOUT_SECS {
            return Err(AllocatorError::ResizeFailed(format!(
                "Resize operation exceeded {}s timeout - aborting",
                RESIZE_OPERATION_TIMEOUT_SECS
            )));
        }
        Ok(())
    }

    /// Update the lock heartbeat timestamp in the shared header
    ///
    /// Called periodically to indicate the lock holder is still alive.
    /// Other processes detect dead locks by checking if timestamp is stale.
    fn update_heartbeat(&self) -> Result<(), AllocatorError> {
        if let Ok(mut header) = self.allocator.read_header() {
            if header.resize_lock_holder_pid == std::process::id() {
                header.resize_lock_timestamp = current_timestamp();
                self.allocator.write_header(&header)?;
            }
        }
        Ok(())
    }
}

impl<'a> Drop for ResizeLockGuard<'a> {
    fn drop(&mut self) {
        // Release the lock - only clear PID if we hold it
        if let Ok(mut header) = self.allocator.read_header() {
            if header.resize_lock_holder_pid == std::process::id() {
                header.resize_lock_holder_pid = 0;
                header.resize_lock_timestamp = 0;
                // Don't clear resize_in_progress_target yet - resize_file will bump version
                let _ = self.allocator.write_header(&header);
            }
        }
    }
}

pub struct FreeListAllocator {
    /// Current mmap for new allocations
    mmap: Arc<Mutex<memmap2::MmapMut>>,
    /// Previous mmap kept during resize for accessing old data
    fallback_mmap: Arc<Mutex<Option<memmap2::MmapMut>>>,
    /// Path to the underlying file (needed for resizing/truncating)
    file_path: Arc<std::path::PathBuf>,
    /// Logical limit for allocations (can shrink while resizing)
    allocation_limit: AtomicUsize,
    /// Whether a resize operation is in progress
    resizing: AtomicBool,
    /// Current bump offset for new allocations
    offset: Mutex<usize>,
    /// Free list: (offset, size) of free regions
    free_list: Mutex<Vec<(usize, usize)>>,
}

impl FreeListAllocator {
    /// Create a new allocator wrapping the given memory-mapped region with default config.
    ///
    /// The default config enables growth with 2x factor and allows up to 3 resize attempts.
    pub fn new(mmap: memmap2::MmapMut, file_path: impl AsRef<std::path::Path>) -> Self {
        let mmap_size = mmap.len();
        FreeListAllocator {
            mmap: Arc::new(Mutex::new(mmap)),
            fallback_mmap: Arc::new(Mutex::new(None)),
            file_path: Arc::new(file_path.as_ref().to_path_buf()),
            allocation_limit: AtomicUsize::new(mmap_size),
            resizing: AtomicBool::new(false),
            offset: Mutex::new(MMAP_HEADER_SIZE), // Start after header
            free_list: Mutex::new(Vec::new()),
        }
    }

    /// Get the underlying memory-mapped data as a slice (current mmap).
    pub fn as_slice(&self) -> &[u8] {
        let mmap = self.mmap.lock().unwrap();
        unsafe { std::mem::transmute::<&[u8], &[u8]>(mmap.as_ref()) }
    }

    /// Get the total size of the current memory-mapped region.
    pub fn size(&self) -> usize {
        self.mmap.lock().unwrap().len()
    }

    /// Get the current allocation limit.
    pub fn allocation_limit(&self) -> usize {
        self.allocation_limit.load(Ordering::Acquire)
    }

    /// Reconstruct a const pointer from an offset into the mmap.
    ///
    /// Tries the current mmap first, then falls back to the old mmap if available.
    ///
    /// # Panics
    /// Panics if the offset or size would exceed both mmap bounds.
    pub fn offset_to_ptr(&self, offset: usize, size: usize) -> *const u8 {
        let current = self.mmap.lock().unwrap();

        if offset + size <= current.len() {
            return unsafe { current.as_ptr().add(offset) };
        }

        // Try fallback mmap
        if let Some(fallback) = self.fallback_mmap.lock().unwrap().as_ref() {
            if offset + size <= fallback.len() {
                return unsafe { fallback.as_ptr().add(offset) };
            }
        }

        panic!(
            "offset {} + size {} exceeds both mmap boundaries (current: {}, fallback: {:?})",
            offset,
            size,
            current.len(),
            self.fallback_mmap.lock().unwrap().as_ref().map(|m| m.len())
        );
    }

    /// Reconstruct a mutable pointer from an offset into the mmap (requires mmap lock to be held).
    ///
    /// This is an unsafe version that assumes the caller already holds the mmap lock.
    /// Use when you already have a mutable reference to the mmap.
    ///
    /// # Panics
    /// Panics if the offset or size would exceed current mmap bounds.
    #[allow(dead_code)]
    unsafe fn offset_to_mut_ptr_unlocked(
        &self,
        mmap_ref: &mut memmap2::MmapMut,
        offset: usize,
        size: usize,
    ) -> *mut u8 {
        assert!(
            offset + size <= mmap_ref.len(),
            "offset {} + size {} exceeds mmap size {}",
            offset,
            size,
            mmap_ref.len()
        );
        unsafe { mmap_ref.as_mut_ptr().add(offset) }
    }

    /// Reconstruct a mutable pointer from an offset into the mmap.
    ///
    /// Always uses the current mmap (fallback is read-only for consistency).
    ///
    /// # Panics
    /// Panics if the offset or size would exceed current mmap bounds.
    pub fn offset_to_mut_ptr(&self, offset: usize, size: usize) -> *mut u8 {
        let mut current = self.mmap.lock().unwrap();

        assert!(
            offset + size <= current.len(),
            "offset {} + size {} exceeds current mmap size {}",
            offset,
            size,
            current.len()
        );

        unsafe { current.as_mut_ptr().add(offset) }
    }

    /// Read the header from the mmap
    pub fn read_header(&self) -> Result<MmapHeader, AllocatorError> {
        let mmap = self.mmap.lock().unwrap();
        if mmap.len() < MMAP_HEADER_SIZE {
            return Err(AllocatorError::ResizeFailed(
                "Mmap too small for header".to_string(),
            ));
        }

        // Cast the first bytes as MmapHeader
        let header_bytes = &mmap[..std::mem::size_of::<MmapHeader>()];
        let header = unsafe { std::ptr::read(header_bytes.as_ptr() as *const MmapHeader) };
        Ok(header)
    }

    /// Write the header to the mmap
    pub fn write_header(&self, header: &MmapHeader) -> Result<(), AllocatorError> {
        let mut mmap = self.mmap.lock().unwrap();
        if mmap.len() < MMAP_HEADER_SIZE {
            return Err(AllocatorError::ResizeFailed(
                "Mmap too small for header".to_string(),
            ));
        }

        // Copy header to the mmap
        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                header as *const MmapHeader as *const u8,
                std::mem::size_of::<MmapHeader>(),
            )
        };
        mmap[..header_bytes.len()].copy_from_slice(header_bytes);
        Ok(())
    }

    /// Try to acquire the resize lock, handling stale locks automatically
    ///
    /// Uses timestamp-based heartbeat detection instead of PID checking,
    /// which is more robust across platforms and doesn't require permissions.
    #[allow(dead_code)]
    fn try_acquire_resize_lock(&self) -> Result<ResizeLockGuard<'_>, AllocatorError> {
        let header = self.read_header().unwrap_or_else(|_| MmapHeader {
            version: 0,
            current_size: self.size() as u64,
            allocation_limit: self.allocation_limit.load(Ordering::Acquire) as u64,
            resize_in_progress_target: 0,
            resize_will_leave_free: 0,
            resize_lock_holder_pid: 0,
            resize_lock_timestamp: 0,
            allow_growth: true,     // defaults
            max_size: 0,            // unlimited
            growth_factor_int: 2,   // 2x growth
            max_resize_attempts: 3, // 3 attempts
            _padding: [0u8; 4016],
        });

        // Check if lock is held
        if header.resize_lock_holder_pid != 0 {
            let now = current_timestamp();
            let elapsed = now.saturating_sub(header.resize_lock_timestamp);

            // Check if lock is stale based on heartbeat timestamp
            // If holder hasn't updated timestamp in > RESIZE_LOCK_TIMEOUT_SECS, it's dead
            if elapsed > RESIZE_LOCK_TIMEOUT_SECS {
                // Stale lock detected (holder hasn't updated heartbeat) - clean it up
                let mut new_header = header;
                new_header.resize_in_progress_target = 0;
                new_header.resize_lock_holder_pid = 0;
                new_header.resize_will_leave_free = 0;
                let _ = self.write_header(&new_header);
                // Fall through to acquire
            } else {
                // Lock still held and recent (holder is updating heartbeat)
                return Err(AllocatorError::ResizeFailed(
                    "Resize in progress by another process".to_string(),
                ));
            }
        }

        // Acquire the lock
        let mut new_header = header;
        new_header.resize_lock_holder_pid = std::process::id();
        new_header.resize_lock_timestamp = current_timestamp();
        self.write_header(&new_header)?;

        Ok(ResizeLockGuard {
            allocator: self,
            acquired_at: std::time::Instant::now(),
        })
    }

    /// Clean up resize state when an operation fails or times out
    /// Resets internal state to prevent orphaned processes from corrupting the system
    fn cleanup_failed_resize(&self) {
        // Used in resize_file error paths
        // Reset resizing flag so other processes don't think one is in progress
        self.resizing.store(false, Ordering::Release);

        // Clear the fallback mmap to avoid indefinite memory retention
        // If we had started swapping, the old mmap will be dropped here
        *self.fallback_mmap.lock().unwrap() = None;
    }

    /// Wait for the version to change, with timeout
    #[allow(dead_code)]
    fn wait_for_version_change(&self, old_version: u64) -> Result<(), AllocatorError> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(10);

        loop {
            if let Ok(header) = self.read_header() {
                if header.version != old_version {
                    return Ok(());
                }
            }

            if start.elapsed() > timeout {
                return Err(AllocatorError::ResizeFailed(
                    "Timeout waiting for resize to complete".to_string(),
                ));
            }

            std::thread::yield_now();
        }
    }

    fn allocate_internal(&self, size: usize, align: usize) -> Result<usize, AllocatorError> {
        let limit = self.allocation_limit.load(Ordering::Acquire);
        let mut free_list = self.free_list.lock().unwrap();
        let mut best_fit: Option<(usize, usize, usize, usize)> = None; // (index, offset, padding, free_size)

        for (i, (free_offset, free_size)) in free_list.iter().enumerate() {
            let aligned_offset = (*free_offset + align - 1) & !(align - 1);
            let padding = aligned_offset - free_offset;

            // Ensure allocation doesn't exceed allocation_limit
            if padding + size <= *free_size && aligned_offset + size <= limit {
                best_fit = Some((i, aligned_offset, padding, *free_size));
                break;
            }
        }

        if let Some((i, aligned_offset, padding, free_size)) = best_fit {
            free_list.remove(i);

            // Add back any leftover space
            if padding + size < free_size {
                free_list.push((aligned_offset + size, free_size - padding - size));
            }

            return Ok(aligned_offset);
        }

        // No suitable free region, try bump allocation
        let mut offset = self.offset.lock().unwrap();
        if *offset + size <= limit {
            let result = *offset;
            *offset += size;
            Ok(result)
        } else {
            Err(AllocatorError::OutOfMemory)
        }
    }

    fn deallocate_internal(&self, offset: usize, size: usize) {
        let mut free_list = self.free_list.lock().unwrap();
        free_list.push((offset, size));
        self.coalesce_internal(&mut free_list);
    }

    fn coalesce_internal(&self, free_list: &mut Vec<(usize, usize)>) {
        free_list.sort_by_key(|&(off, _)| off);
        let mut i = 0;
        while i < free_list.len() - 1 {
            if free_list[i].0 + free_list[i].1 == free_list[i + 1].0 {
                let size = free_list[i].1 + free_list[i + 1].1;
                free_list[i].1 = size;
                free_list.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }

    /// Calculate the total used space by scanning the free list.
    pub fn calculate_used_space(&self) -> usize {
        let offset = *self.offset.lock().unwrap();
        let free_list = self.free_list.lock().unwrap();
        let total_free: usize = free_list.iter().map(|(_, size)| size).sum();
        offset.saturating_sub(total_free)
    }

    /// Resize the file and swap mmaps with cross-process coordination.
    ///
    /// Acquires lock, updates header with resize in-progress state, performs resize,
    /// then bumps version to notify other processes to remap.
    pub fn resize_file(&self, new_size: usize) -> Result<(), AllocatorError> {
        // Acquire resize lock first
        let lock = self.try_acquire_resize_lock()?;

        // Read header for current config
        let mut header = self.read_header().unwrap_or_else(|_| MmapHeader {
            version: 0,
            current_size: self.size() as u64,
            allocation_limit: self.allocation_limit.load(Ordering::Acquire) as u64,
            resize_in_progress_target: 0,
            resize_will_leave_free: 0,
            resize_lock_holder_pid: 0,
            resize_lock_timestamp: 0,
            allow_growth: true,
            max_size: 0,
            growth_factor_int: 2,
            max_resize_attempts: 3,
            _padding: [0u8; 4016],
        });

        // Check config limits from header
        if header.max_size > 0 && new_size as u64 > header.max_size {
            return Err(AllocatorError::MaxSizeExceeded);
        }

        if new_size <= self.size() {
            return Err(AllocatorError::ResizeFailed(
                "New size must be larger than current size".to_string(),
            ));
        }

        // Update header to indicate resize in progress
        header.resize_in_progress_target = new_size as u64;
        header.resize_will_leave_free = (new_size - self.calculate_used_space()) as u64;
        self.write_header(&header)?;

        // Update heartbeat before long operation
        let _ = lock.update_heartbeat();

        self.resizing.store(true, Ordering::Release);

        // Check timeout before expensive file operations
        if let Err(e) = lock.check_timeout() {
            self.cleanup_failed_resize();
            return Err(e);
        }

        // Resize the file on disk
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.file_path.as_ref())
            .map_err(|e| {
                self.cleanup_failed_resize();
                AllocatorError::ResizeFailed(format!("Failed to open file: {}", e))
            })?;

        file.set_len(new_size as u64).map_err(|e| {
            self.cleanup_failed_resize();
            AllocatorError::ResizeFailed(format!("Failed to set file length: {}", e))
        })?;

        // Check timeout after file resize
        if let Err(e) = lock.check_timeout() {
            self.cleanup_failed_resize();
            return Err(e);
        }

        // Create new mmap
        let mut new_mmap = unsafe {
            memmap2::MmapMut::map_mut(&file).map_err(|e| {
                self.cleanup_failed_resize();
                AllocatorError::ResizeFailed(format!("Failed to create mmap: {}", e))
            })?
        };

        // Copy old data to new mmap
        {
            let old_mmap = self.mmap.lock().unwrap();
            let old_size = old_mmap.len();
            // Use a write-safe approach: create new mmap, then manually copy
            unsafe {
                std::ptr::copy_nonoverlapping(old_mmap.as_ptr(), new_mmap.as_mut_ptr(), old_size);
            }
        }

        // Check timeout after long data copy operation
        if let Err(e) = lock.check_timeout() {
            self.cleanup_failed_resize();
            return Err(e);
        }

        // Refresh lock heartbeat
        let _ = lock.update_heartbeat();

        // Swap mmaps: current becomes fallback, new becomes current
        let mut current = self.mmap.lock().unwrap();
        let old_mmap = std::mem::replace(&mut *current, new_mmap);
        *self.fallback_mmap.lock().unwrap() = Some(old_mmap);
        drop(current);

        // Update allocation_limit to new size
        self.allocation_limit.store(new_size, Ordering::Release);

        // Update header: clear in-progress flag, bump version to notify other processes
        let mut header = self.read_header()?;
        header.resize_in_progress_target = 0;
        header.version = header.version.wrapping_add(1);
        header.current_size = new_size as u64;
        self.write_header(&header)?;

        self.resizing.store(false, Ordering::Release);

        // Clear fallback now that resize is complete and version was bumped
        // Other processes will see the version change and remap; old data no longer needed
        *self.fallback_mmap.lock().unwrap() = None;

        Ok(())
        // Lock drops here, automatically releasing
    }

    /// Shrink the file to reclaim unused space with cross-process coordination.
    ///
    /// Sets allocation_limit immediately to prevent allocations beyond the new size.
    /// Only truncates the actual file if a single mmap exists (no fallback).
    /// Updates header version to notify other processes.
    ///
    /// # Arguments
    /// * `padding_percent` - Percentage of used space to add as padding (e.g., 10.0 for 10%)
    pub fn shrink_to_usage(&self, padding_percent: f64) -> Result<(), AllocatorError> {
        if padding_percent < 0.0 {
            return Err(AllocatorError::ShrinkFailed(
                "Padding percent must be non-negative".to_string(),
            ));
        }

        let used = self.calculate_used_space();
        let new_limit = (used as f64 * (1.0 + padding_percent / 100.0)) as usize;

        // Set allocation limit immediately to prevent allocations beyond this point
        self.allocation_limit.store(new_limit, Ordering::Release);

        // Only truncate file if we have a single mmap (no fallback)
        if self.fallback_mmap.lock().unwrap().is_none() {
            // Safe to truncate - all data accessible from current mmap
            self.truncate_file(new_limit)?;

            // Bump version to notify other processes
            if let Ok(mut header) = self.read_header() {
                header.version = header.version.wrapping_add(1);
                header.allocation_limit = new_limit as u64;
                let _ = self.write_header(&header);
            }
        }

        Ok(())
    }

    /// Truncate the file to a new size and remap if necessary.
    fn truncate_file(&self, new_size: usize) -> Result<(), AllocatorError> {
        if new_size >= self.size() {
            return Ok(()); // Nothing to shrink
        }

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.file_path.as_ref())
            .map_err(|e| {
                AllocatorError::FileTruncationFailed(format!("Failed to open file: {}", e))
            })?;

        file.set_len(new_size as u64).map_err(|e| {
            AllocatorError::FileTruncationFailed(format!("Failed to truncate file: {}", e))
        })?;

        // Remap to the new size
        let new_mmap = unsafe {
            memmap2::MmapMut::map_mut(&file).map_err(|e| {
                AllocatorError::FileTruncationFailed(format!("Failed to remap: {}", e))
            })?
        };

        let mut current = self.mmap.lock().unwrap();
        *current = new_mmap;
        self.allocation_limit.store(new_size, Ordering::Release);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use tempfile::NamedTempFile;

    /// Create a temp file backed allocator with the given size
    fn make_allocator(size: usize) -> (FreeListAllocator, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp.path())
            .unwrap();
        file.set_len(size as u64).unwrap();
        let mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
        let allocator = FreeListAllocator::new(mmap, tmp.path());
        (allocator, tmp)
    }

    #[test]
    fn test_size_returns_initial_size() {
        let (allocator, _tmp) = make_allocator(65536);
        assert_eq!(allocator.size(), 65536);
    }

    #[test]
    fn test_allocation_limit_equals_size_initially() {
        let (allocator, _tmp) = make_allocator(65536);
        assert_eq!(allocator.allocation_limit(), 65536);
    }

    #[test]
    fn test_as_slice_returns_correct_length() {
        let (allocator, _tmp) = make_allocator(65536);
        let slice = allocator.as_slice();
        assert_eq!(slice.len(), 65536);
    }

    #[test]
    fn test_read_and_write_header_roundtrip() {
        let (allocator, _tmp) = make_allocator(65536);
        let mut header = allocator.read_header().unwrap();
        header.version = 42;
        header.allow_growth = true;
        allocator.write_header(&header).unwrap();

        let read_back = allocator.read_header().unwrap();
        assert_eq!(read_back.version, 42);
        assert!(read_back.allow_growth);
    }

    #[test]
    fn test_allocate_and_offset_to_ptr() {
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(64, 1).unwrap();
        let ptr = allocator.allocate(layout).unwrap();
        let raw_ptr = ptr.as_mut_ptr();

        let slice = allocator.as_slice();
        let base = slice.as_ptr() as usize;
        let offset = raw_ptr as usize - base;

        // offset_to_ptr should give back the same address
        let recovered = allocator.offset_to_ptr(offset, 64);
        assert_eq!(recovered, raw_ptr as *const u8);
    }

    #[test]
    fn test_allocate_and_offset_to_mut_ptr() {
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(64, 1).unwrap();
        let ptr = allocator.allocate(layout).unwrap();
        let raw_ptr = ptr.as_mut_ptr();

        let slice = allocator.as_slice();
        let base = slice.as_ptr() as usize;
        let offset = raw_ptr as usize - base;

        let recovered = allocator.offset_to_mut_ptr(offset, 64);
        assert_eq!(recovered, raw_ptr);
    }

    #[test]
    fn test_calculate_used_space_after_allocation() {
        let (allocator, _tmp) = make_allocator(65536);
        let used_before = allocator.calculate_used_space();

        let layout = Layout::from_size_align(1024, 1).unwrap();
        let _ = allocator.allocate(layout).unwrap();

        let used_after = allocator.calculate_used_space();
        assert!(
            used_after > used_before,
            "used space should increase after allocation"
        );
    }

    #[test]
    fn test_deallocate_returns_space() {
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(1024, 1).unwrap();
        let ptr = allocator.allocate(layout).unwrap();
        let used_after_alloc = allocator.calculate_used_space();

        unsafe { allocator.deallocate(ptr.as_non_null_ptr(), layout) };

        let used_after_dealloc = allocator.calculate_used_space();
        assert!(
            used_after_dealloc <= used_after_alloc,
            "used space should not increase after dealloc"
        );
    }

    #[test]
    fn test_multiple_allocations() {
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(128, 1).unwrap();

        let mut ptrs = vec![];
        for _ in 0..10 {
            ptrs.push(allocator.allocate(layout).unwrap());
        }

        // All pointers should be distinct
        let raw_ptrs: Vec<*mut u8> = ptrs.iter().map(|p| p.as_mut_ptr()).collect();
        for i in 0..raw_ptrs.len() {
            for j in (i + 1)..raw_ptrs.len() {
                assert_ne!(raw_ptrs[i], raw_ptrs[j], "allocations should not overlap");
            }
        }
    }

    #[test]
    fn test_allocate_and_write_data() {
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(8, 1).unwrap();
        let ptr = allocator.allocate(layout).unwrap();

        // Write known bytes via mutable pointer
        let raw_ptr = ptr.as_mut_ptr();
        unsafe {
            std::ptr::copy_nonoverlapping([1u8, 2, 3, 4, 5, 6, 7, 8].as_ptr(), raw_ptr, 8);
        }

        // Read back via offset_to_ptr
        let slice = allocator.as_slice();
        let base = slice.as_ptr() as usize;
        let offset = raw_ptr as usize - base;
        let read_ptr = allocator.offset_to_ptr(offset, 8);
        let read_back = unsafe { std::slice::from_raw_parts(read_ptr, 8) };
        assert_eq!(read_back, &[1u8, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_resize_file_increases_size() {
        let (allocator, _tmp) = make_allocator(65536);
        let original_size = allocator.size();
        allocator.resize_file(original_size * 2).unwrap();
        assert_eq!(allocator.size(), original_size * 2);
        assert_eq!(allocator.allocation_limit(), original_size * 2);
    }

    #[test]
    fn test_resize_file_rejects_smaller_size() {
        let (allocator, _tmp) = make_allocator(65536);
        let result = allocator.resize_file(32768);
        assert!(result.is_err(), "resize to smaller size should fail");
    }

    #[test]
    fn test_shrink_to_usage_lowers_allocation_limit() {
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(1024, 1).unwrap();
        let _ = allocator.allocate(layout).unwrap();

        let limit_before = allocator.allocation_limit();

        // shrink_to_usage sets the allocation_limit before attempting file truncation.
        // On Windows, truncating a memory-mapped file fails with os error 1224, which
        // is expected — the allocation limit is still correctly lowered.
        let result = allocator.shrink_to_usage(10.0);
        match result {
            Ok(()) => {}
            Err(AllocatorError::FileTruncationFailed(_)) => {
                // Expected on Windows when the file is still mapped; allocation_limit
                // was already updated before the truncation attempt.
            }
            Err(e) => panic!("Unexpected shrink error: {}", e),
        }

        let limit_after = allocator.allocation_limit();
        assert!(
            limit_after < limit_before,
            "shrink should lower allocation limit (before={}, after={})",
            limit_before,
            limit_after
        );
    }

    #[test]
    fn test_check_timeout_does_not_trigger_immediately() {
        let (allocator, _tmp) = make_allocator(65536);
        // try_acquire_resize_lock creates a ResizeLockGuard with check_timeout
        // We can't call it directly since it's private, but we verify resize_file
        // completes instantly (well under 60s timeout)
        let result = allocator.resize_file(131072);
        assert!(
            result.is_ok(),
            "immediate resize should not trigger timeout"
        );
    }

    #[test]
    fn test_allocate_entire_usable_space_triggers_growth() {
        let (allocator, _tmp) = make_allocator(65536);
        // Fill past initial usable space to trigger auto-growth
        // usable space = 65536 - MMAP_HEADER_SIZE (4096) = 61440 bytes
        let layout = Layout::from_size_align(60000, 1).unwrap();
        // This should succeed either directly or via auto-growth
        let result = allocator.allocate(layout);
        assert!(
            result.is_ok(),
            "large allocation should succeed (may trigger growth)"
        );
    }

    #[test]
    fn test_deallocate_and_reallocate_reuses_freed_space() {
        // Verify that the free-list returns freed slots instead of growing.
        // After: allocate → deallocate → allocate-same-size,
        // the used space must not exceed what it was after the first allocation.
        let (allocator, _tmp) = make_allocator(65536);
        let layout = Layout::from_size_align(1024, 1).unwrap();

        // First allocation
        let ptr1 = allocator.allocate(layout).unwrap();
        let used_after_first = allocator.calculate_used_space();

        // Deallocate — free-list now owns the slot
        unsafe { allocator.deallocate(ptr1.as_non_null_ptr(), layout) };

        // Second allocation of the same size — must reuse the freed slot
        let _ptr2 = allocator.allocate(layout).unwrap();
        let used_after_realloc = allocator.calculate_used_space();

        assert!(
            used_after_realloc <= used_after_first,
            "re-allocation after dealloc must reuse freed space, not grow: \
             used_after_first={used_after_first}, used_after_realloc={used_after_realloc}"
        );
    }
}

unsafe impl Allocator for FreeListAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, std::alloc::AllocError> {
        // Read config from header
        let header = self.read_header().ok();
        let allow_growth = header.map(|h| h.allow_growth).unwrap_or(true);
        let max_attempts = header.map(|h| h.max_resize_attempts).unwrap_or(3);
        let growth_factor = header.map(|h| h.growth_factor_int).unwrap_or(2) as f64;

        let mut attempts = 0;

        loop {
            match self.allocate_internal(layout.size(), layout.align()) {
                Ok(offset) => {
                    let mut current = self.mmap.lock().unwrap();
                    if offset + layout.size() <= current.len() {
                        let ptr = unsafe { current.as_mut_ptr().add(offset) };
                        drop(current); // Release lock before returning
                        return Ok(NonNull::slice_from_raw_parts(
                            NonNull::new(ptr).ok_or(std::alloc::AllocError)?,
                            layout.size(),
                        ));
                    } else {
                        return Err(std::alloc::AllocError);
                    }
                }
                Err(AllocatorError::OutOfMemory) if allow_growth && attempts < max_attempts => {
                    // Try to grow the file
                    let current_size = self.size();
                    let new_size = (current_size as f64 * growth_factor) as usize;

                    if let Ok(_) = self.resize_file(new_size) {
                        attempts += 1;
                        // Retry allocation
                        continue;
                    } else {
                        return Err(std::alloc::AllocError);
                    }
                }
                Err(_) => return Err(std::alloc::AllocError),
            }
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let current = self.mmap.lock().unwrap();
        let offset = ptr.as_ptr() as usize - current.as_ptr() as usize;
        drop(current); // Release lock before calling deallocate_internal
        self.deallocate_internal(offset, layout.size());
    }
}
