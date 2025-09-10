use std::sync::{Arc, Mutex};

/// Very small generic object pool for reusing heap objects in tests and
/// low-risk hot paths. This is intentionally simple and safe: a Mutex
/// protects a Vec of boxed objects. Not optimized for extremely high
/// contention workloads but fine for common reuse scenarios.
#[derive(Clone)]
pub struct ObjectPool<T> {
    inner: Arc<Mutex<Vec<T>>>,
}

impl<T> ObjectPool<T> {
    /// Create a new pool with optional initial capacity
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::with_capacity(cap))),
        }
    }

    /// Put an object back into the pool
    pub fn put(&self, item: T) {
        let mut guard = self.inner.lock().unwrap();
        guard.push(item);
    }

    /// Try to take an object from the pool; returns Some(item) or None
    pub fn take(&self) -> Option<T> {
        let mut guard = self.inner.lock().unwrap();
        guard.pop()
    }

    /// Acquire an object or, if none available, create one using the provided
    /// factory closure. This helper makes the common pattern concise.
    pub fn get_or<F>(&self, factory: F) -> T
    where
        F: FnOnce() -> T,
    {
        self.take().unwrap_or_else(factory)
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectPool;

    #[test]
    fn basic_put_take() {
        let pool = ObjectPool::with_capacity(2);
        pool.put(10);
        pool.put(20);

        let a = pool.take();
        let b = pool.take();
        let c = pool.take();

        let mut vals = vec![];
        if let Some(v) = a {
            vals.push(v);
        }
        if let Some(v) = b {
            vals.push(v);
        }
        assert_eq!(vals.len(), 2);
        assert!(c.is_none());
    }
}
