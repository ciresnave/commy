use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A small, flexible object pool used by small subsystems and tests.
///
/// Features:
/// - Shared (Arc) and thread-safe (Mutex) storage for simplicity.
/// - Optional `max_size` to bound memory usage (None means unbounded).
/// - Supports both LIFO-style helpers (`put` / `take` / `get_or`) which
///   mirror the old `utils` behaviour and FIFO-style helpers
///   (`release` / `try_acquire`) which mirror the old `manager` behaviour.
///
/// The internal storage is a `VecDeque<T>` so both front/back operations
/// are efficient; callers can use the API that best matches their
/// performance/fairness needs.
#[derive(Debug, Clone)]
pub struct ObjectPool<T> {
    inner: Arc<Mutex<VecDeque<T>>>,
    max_size: Option<usize>,
}

impl<T> ObjectPool<T> {
    /// Create an unbounded pool with the given initial capacity.
    /// This preserves the old `utils::ObjectPool::with_capacity` behaviour.
    pub fn with_capacity(cap: usize) -> Self {
        let q = VecDeque::with_capacity(cap);
        Self {
            inner: Arc::new(Mutex::new(q)),
            max_size: None,
        }
    }

    /// Create a bounded pool with an initial set of items.
    /// This preserves the old `manager::ObjectPool::with_capacity(max, initial)` behaviour.
    pub fn with_max_size_and_initial(max_size: usize, initial: Vec<T>) -> Self {
        let mut q = VecDeque::new();
        for item in initial.into_iter() {
            q.push_back(item);
        }
        Self {
            inner: Arc::new(Mutex::new(q)),
            max_size: Some(max_size),
        }
    }

    /// Put an object back into the pool (LIFO-friendly when paired with `take`).
    pub fn put(&self, item: T) {
        let mut guard = self.inner.lock().unwrap();
        guard.push_back(item);
    }

    /// Try to take an object from the pool using LIFO semantics.
    pub fn take(&self) -> Option<T> {
        let mut guard = self.inner.lock().unwrap();
        guard.pop_back()
    }

    /// Acquire an object or, if none available, create one using the provided
    /// factory closure. This mirrors the old `get_or` helper.
    pub fn get_or<F>(&self, factory: F) -> T
    where
        F: FnOnce() -> T,
    {
        self.take().unwrap_or_else(factory)
    }

    /// Try to acquire an object using FIFO semantics (pop front).
    pub fn try_acquire(&self) -> Option<T> {
        let mut guard = self.inner.lock().unwrap();
        guard.pop_front()
    }

    /// Return an object to the pool using FIFO-friendly semantics (push back).
    /// If a max_size is set and the pool is full, the item is dropped.
    pub fn release(&self, item: T) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(max) = self.max_size {
            if guard.len() < max {
                guard.push_back(item);
            }
        } else {
            guard.push_back(item);
        }
    }

    /// Current number of items in the pool
    pub fn len(&self) -> usize {
        let guard = self.inner.lock().unwrap();
        guard.len()
    }

    /// Whether the pool is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Default for ObjectPool<T> {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectPool;

    #[test]
    fn basic_put_take_lifo() {
        let pool = ObjectPool::with_capacity(2);
        pool.put(10);
        pool.put(20);

        // LIFO: last pushed is first popped
        assert_eq!(pool.take(), Some(20));
        assert_eq!(pool.take(), Some(10));
        assert_eq!(pool.take(), None);
    }

    #[test]
    fn fifo_release_acquire_with_max() {
        let pool = ObjectPool::with_max_size_and_initial(2, vec![1, 2]);
        assert_eq!(pool.try_acquire(), Some(1));
        assert_eq!(pool.try_acquire(), Some(2));
        assert_eq!(pool.try_acquire(), None);

        pool.release(3);
        pool.release(4);
        // max_size == 2, further releases will be dropped
        pool.release(5);
        assert_eq!(pool.len(), 2);
    }
}
