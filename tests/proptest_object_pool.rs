use std::sync::Arc;
use std::thread;
use std::time::Duration;

use proptest::prelude::*;

use commy::utils::object_pool::ObjectPool;

// Randomized sequences: generate operation counts and interleave releases and acquires
proptest! {
    #[test]
    fn randomized_ops(num_threads in 1usize..8, ops_per_thread in 10usize..200) {
        let pool = Arc::new(ObjectPool::with_max_size_and_initial(50, vec![]));

        let mut handles = vec![];
        for t in 0..num_threads {
            let p = Arc::clone(&pool);
            handles.push(thread::spawn(move || {
                let mut got = 0usize;
                for i in 0..ops_per_thread {
                    // randomly choose op based on simple pattern
                    if (i + t) % 3 == 0 {
                        p.release(i + t);
                    } else if p.try_acquire().is_some() {
                        got += 1;
                    }
                    if i % 17 == 0 { thread::sleep(Duration::from_micros(1)); }
                }
                got
            }));
        }

        let mut total = 0usize;
        for h in handles {
            total += h.join().unwrap_or(0);
        }

        // invariants
        assert!(pool.len() <= 50);
        // either some acquires happened or the pool contains items
    assert!(total > 0 || !pool.is_empty());
    }
}
