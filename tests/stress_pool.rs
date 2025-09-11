#![cfg(feature = "stress-tests")]

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use commy::utils::object_pool::ObjectPool;

// Long-running stress test: use with --features stress-tests and run manually.
#[test]
#[ignore]
fn long_running_stress() {
    let pool = Arc::new(ObjectPool::with_max_size_and_initial(500, vec![]));

    let start = Instant::now();
    let mut handles = Vec::new();

    for i in 0..64 {
        let p = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            let mut local = 0usize;
            while start.elapsed() < Duration::from_secs(10) {
                for j in 0..1000 {
                    p.release(i * 1000 + j);
                    if p.try_acquire().is_some() {
                        local += 1;
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
            local
        }));
    }

    let mut total = 0usize;
    for h in handles {
        total += h.join().unwrap_or(0);
    }

    // sanity checks
    assert!(pool.len() <= 500);
    assert!(total > 0);
}
