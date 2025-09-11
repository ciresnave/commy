/// Compatibility wrapper: re-export the utils ObjectPool with the manager-friendly API.
pub use crate::utils::object_pool::ObjectPool as _UnifiedObjectPool;

/// Manager-facing ObjectPool type preserving the original API surface.
#[derive(Debug, Clone)]
pub struct ObjectPool<T> {
    inner: _UnifiedObjectPool<T>,
}

impl<T> ObjectPool<T> {
    /// Create a new pool with a maximum size and an initial set of items.
    pub fn with_capacity(max_size: usize, initial: Vec<T>) -> Self {
        Self {
            inner: _UnifiedObjectPool::with_max_size_and_initial(max_size, initial),
        }
    }

    /// Acquire an object from the pool, or return None if empty.
    pub fn try_acquire(&self) -> Option<T> {
        self.inner.try_acquire()
    }

    /// Return an object to the pool. If the pool is already at max_size, the object
    /// will be dropped.
    pub fn release(&self, item: T) {
        self.inner.release(item)
    }

    /// Current number of items in the pool
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the pool is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<T> Default for ObjectPool<T> {
    fn default() -> Self {
        Self::with_capacity(0, vec![])
    }
}
