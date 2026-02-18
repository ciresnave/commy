#![feature(allocator_api)]
#![feature(slice_ptr_get)]

use std::alloc::Allocator;
use std::fs;
use std::process::Command;
use std::time::Instant;

fn main() {
    let test_file = "stress_test_intensive.mmap";
    let num_processes = 8;
    let allocations_per_process = 200;

    // Check if we're a child process
    if let Ok(child_id) = std::env::var("STRESS_TEST_CHILD") {
        run_child(&child_id, test_file, allocations_per_process);
        return;
    }

    // Parent: set up test file and spawn children
    let _ = fs::remove_file(test_file);

    println!("Creating {}MB test file...", 50);
    fs::write(test_file, vec![0u8; 50 * 1024 * 1024]).expect("Failed to write file");

    let start = Instant::now();

    println!(
        "Spawning {} processes with {} allocations each...",
        num_processes, allocations_per_process
    );

    let mut handles = vec![];

    for i in 0..num_processes {
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
        println!(
            "\n✓ Intensive stress test PASSED in {:.2}s",
            elapsed.as_secs_f64()
        );
        let total_ops = num_processes * allocations_per_process;
        println!(
            "  {} processes × {} allocations = {} total ops",
            num_processes, allocations_per_process, total_ops
        );
        println!(
            "  Throughput: {:.0} ops/sec",
            total_ops as f64 / elapsed.as_secs_f64()
        );
    } else {
        eprintln!("\n✗ Intensive stress test FAILED!");
        std::process::exit(1);
    }

    // Clean up
    let _ = fs::remove_file(test_file);
}

fn run_child(child_id: &str, test_file: &str, num_allocations: usize) {
    // Give parent time to set up
    std::thread::sleep(std::time::Duration::from_millis(200));

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
            }
            Err(_e) => {
                eprintln!("Child {}: Allocation {} failed", child_id, i);
                std::process::exit(1);
            }
        }

        // Occasionally deallocate old allocations to test fragmentation
        if allocations.len() > 10 && i % 11 == 0 {
            for _ in 0..3 {
                if let Some((ptr, layout)) = allocations.pop() {
                    unsafe {
                        allocator.deallocate(ptr.as_non_null_ptr(), layout);
                    }
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
}
