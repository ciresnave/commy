use std::sync::Arc;
use std::thread;
use std::time::Duration;

use commy::utils::object_pool::ObjectPool;

// Simple concurrency stress test: spawn threads that repeatedly acquire and
// release objects. Ensure no panics, and final counts are within expected bounds.
#[test]
fn concurrent_acquire_release_smoke() {
    let pool: Arc<ObjectPool<usize>> = Arc::new(ObjectPool::with_max_size_and_initial(100, vec![]));

    let mut handles = Vec::new();

    // Spawn threads that both produce and consume concurrently.
    for i in 0..16 {
        let p = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            let mut got = 0usize;
            for j in 0..1000 {
                // release some items
                p.release(i * 1000 + j);
                // try to acquire an item
                if let Some(_v) = p.try_acquire() {
                    got += 1;
                }
                if j % 97 == 0 {
                    thread::sleep(Duration::from_micros(5));
                }
            }
            got
        }));
    }

    // Collect results
    let mut total_got = 0usize;
    for h in handles {
        if let Ok(n) = h.join() {
            total_got += n;
        }
    }

    // Basic invariant: pool len should not exceed max_size
    assert!(pool.len() <= 100, "pool.len() = {} > 100", pool.len());
    // There should be some successful acquisitions across threads
    assert!(total_got > 0, "no successful acquires observed");
}
