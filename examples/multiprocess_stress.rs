#![feature(allocator_api)]
#![feature(slice_ptr_get)]

use std::alloc::Allocator;
use std::fs;
use std::process::Command;
use std::time::Instant;

fn main() {
    let test_file = "stress_test.mmap";
    let num_processes = 4;
    let allocations_per_process = 50;

    // Check if we're a child process
    if let Ok(child_id) = std::env::var("STRESS_TEST_CHILD") {
        run_child(&child_id, test_file, allocations_per_process);
        return;
    }

    // Parent: set up test file and spawn children
    let _ = fs::remove_file(test_file);

    println!("Creating {}MB test file...", 10);
    fs::write(test_file, vec![0u8; 10 * 1024 * 1024]).expect("Failed to write file");

    let start = Instant::now();

    println!(
        "Spawning {} processes with {} allocations each...",
        num_processes, allocations_per_process
    );

    let mut handles = vec![];

    for i in 0..num_processes {
        let _test_file_clone = test_file.to_string();
        let handle = std::thread::spawn(move || {
            let output = Command::new(std::env::current_exe().unwrap())
                .env("STRESS_TEST_CHILD", i.to_string())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .expect("Failed to spawn child");

            if !output.status.success() {
                eprintln!(
                    "Child {} failed:\n{}\n{}",
                    i,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                false
            } else {
                println!("Child {} completed successfully", i);
                true
            }
        });
        handles.push(handle);
    }

    // Wait for all children
    let mut success = true;
    for (i, handle) in handles.into_iter().enumerate() {
        if !handle.join().unwrap() {
            success = false;
            eprintln!("Child process {} failed", i);
        }
    }

    let elapsed = start.elapsed();

    if success {
        println!("\n✓ Stress test completed in {:.2}s", elapsed.as_secs_f64());
        println!(
            "  {} processes × {} allocations = {} total ops",
            num_processes,
            allocations_per_process,
            num_processes * allocations_per_process
        );
        println!(
            "  Throughput: {:.0} ops/sec",
            (num_processes * allocations_per_process) as f64 / elapsed.as_secs_f64()
        );
    } else {
        eprintln!("\n✗ Stress test failed!");
        std::process::exit(1);
    }

    // Clean up
    let _ = fs::remove_file(test_file);
}

fn run_child(child_id: &str, test_file: &str, num_allocations: usize) {
    // Give parent time to set up
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Open the test file
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(test_file)
        .expect("Child: Failed to open file");

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).expect("Child: Failed to mmap") };
    let allocator = commy::FreeListAllocator::new(mmap, "stress_test");

    let mut allocations = vec![];

    // Perform allocations
    for i in 0..num_allocations {
        let layout = std::alloc::Layout::new::<[u64; 16]>(); // 128 bytes each
        match allocator.allocate(layout) {
            Ok(ptr) => {
                allocations.push((ptr, layout));
                if i % 10 == 0 {
                    eprintln!(
                        "Child {}: Allocated {}/{}",
                        child_id,
                        i + 1,
                        num_allocations
                    );
                }
            }
            Err(e) => {
                eprintln!("Child {}: Allocation {} failed: {:?}", child_id, i, e);
                std::process::exit(1);
            }
        }

        // Occasionally deallocate old allocations to test fragmentation
        if allocations.len() > 5 && i % 7 == 0 {
            if let Some((ptr, layout)) = allocations.pop() {
                unsafe {
                    allocator.deallocate(ptr.as_non_null_ptr(), layout);
                }
            }
        }
    }

    // Clean up remaining allocations
    while let Some((ptr, layout)) = allocations.pop() {
        unsafe {
            allocator.deallocate(ptr.as_non_null_ptr(), layout);
        }
    }

    eprintln!("Child {}: All operations completed successfully", child_id);
}
