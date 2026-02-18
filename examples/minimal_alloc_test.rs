#![feature(allocator_api)]
#![feature(slice_ptr_get)]

use std::alloc::Allocator;
use std::fs;

fn main() {
    let test_file = "test_minimal_alloc.mmap";

    // Clean up any existing test file
    let _ = fs::remove_file(test_file);

    println!("Creating file...");
    let start = std::time::Instant::now();

    // Create a small test file
    fs::write(test_file, vec![0u8; 1024 * 1024]).expect("Failed to write file");
    println!("File created in {:?}", start.elapsed());

    // Open mmap
    let start = std::time::Instant::now();
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(test_file)
        .expect("Failed to open file");
    println!("File opened in {:?}", start.elapsed());

    let start = std::time::Instant::now();
    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).expect("Failed to mmap") };
    println!("Mmap created in {:?}", start.elapsed());

    // Initialize allocator
    let start = std::time::Instant::now();
    let allocator = commy::FreeListAllocator::new(mmap, "test_allocator");
    println!("Allocator initialized in {:?}", start.elapsed());

    // Single allocation
    let start = std::time::Instant::now();
    let layout = std::alloc::Layout::new::<i32>();
    let result = allocator.allocate(layout);
    println!(
        "Single allocation in {:?}, result: {}",
        start.elapsed(),
        if result.is_ok() { "OK" } else { "FAILED" }
    );

    if let Ok(allocated) = result {
        // Single deallocation
        let start = std::time::Instant::now();
        unsafe {
            allocator.deallocate(allocated.as_non_null_ptr(), layout);
        }
        println!("Single deallocation in {:?}", start.elapsed());
    }

    println!("\n✓ Test completed successfully!");

    // Clean up
    let _ = fs::remove_file(test_file);
}
