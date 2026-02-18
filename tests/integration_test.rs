use commy::allocator::{FreeListAllocator, MmapHeader};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn setup_test_file(name: &str, size: usize) -> PathBuf {
    let path = std::env::temp_dir().join(format!("commy_test_{}.bin", name));
    let mut file = File::create(&path).unwrap();
    file.write_all(&vec![0u8; size]).unwrap();
    file.flush().unwrap();
    drop(file);
    path
}

fn cleanup_test_file(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_mmap_header_structure() {
    // Verify header size is exactly 4096 bytes
    assert_eq!(std::mem::size_of::<MmapHeader>(), 4096);
}

#[test]
fn test_header_read_write() {
    let path = setup_test_file("header_test", 8192);

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
    let allocator = FreeListAllocator::new(mmap, &path);

    // Create a test header
    let test_header = MmapHeader {
        version: 42,
        current_size: 8192,
        allocation_limit: 4096,
        resize_in_progress_target: 0,
        resize_will_leave_free: 0,
        resize_lock_holder_pid: std::process::id(),
        resize_lock_timestamp: 12345,
        allow_growth: true,
        max_size: 0,
        growth_factor_int: 2,
        max_resize_attempts: 3,
        _padding: [0u8; 4016],
    };

    // Write header
    allocator.write_header(&test_header).unwrap();

    // Read it back
    let read_header = allocator.read_header().unwrap();

    // Verify all fields match
    assert_eq!(read_header.version, 42);
    assert_eq!(read_header.current_size, 8192);
    assert_eq!(read_header.allocation_limit, 4096);
    assert_eq!(read_header.resize_lock_holder_pid, std::process::id());
    assert_eq!(read_header.resize_lock_timestamp, 12345);
    assert_eq!(read_header.allow_growth, true);
    assert_eq!(read_header.growth_factor_int, 2);
    assert_eq!(read_header.max_resize_attempts, 3);

    cleanup_test_file(&path);
}

#[test]
fn test_header_version_bump() {
    let path = setup_test_file("version_test", 8192);

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
    let allocator = FreeListAllocator::new(mmap, &path);

    // Read initial header (uninitialized, should be zeroes)
    let header1 = allocator.read_header().unwrap_or(MmapHeader {
        version: 0,
        current_size: 0,
        allocation_limit: 0,
        resize_in_progress_target: 0,
        resize_will_leave_free: 0,
        resize_lock_holder_pid: 0,
        resize_lock_timestamp: 0,
        allow_growth: false,
        max_size: 0,
        growth_factor_int: 0,
        max_resize_attempts: 0,
        _padding: [0u8; 4016],
    });

    let initial_version = header1.version;

    // Bump version
    let mut header2 = header1;
    header2.version = header2.version.wrapping_add(1);
    allocator.write_header(&header2).unwrap();

    // Read back and verify version changed
    let header3 = allocator.read_header().unwrap();
    assert_eq!(header3.version, initial_version.wrapping_add(1));

    cleanup_test_file(&path);
}

#[test]
fn test_header_config_storage() {
    let path = setup_test_file("config_test", 8192);

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).unwrap() };
    let allocator = FreeListAllocator::new(mmap, &path);

    // Create header with specific config
    let config_header = MmapHeader {
        version: 1,
        current_size: 16384,
        allocation_limit: 12288,
        resize_in_progress_target: 0,
        resize_will_leave_free: 0,
        resize_lock_holder_pid: 0,
        resize_lock_timestamp: 0,
        allow_growth: true,
        max_size: 1_000_000,  // 1MB max
        growth_factor_int: 3, // 3x growth
        max_resize_attempts: 5,
        _padding: [0u8; 4016],
    };

    allocator.write_header(&config_header).unwrap();

    // Read back config
    let read = allocator.read_header().unwrap();

    assert_eq!(read.allow_growth, true);
    assert_eq!(read.max_size, 1_000_000);
    assert_eq!(read.growth_factor_int, 3);
    assert_eq!(read.max_resize_attempts, 5);

    cleanup_test_file(&path);
}
