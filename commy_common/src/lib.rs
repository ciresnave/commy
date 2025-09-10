use memmap2::MmapMut;
use std::{
    error::Error,
    fs::{File, OpenOptions},
    path::Path,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
pub struct FieldHolder<T> {
    value: T,
    writer_id: usize,
    field_name: String,
}

impl<T> FieldHolder<T> {
    pub fn new(value: T, writer_id: usize) -> Self {
        FieldHolder {
            value,
            writer_id,
            field_name: String::new(),
        }
    }

    pub fn new_with_name(value: T, writer_id: usize, field_name: String) -> Self {
        FieldHolder {
            value,
            writer_id,
            field_name,
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn set(&mut self, value: T, writer_id: usize) {
        self.value = value;
        self.writer_id = writer_id;

        // Trigger callback if registered
        if !self.field_name.is_empty() {
            let callback_key = format!("{}_{}", writer_id, &self.field_name);
            invoke_callback(&callback_key, writer_id);
        }
    }

    pub fn get_writer_id(&self) -> usize {
        self.writer_id
    }

    pub fn get_field_name(&self) -> &str {
        &self.field_name
    }

    /// Registers callbacks for the current field.
    pub fn register_callback(&self, callback: Arc<Mutex<dyn FnMut(usize) + Send>>) {
        let callback_key = format!("{}_{}", self.writer_id, &self.field_name);
        register_callback(&callback_key, callback);
    }
}

// Specific implementation for Vec<u8> (shared memory operations)
impl FieldHolder<Vec<u8>> {
    /// Create a new shared memory file and return a FieldHolder for it
    pub fn create(path: impl AsRef<Path>, size: usize) -> std::io::Result<FieldHolder<Vec<u8>>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        file.set_len(size as u64)?;

        // Initialize with empty data
        let data = vec![0u8; size];

        Ok(FieldHolder {
            value: data,
            writer_id: 0, // Default writer ID
            field_name: String::new(),
        })
    }

    /// Open an existing shared memory file and return a FieldHolder for it
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<FieldHolder<Vec<u8>>> {
        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;
        let size = metadata.len() as usize;

        // Read existing data
        let data = vec![0u8; size];
        // Note: For a real implementation, we'd use memory mapping here
        // For now, we'll create an empty buffer

        Ok(FieldHolder {
            value: data,
            writer_id: 0, // Default writer ID
            field_name: String::new(),
        })
    }

    /// Write raw bytes to the field holder
    pub fn write_raw(&mut self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.value.len() {
            return Err("Write would exceed buffer size");
        }

        self.value[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Read raw bytes from the field holder
    pub fn read_raw(&self, offset: usize, length: usize) -> Result<Vec<u8>, &'static str> {
        if offset + length > self.value.len() {
            return Err("Read would exceed buffer size");
        }

        Ok(self.value[offset..offset + length].to_vec())
    }
}

/// Trait for types that require unique writer IDs.
///
/// This trait provides a mechanism to generate and retrieve unique writer IDs
/// in a thread-safe manner using an `AtomicUsize`. Implementors must provide
/// a static `AtomicUsize` counter.
///
/// # Examples
///
/// ```
/// struct MyWriter;
/// impl WithUniqueId for MyWriter {
///     fn writer_id_counter() -> &'static AtomicUsize {
///         static COUNTER: AtomicUsize = AtomicUsize::new(0);
///         &COUNTER
///     }
/// }
///
/// let id = MyWriter::next_writer_id().expect("Failed to get next ID");
/// ```
pub trait WithUniqueId {
    /// Returns a reference to a static `AtomicUsize` used for generating unique IDs.
    ///
    /// Implementors must ensure this returns a reference to a unique `AtomicUsize` for each type.
    fn id_counter() -> &'static AtomicUsize;

    /// Retrieves the next available writer ID, incrementing the counter atomically.
    ///
    /// This method encapsulates the logic for safely incrementing the ID counter,
    /// ensuring thread safety and handling potential overflow errors.
    ///
    /// # Returns
    /// - `Ok(usize)`: The next available writer ID.
    /// - `Err(&'static str)`: An error message if the counter overflows.
    ///
    /// # Examples
    ///
    /// ```
    /// let next_id = MyType::next_id().expect("Failed to get next ID");
    /// ```
    fn next_id() -> Result<usize, &'static str> {
        let counter = Self::id_counter();
        let id = counter.fetch_add(1, Ordering::Relaxed);
        if id == usize::MAX {
            Err("Writer ID counter has overflowed")
        } else {
            Ok(id)
        }
    }
}

pub struct WriterStruct<'a, T> {
    pub mmap: Pin<MmapMut>,
    pub data: &'a mut T,
    pub writer_id: usize,
}

impl<'a, T: WithUniqueId> WriterStruct<'a, T> {
    pub fn new(file_path: impl AsRef<Path>) -> std::io::Result<WriterStruct<'a, T>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(file_path)?;

        file.set_len(std::mem::size_of::<T>() as u64)?;

        let mmap = unsafe { Pin::new(MmapMut::map_mut(&file)?) };
        let writer_id = T::id_counter().fetch_add(1, Ordering::Relaxed);

        // Create the reference directly from the mmap pointer
        let data_ptr = mmap.as_ptr() as *mut T;
        let data_ref = unsafe { &mut *data_ptr };

        let mmap_struct = WriterStruct {
            mmap,
            data: data_ref,
            writer_id,
        };

        Ok(mmap_struct)
    }
}

static CALLBACK_REGISTRY: once_cell::sync::Lazy<
    dashmap::DashMap<String, Arc<Mutex<dyn FnMut(usize) + Send>>>,
> = once_cell::sync::Lazy::new(|| dashmap::DashMap::new());

pub fn register_callback(identifier: &str, callback: Arc<Mutex<dyn FnMut(usize) + Send>>) {
    CALLBACK_REGISTRY.insert(identifier.to_string(), callback);
}

pub fn invoke_callback(identifier: &str, value: usize) {
    if let Some(callback) = CALLBACK_REGISTRY.get(identifier) {
        if let Ok(mut callback) = callback.lock() {
            (*callback)(value);
        }
    }
}

pub fn remove_callback(identifier: &str) -> bool {
    CALLBACK_REGISTRY.remove(identifier).is_some()
}

pub fn list_callback_identifiers() -> Vec<String> {
    // Simple implementation for now
    vec![]
}

pub struct MappedFile<T> {
    mmap: MmapMut,
    _marker: std::marker::PhantomData<T>,
}

impl<T> MappedFile<T> {
    pub fn new(path: &Path, is_write: bool) -> Result<Self, Box<dyn Error>> {
        let file = if is_write {
            File::create(path)?
        } else {
            File::open(path)?
        };
        let mmap = unsafe { MmapMut::map_mut(&file)? };
        Ok(MappedFile {
            mmap,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn get_mut(&mut self) -> &mut T {
        let ptr = self.mmap.as_mut_ptr() as *mut T;
        unsafe { &mut *ptr }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_counter_increments_correctly() {
        // Assuming a simple implementation of WithUniqueId for a test type
        struct TestType;
        impl WithUniqueId for TestType {
            fn id_counter() -> &'static AtomicUsize {
                static COUNTER: AtomicUsize = AtomicUsize::new(0);
                &COUNTER
            }
        }

        let initial_id = TestType::id_counter().load(Ordering::SeqCst);
        let _ = TestType::id_counter().fetch_add(1, Ordering::SeqCst);
        let new_id = TestType::id_counter().load(Ordering::SeqCst);

        assert_eq!(new_id, initial_id + 1);
    }
}

/// Synchronization primitives for coordinated access between processes
pub struct ProcessSynchronizer {
    lock_file_path: String,
    is_locked: AtomicBool,
}

impl ProcessSynchronizer {
    pub fn new(lock_file_path: &str) -> Self {
        ProcessSynchronizer {
            lock_file_path: lock_file_path.to_string(),
            is_locked: AtomicBool::new(false),
        }
    }

    pub fn try_lock(&self) -> Result<bool, Box<dyn Error>> {
        if self.is_locked.load(Ordering::SeqCst) {
            return Ok(false);
        }

        // Try to create lock file exclusively
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.lock_file_path)
        {
            Ok(_) => {
                self.is_locked.store(true, Ordering::SeqCst);
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub fn unlock(&self) -> Result<(), Box<dyn Error>> {
        if self.is_locked.load(Ordering::SeqCst) {
            std::fs::remove_file(&self.lock_file_path)?;
            self.is_locked.store(false, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.is_locked.load(Ordering::SeqCst)
    }
}

impl Drop for ProcessSynchronizer {
    fn drop(&mut self) {
        let _ = self.unlock();
    }
}

/// Reader struct for accessing memory-mapped data from other processes
pub struct ReaderStruct<T> {
    mmap: memmap2::Mmap,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ReaderStruct<T> {
    pub fn new(file_path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = File::open(file_path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        Ok(ReaderStruct {
            mmap,
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn data(&self) -> &T {
        unsafe { &*(self.mmap.as_ptr() as *const T) }
    }
}
