/// Offset-based container types for use with the FreeListAllocator.
///
/// These containers store their data in shared memory via offsets rather than pointers,
/// making them safe to use across process boundaries.
///
/// # Type Safety
/// All types require `T: Copy` to be safely shared across processes.
///
/// # Conversion
/// All container types implement `from_*` constructors and `into_*` methods for conversion
/// between Rust's standard containers and their offset-based equivalents.
///
/// # Container Types
///
/// ## Sequences
/// - `SharedVec<T>` - Dynamic array, equivalent to `Vec<T>`
/// - `SharedVecDeque<T>` - Double-ended queue, equivalent to `VecDeque<T>`
/// - `SharedString` - UTF-8 string, equivalent to `String`
///
/// ## Single Value
/// - `SharedBox<T>` - Single-value allocation, equivalent to `Box<T>`
///
/// ## Maps & Sets (Hash-based)
/// - `SharedHashMap<K, V>` - Hash map using linear probing, equivalent to `HashMap<K, V>`
/// - `SharedHashSet<T>` - Hash set using linear probing, equivalent to `HashSet<T>`
///
/// ## Maps & Sets (Tree-based, Ordered)
/// - `SharedBTreeMap<K, V>` - Ordered map backed by sorted SharedVec, equivalent to `BTreeMap<K, V>`
/// - `SharedBTreeSet<T>` - Ordered set backed by sorted SharedVec, equivalent to `BTreeSet<T>`
///
/// ## Lists
/// - `SharedLinkedList<T>` - Singly-linked list, equivalent to `LinkedList<T>`
///
/// # API Compatibility
///
/// Each container type provides methods that closely match their standard library equivalents:
///
/// **SharedVec** provides: `push()`, `pop()`, `insert()`, `remove()`, `clear()`, `get()`,
/// `len()`, `is_empty()`, `capacity()`, `reserve()`, `append()`, `extend()`, `retain()`,
/// `resize()`, `dedup()`, `sort_by_key()`, `reverse()`, `fill()`, `binary_search()`, etc.
///
/// **SharedString** provides: `push_str()`, `push_char()`, `pop()`, `clear()`, `chars()`,
/// `split()`, `trim()`, `starts_with()`, `ends_with()`, `replace()`, `to_uppercase()`,
/// `to_lowercase()`, `contains_substring()`, `is_char_boundary()`, etc.
///
/// **SharedHashMap** provides: `insert()`, `get()`, `get_mut()`, `remove()`, `contains_key()`,
/// `len()`, `is_empty()`, `clear()`, `iter()`, `keys()`, `values()`, `retain()`, etc.
///
/// **SharedHashSet** provides: `insert()`, `contains()`, `remove()`, `iter()`, `len()`,
/// `is_empty()`, `clear()`, `is_subset()`, `is_superset()`, `is_disjoint()`, `retain()`, etc.
///
/// **SharedBTreeMap** provides: `insert()`, `get()`, `get_mut()`, `remove()`, `contains_key()`,
/// `iter()`, `keys()`, `values()`, `first()`, `last()`, `retain()`, etc. (Maintains sorted order)
///
/// **SharedBTreeSet** provides: `insert()`, `contains()`, `remove()`, `iter()`, `len()`,
/// `is_empty()`, `first()`, `last()`, `is_subset()`, `is_superset()`, `is_disjoint()`, etc. (Maintains sorted order)
///
/// **SharedVecDeque** provides: `push_back()`, `push_front()`, `pop_back()`, `pop_front()`,
/// `front()`, `back()`, `len()`, `is_empty()`, `clear()`, `iter()`, etc.
///
/// **SharedLinkedList** provides: `push_back()`, `push_front()`, `pop_front()`, `front()`,
/// `len()`, `is_empty()`, `clear()`, `iter()`, etc.
///
/// # Trait Implementations
///
/// All containers implement:
/// - `Debug` for debugging output
/// - `Deref`/`DerefMut` where applicable (e.g., SharedVec derefs to `[T]`, SharedString to `str`)
/// - `PartialEq`/`Eq` for comparisons
/// - `Index`/`IndexMut` where applicable
/// - `Default` (panics - requires allocator context; use `new_in()` instead)
///
/// # Performance Notes
///
/// - **HashMap/HashSet**: Use linear probing. O(1) average for inserts/lookups in sparse maps,
///   but degrade to O(n) in heavily populated maps.
/// - **BTreeMap/BTreeSet**: Use sorted SharedVec with binary search. O(log n) lookups but O(n)
///   insertions/deletions due to element shifting.
/// - **Linked lists**: O(1) front operations but O(n) random access.
/// - **Reallocations**: Vectors reallocate with geometric growth strategy (2x).
///
/// # Example
///
/// ```no_run
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::{SharedVec, SharedString, SharedHashMap};
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
///
/// // Dynamic array
/// let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
/// vec.push(42);
/// vec.push(100);
/// assert_eq!(vec.len(), 2);
/// assert_eq!(vec[0], 42);
///
/// // String
/// let mut s: SharedString = SharedString::new_in(&allocator);
/// s.push_str("hello");
/// s.push_char('!');
/// assert_eq!(s.as_str(), "hello!");
///
/// // Hash map
/// let mut map: SharedHashMap<i32, i32> = SharedHashMap::new_in(&allocator);
/// map.insert(1, 100);
/// map.insert(2, 200);
/// assert_eq!(map.get(&1), Some(&100));
/// # Ok(())
/// # }
/// ```
///
/// # Cross-Process Usage
///
/// All these containers can be stored in shared memory and accessed from multiple processes
/// through the same memory-mapped file. The offset-based design ensures that:
///
/// 1. Container metadata (offsets, lengths, capacities) is serializable as plain integers
/// 2. Pointers are reconstructed on-demand via the allocator's `offset_to_ptr()` method
/// 3. The same offset maps to the same memory location in all processes that map the file
use crate::FreeListAllocator;
use std::alloc::{Allocator, Layout};
use std::fmt;
use std::ops::{Deref, DerefMut, Index, IndexMut};

/// A vector-like container that stores its data in shared memory via offsets.
///
/// Unlike `Vec<T>`, `SharedVec<T>` stores metadata that is offset-based, allowing
/// it to be safely shared across process boundaries.
///
/// # Metadata
/// - `offset: usize` - Location in shared memory (via allocator)
/// - `len: usize` - Number of elements
/// - `capacity: usize` - Allocated space for elements
///
/// # Example
/// ```
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedVec;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
/// vec.push(42);
/// assert_eq!(vec[0], 42);
/// # Ok(())
/// # }
/// ```
pub struct SharedVec<T: Copy> {
    allocator: *const FreeListAllocator,
    offset: usize,
    len: usize,
    capacity: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Copy> SharedVec<T> {
    /// Create a new empty `SharedVec` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedVec {
            allocator: allocator as *const FreeListAllocator,
            offset: 0,
            len: 0,
            capacity: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a `SharedVec` from a standard `Vec<T>`.
    ///
    /// Copies the data into shared memory and forgets the original vec.
    pub fn from_vec(vec: Vec<T>, allocator: &FreeListAllocator) -> Option<Self> {
        if vec.is_empty() {
            return Some(Self::new_in(allocator));
        }

        let len = vec.len();
        let elem_size = std::mem::size_of::<T>();
        let elem_align = std::mem::align_of::<T>();
        let layout = Layout::from_size_align(len * elem_size, elem_align).ok()?;

        let allocated = allocator.allocate(layout).ok()?;
        let offset =
            allocated.as_ptr() as *const u8 as usize - allocator.as_slice().as_ptr() as usize;

        unsafe {
            std::ptr::copy_nonoverlapping(vec.as_ptr(), allocated.as_mut_ptr() as *mut T, len);
        }

        std::mem::forget(vec); // Don't deallocate original vec

        Some(SharedVec {
            allocator: allocator as *const FreeListAllocator,
            offset,
            len,
            capacity: len,
            _marker: std::marker::PhantomData,
        })
    }

    /// Convert to a standard `Vec<T>`.
    ///
    /// Copies data out but leaves the SharedVec empty without deallocating.
    pub fn into_vec(mut self) -> Vec<T> {
        if self.len == 0 {
            return Vec::new();
        }

        let mut vec = Vec::with_capacity(self.len);
        unsafe {
            let ptr =
                (*self.allocator).offset_to_ptr(self.offset, self.len * std::mem::size_of::<T>());
            std::ptr::copy_nonoverlapping(ptr as *const T, vec.as_mut_ptr(), self.len);
            vec.set_len(self.len);
        }

        self.len = 0; // Mark as consumed
        vec
    }

    /// Return the number of elements in the vector.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the allocated capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the offset in shared memory where data starts.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Reserve capacity for at least `additional` more elements.
    pub fn reserve(&mut self, additional: usize) {
        let required = self.len + additional;
        if self.capacity >= required {
            return;
        }

        let new_capacity = std::cmp::max(self.capacity * 2, required);
        self.reallocate(new_capacity);
    }

    /// Reallocate to a new capacity, copying existing data.
    fn reallocate(&mut self, new_capacity: usize) {
        unsafe {
            let allocator = &*self.allocator;
            let elem_size = std::mem::size_of::<T>();
            let elem_align = std::mem::align_of::<T>();

            let new_layout =
                Layout::from_size_align_unchecked(new_capacity * elem_size, elem_align);
            let new_allocated = allocator.allocate(new_layout).expect("allocation failed");
            let new_offset = new_allocated.as_ptr() as *const u8 as usize
                - allocator.as_slice().as_ptr() as usize;

            if self.capacity > 0 && self.len > 0 {
                let old_ptr = allocator.offset_to_ptr(self.offset, self.len * elem_size);
                std::ptr::copy_nonoverlapping(
                    old_ptr as *const T,
                    new_allocated.as_mut_ptr() as *mut T,
                    self.len,
                );

                let old_layout =
                    Layout::from_size_align_unchecked(self.capacity * elem_size, elem_align);
                allocator.deallocate(
                    std::ptr::NonNull::new_unchecked(
                        allocator.offset_to_mut_ptr(self.offset, self.capacity * elem_size),
                    ),
                    old_layout,
                );
            }

            self.offset = new_offset;
            self.capacity = new_capacity;
        }
    }

    /// Push an element to the back of the vector.
    pub fn push(&mut self, value: T) {
        if self.len >= self.capacity {
            self.reserve(1);
        }

        unsafe {
            let ptr = (*self.allocator)
                .offset_to_mut_ptr(self.offset, self.len * std::mem::size_of::<T>());
            (ptr as *mut T).add(self.len).write(value);
        }

        self.len += 1;
    }

    /// Remove the last element and return it, or `None` if empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        unsafe {
            let ptr =
                (*self.allocator).offset_to_ptr(self.offset, self.len * std::mem::size_of::<T>());
            let value = (ptr as *const T).add(self.len - 1).read();
            self.len -= 1;
            Some(value)
        }
    }

    /// Insert an element at position `index` within the vector, shifting all elements after it to the right.
    pub fn insert(&mut self, index: usize, value: T) {
        assert!(index <= self.len, "insertion index out of bounds");

        if self.len >= self.capacity {
            self.reserve(1);
        }

        unsafe {
            let ptr = (*self.allocator)
                .offset_to_mut_ptr(self.offset, self.len * std::mem::size_of::<T>())
                as *mut T;
            std::ptr::copy(ptr.add(index), ptr.add(index + 1), self.len - index);
            ptr.add(index).write(value);
        }

        self.len += 1;
    }

    /// Remove and return the element at position `index` within the vector, shifting all elements after it to the left.
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "removal index out of bounds");

        unsafe {
            let ptr = (*self.allocator)
                .offset_to_mut_ptr(self.offset, self.len * std::mem::size_of::<T>())
                as *mut T;
            let value = ptr.add(index).read();
            std::ptr::copy(ptr.add(index + 1), ptr.add(index), self.len - index - 1);
            self.len -= 1;
            value
        }
    }

    /// Clears the vector, removing all values.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Returns a reference to an element or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            Some(&self[index])
        } else {
            None
        }
    }

    /// Returns a mutable reference to an element or `None` if out of bounds.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            Some(&mut self[index])
        } else {
            None
        }
    }

    /// Returns an iterator over the vector.
    pub fn iter(&self) -> SharedVecIter<'_, T> {
        SharedVecIter {
            data: self.deref(),
            index: 0,
        }
    }

    /// Shrink the capacity of the vector as close to the length as possible.
    pub fn shrink_to_fit(&mut self) {
        if self.capacity > self.len && self.len > 0 {
            self.reallocate(self.len);
        } else if self.len == 0 && self.capacity > 0 {
            unsafe {
                let allocator = &*self.allocator;
                if self.capacity > 0 {
                    let layout = Layout::from_size_align_unchecked(
                        self.capacity * std::mem::size_of::<T>(),
                        std::mem::align_of::<T>(),
                    );
                    allocator.deallocate(
                        std::ptr::NonNull::new_unchecked(allocator.offset_to_mut_ptr(
                            self.offset,
                            self.capacity * std::mem::size_of::<T>(),
                        )),
                        layout,
                    );
                }
            }
            self.offset = 0;
            self.capacity = 0;
        }
    }

    /// Get an immutable pointer to the data in shared memory.
    unsafe fn get_ptr(&self) -> *const T {
        unsafe {
            (*self.allocator).offset_to_ptr(self.offset, self.len * std::mem::size_of::<T>())
                as *const T
        }
    }

    /// Get a mutable pointer to the data in shared memory.
    unsafe fn get_mut_ptr(&self) -> *mut T {
        unsafe {
            (*self.allocator).offset_to_mut_ptr(self.offset, self.len * std::mem::size_of::<T>())
                as *mut T
        }
    }

    /// Appends an element to the back of a collection.
    ///
    /// Alias for `push`.
    pub fn append_one(&mut self, value: T) {
        self.push(value);
    }

    /// Removes and returns the element at the front of the vector, or `None` if empty.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            Some(self.remove(0))
        }
    }

    /// Moves all elements from `other` into `self`, leaving `other` empty.
    pub fn append(&mut self, other: &mut SharedVec<T>) {
        if other.is_empty() {
            return;
        }
        self.reserve(other.len());
        for &elem in other.iter() {
            self.push(elem);
        }
        other.clear();
    }

    /// Extends the vector with the contents of an iterator.
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }

    /// Retains only elements matching the predicate.
    pub fn retain<F: Fn(&T) -> bool>(&mut self, f: F) {
        let mut write_idx = 0;
        for read_idx in 0..self.len {
            if f(&self[read_idx]) {
                if read_idx != write_idx {
                    self[write_idx] = self[read_idx];
                }
                write_idx += 1;
            }
        }
        self.len = write_idx;
    }

    /// Resizes the vector in-place so that `len` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len`, the vector is extended by the difference,
    /// with each additional slot filled with `value`.
    pub fn resize(&mut self, new_len: usize, value: T) {
        match new_len.cmp(&self.len) {
            std::cmp::Ordering::Greater => {
                self.reserve(new_len - self.len);
                for _ in self.len..new_len {
                    self.push(value);
                }
            }
            std::cmp::Ordering::Less => {
                self.len = new_len;
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    /// Removes all but the first of consecutive elements in the vector that satisfy the given equality relation.
    pub fn dedup_by_key<F: Fn(&T) -> K, K: PartialEq>(&mut self, f: F) {
        if self.len <= 1 {
            return;
        }

        let mut write_idx = 1;
        let mut last_key = f(&self[0]);

        for read_idx in 1..self.len {
            let key = f(&self[read_idx]);
            if key != last_key {
                if read_idx != write_idx {
                    self[write_idx] = self[read_idx];
                }
                last_key = key;
                write_idx += 1;
            }
        }

        self.len = write_idx;
    }

    /// Removes consecutive duplicate elements.
    pub fn dedup(&mut self)
    where
        T: PartialEq,
    {
        self.dedup_by_key(|x| *x);
    }

    /// Removes all occurrences of a value.
    pub fn remove_all(&mut self, value: T)
    where
        T: PartialEq,
    {
        self.retain(|&x| x != value);
    }

    /// Returns the number of elements equal to a given value.
    pub fn count(&self, value: T) -> usize
    where
        T: PartialEq,
    {
        self.iter().filter(|&&x| x == value).count()
    }

    /// Fills the entire vector with a given value.
    pub fn fill(&mut self, value: T) {
        for i in 0..self.len {
            self[i] = value;
        }
    }

    /// Fills a range in the vector with a given value.
    pub fn fill_range(&mut self, range: std::ops::Range<usize>, value: T) {
        for i in range {
            if i < self.len {
                self[i] = value;
            }
        }
    }

    /// Performs a reverse sort based on a comparison function.
    pub fn sort_by<F: Fn(&T, &T) -> std::cmp::Ordering>(&mut self, compare: F) {
        self.sort_unstable_by(compare);
    }

    /// Sorts the vector with a key extraction function.
    pub fn sort_by_key<F: Fn(&T) -> K, K: Ord>(&mut self, f: F) {
        let len = self.len;
        if len <= 1 {
            return;
        }

        // Simple bubble sort for now - stable but O(n²)
        for i in 0..len {
            for j in 0..len - i - 1 {
                if f(&self[j]) > f(&self[j + 1]) {
                    self.swap(j, j + 1);
                }
            }
        }
    }

    /// Swaps two elements in the vector.
    pub fn swap(&mut self, a: usize, b: usize) {
        assert!(a < self.len && b < self.len, "swap indices out of bounds");
        if a != b {
            unsafe {
                let ptr = self.get_mut_ptr();
                std::ptr::swap(ptr.add(a), ptr.add(b));
            }
        }
    }

    /// Reverses the elements of the vector in-place.
    pub fn reverse(&mut self) {
        let mut left = 0;
        let mut right = self.len.saturating_sub(1);
        while left < right {
            self.swap(left, right);
            left += 1;
            right -= 1;
        }
    }

    /// Performs an unstable sort.
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.sort_unstable_by(|a, b| a.cmp(b));
    }

    /// Performs an unstable sort with a comparison function.
    pub fn sort_unstable_by<F: Fn(&T, &T) -> std::cmp::Ordering>(&mut self, compare: F) {
        let len = self.len;
        if len <= 1 {
            return;
        }

        // Simple bubble sort for unstable sort
        for i in 0..len {
            for j in 0..len - i - 1 {
                if compare(&self[j], &self[j + 1]) == std::cmp::Ordering::Greater {
                    self.swap(j, j + 1);
                }
            }
        }
    }

    /// Returns a reference to the first element, or `None` if empty.
    pub fn first(&self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            Some(&self[0])
        }
    }

    /// Returns a mutable reference to the first element, or `None` if empty.
    pub fn first_mut(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            None
        } else {
            Some(&mut self[0])
        }
    }

    /// Returns a reference to the last element, or `None` if empty.
    pub fn last(&self) -> Option<&T> {
        if self.is_empty() {
            None
        } else {
            Some(&self[self.len - 1])
        }
    }

    /// Returns a mutable reference to the last element, or `None` if empty.
    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.is_empty() {
            None
        } else {
            let idx = self.len - 1;
            Some(&mut self[idx])
        }
    }

    /// Returns a slice of the vector.
    pub fn slice(&self, range: std::ops::Range<usize>) -> Option<&[T]> {
        if range.start <= range.end && range.end <= self.len {
            Some(&self.deref()[range])
        } else {
            None
        }
    }

    /// Returns a mutable slice of the vector.
    pub fn slice_mut(&mut self, range: std::ops::Range<usize>) -> Option<&mut [T]> {
        if range.start <= range.end && range.end <= self.len {
            Some(&mut self.deref_mut()[range])
        } else {
            None
        }
    }

    /// Returns a reference to the underlying allocator.
    pub fn allocator(&self) -> &FreeListAllocator {
        unsafe { &*self.allocator }
    }

    /// Shrinks the capacity to fit exactly `len` elements.
    pub fn shrink_to(&mut self, min_capacity: usize) {
        if self.capacity > min_capacity.max(self.len) {
            self.reallocate(self.len.max(min_capacity));
        }
    }

    /// Binary search for a value. Returns the index of the value if found, or the index where it should be inserted.
    pub fn binary_search(&self, value: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;
            if self[mid] < *value {
                left = mid + 1;
            } else if self[mid] > *value {
                right = mid;
            } else {
                return Ok(mid);
            }
        }

        Err(left)
    }

    /// Binary search using a comparison function.
    pub fn binary_search_by<F: Fn(&T) -> std::cmp::Ordering>(&self, f: F) -> Result<usize, usize> {
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;
            match f(&self[mid]) {
                std::cmp::Ordering::Less => left = mid + 1,
                std::cmp::Ordering::Greater => right = mid,
                std::cmp::Ordering::Equal => return Ok(mid),
            }
        }

        Err(left)
    }

    /// Binary search using a key extraction function.
    pub fn binary_search_by_key<B: Ord, F: Fn(&T) -> B>(
        &self,
        b: &B,
        f: F,
    ) -> Result<usize, usize> {
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;
            let key = f(&self[mid]);
            if key < *b {
                left = mid + 1;
            } else if key > *b {
                right = mid;
            } else {
                return Ok(mid);
            }
        }

        Err(left)
    }
}

impl<T: Copy> Deref for SharedVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe {
            let ptr = self.get_ptr();
            std::slice::from_raw_parts(ptr, self.len)
        }
    }
}

impl<T: Copy> DerefMut for SharedVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            let ptr = self.get_mut_ptr();
            std::slice::from_raw_parts_mut(ptr, self.len)
        }
    }
}

impl<T: Copy> Index<usize> for SharedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        &self.deref()[index]
    }
}

impl<T: Copy> IndexMut<usize> for SharedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.deref_mut()[index]
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for SharedVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedVec")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .field("capacity", &self.capacity)
            .field("data", &self.deref())
            .finish()
    }
}

impl<T: Copy + PartialEq> PartialEq for SharedVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl<T: Copy + Eq> Eq for SharedVec<T> {}

impl<T: Copy + PartialEq> PartialEq<Vec<T>> for SharedVec<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.deref() == &other[..]
    }
}

impl<T: Copy + PartialEq> PartialEq<[T]> for SharedVec<T> {
    fn eq(&self, other: &[T]) -> bool {
        self.deref() == other
    }
}

impl<T: Copy + Default> Default for SharedVec<T> {
    fn default() -> Self {
        // Note: This requires external allocator, so it returns a zero-capacity vec
        SharedVec {
            allocator: std::ptr::null(),
            offset: 0,
            len: 0,
            capacity: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Copy> From<Vec<T>> for SharedVec<T> {
    fn from(_vec: Vec<T>) -> Self {
        // Can't implement without allocator context
        panic!("Use from_vec(vec, allocator) instead")
    }
}

/// An iterator over a `SharedVec`.
pub struct SharedVecIter<'a, T: Copy> {
    data: &'a [T],
    index: usize,
}

impl<'a, T: Copy> Iterator for SharedVecIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.len() {
            let item = &self.data[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

// ============================================================================

/// A string type that stores UTF-8 data in shared memory via offsets.
///
/// `SharedString` is essentially `SharedVec<u8>` with UTF-8 validation and string methods.
///
/// # Example
/// ```
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedString;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let mut s: SharedString = SharedString::new_in(&allocator);
/// s.push_str("hello");
/// assert_eq!(s.as_str(), "hello");
/// # Ok(())
/// # }
/// ```
pub struct SharedString {
    inner: SharedVec<u8>,
}

impl SharedString {
    /// Create a new empty `SharedString` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedString {
            inner: SharedVec::new_in(allocator),
        }
    }

    /// Create a `SharedString` from a standard `String`.
    pub fn from_string(s: String, allocator: &FreeListAllocator) -> Option<Self> {
        let vec = s.into_bytes();
        let inner = SharedVec::from_vec(vec, allocator)?;
        Some(SharedString { inner })
    }

    /// Convert to a standard `String`.
    ///
    /// Copies data out but leaves the SharedString empty without deallocating.
    pub fn into_string(self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.inner.into_vec())
    }

    /// Return the length in bytes.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Return the string as `&str`.
    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.inner) }
    }

    /// Return the string as `&mut str`.
    pub fn as_mut_str(&mut self) -> &mut str {
        unsafe { std::str::from_utf8_unchecked_mut(&mut self.inner) }
    }

    /// Push a string slice onto this string.
    pub fn push_str(&mut self, s: &str) {
        self.inner.reserve(s.len());
        for &byte in s.as_bytes() {
            self.inner.push(byte);
        }
    }

    /// Push a single character onto this string.
    pub fn push_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let len = c.encode_utf8(&mut buf).len();
        self.inner.reserve(len);
        for &byte in &buf[..len] {
            self.inner.push(byte);
        }
    }

    /// Removes the last character and returns it, or `None` if the string is empty.
    pub fn pop(&mut self) -> Option<char> {
        let c = self.chars().last()?;
        let len = c.len_utf8();
        let new_len = self.len() - len;
        self.inner.len = new_len;
        Some(c)
    }

    /// Returns a reference to a substring or `None` if range is out of bounds.
    pub fn get(&self, range: std::ops::Range<usize>) -> Option<&str> {
        if range.start <= range.end && range.end <= self.len() {
            let bytes = &self.inner.deref()[range];
            unsafe { Some(std::str::from_utf8_unchecked(bytes)) }
        } else {
            None
        }
    }

    /// Clears the string, removing all characters.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns the capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Reserves capacity for at least `additional` more bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);
    }

    /// Returns an iterator over characters.
    pub fn chars(&self) -> std::str::Chars<'_> {
        self.as_str().chars()
    }

    /// Returns `true` if the string starts with the given substring.
    pub fn starts_with(&self, s: &str) -> bool {
        self.as_str().starts_with(s)
    }

    /// Returns `true` if the string ends with the given substring.
    pub fn ends_with(&self, s: &str) -> bool {
        self.as_str().ends_with(s)
    }

    /// Returns an iterator over lines.
    pub fn lines(&self) -> std::str::Lines<'_> {
        self.as_str().lines()
    }

    /// Get the offset in shared memory where data starts.
    pub fn offset(&self) -> usize {
        self.inner.offset()
    }

    /// Returns a byte slice of the entire string.
    pub fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    /// Returns a mutable byte slice of the entire string.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { self.as_mut_str().as_bytes_mut() }
    }

    /// Returns the length in UTF-8 characters.
    pub fn char_count(&self) -> usize {
        self.chars().count()
    }

    /// Truncates the string to the specified length in bytes.
    ///
    /// Panics if the specified position is not on a char boundary.
    pub fn truncate(&mut self, new_len: usize) {
        assert!(
            self.is_char_boundary(new_len),
            "truncate at invalid char boundary"
        );
        self.inner.len = new_len;
    }

    /// Checks if the given position is on a char boundary.
    pub fn is_char_boundary(&self, index: usize) -> bool {
        if index > self.len() {
            return false;
        }
        if index == self.len() {
            return true;
        }
        self.as_bytes()[index] & 0xC0 != 0x80
    }

    /// Returns the byte index of the first character that matches the predicate.
    pub fn find<F: Fn(char) -> bool>(&self, f: F) -> Option<usize> {
        let mut byte_index = 0;
        for ch in self.chars() {
            if f(ch) {
                return Some(byte_index);
            }
            byte_index += ch.len_utf8();
        }
        None
    }

    /// Returns the byte index of the last character that matches the predicate.
    pub fn rfind<F: Fn(char) -> bool>(&self, f: F) -> Option<usize> {
        let mut byte_index = self.len();
        let mut chars = self.chars().rev();
        for ch in &mut chars {
            byte_index -= ch.len_utf8();
            if f(ch) {
                return Some(byte_index);
            }
        }
        None
    }

    /// Returns an iterator over substrings separated by a pattern.
    pub fn split<'a>(&'a self, pattern: &'a str) -> impl Iterator<Item = &'a str> {
        self.as_str().split(pattern)
    }

    /// Returns an iterator over lines (without trailing newlines).
    pub fn split_lines(&self) -> impl Iterator<Item = &str> {
        self.as_str().lines()
    }

    /// Returns an iterator over words separated by whitespace.
    pub fn split_whitespace(&self) -> impl Iterator<Item = &str> {
        self.as_str().split_whitespace()
    }

    /// Returns a string with leading and trailing whitespace removed.
    pub fn trim(&self) -> &str {
        self.as_str().trim()
    }

    /// Returns a string with leading whitespace removed.
    pub fn trim_start(&self) -> &str {
        self.as_str().trim_start()
    }

    /// Returns a string with trailing whitespace removed.
    pub fn trim_end(&self) -> &str {
        self.as_str().trim_end()
    }

    /// Removes the prefix if present and returns the remainder.
    pub fn strip_prefix(&self, prefix: &str) -> Option<&str> {
        self.as_str().strip_prefix(prefix)
    }

    /// Removes the suffix if present and returns the remainder.
    pub fn strip_suffix(&self, suffix: &str) -> Option<&str> {
        self.as_str().strip_suffix(suffix)
    }

    /// Inserts a string at the specified byte position.
    ///
    /// Panics if the position is not on a char boundary.
    pub fn insert_str(&mut self, idx: usize, s: &str) {
        assert!(
            self.is_char_boundary(idx),
            "insert_str at invalid char boundary"
        );

        let bytes = s.as_bytes();
        self.inner.reserve(bytes.len());

        // Shift existing bytes
        unsafe {
            let ptr = self.inner.get_mut_ptr() as *mut u8;
            std::ptr::copy(ptr.add(idx), ptr.add(idx + bytes.len()), self.len() - idx);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(idx), bytes.len());
        }
        self.inner.len += bytes.len();
    }

    /// Removes a substring at the specified byte range.
    ///
    /// Panics if the range is not on char boundaries.
    pub fn remove_range(&mut self, range: std::ops::Range<usize>) {
        assert!(
            self.is_char_boundary(range.start) && self.is_char_boundary(range.end),
            "remove_range at invalid char boundaries"
        );
        assert!(range.end <= self.len(), "remove_range out of bounds");

        let len = range.end - range.start;
        unsafe {
            let ptr = self.inner.get_mut_ptr() as *mut u8;
            std::ptr::copy(
                ptr.add(range.end),
                ptr.add(range.start),
                self.len() - range.end,
            );
        }
        self.inner.len -= len;
    }

    /// Returns true if the string contains the substring.
    pub fn contains_substring(&self, s: &str) -> bool {
        self.as_str().contains(s)
    }

    /// Replaces all occurrences of a substring.
    ///
    /// Note: This allocates a new SharedString with the result.
    pub fn replace(&self, from: &str, to: &str, allocator: &FreeListAllocator) -> Option<Self> {
        let replaced = self.as_str().replace(from, to);
        SharedString::from_string(replaced, allocator)
    }

    /// Repeats the string `n` times.
    ///
    /// Note: This allocates a new SharedString with the result.
    pub fn repeat(&self, n: usize, allocator: &FreeListAllocator) -> Option<Self> {
        let repeated = self.as_str().repeat(n);
        SharedString::from_string(repeated, allocator)
    }

    /// Returns true if the string is all uppercase.
    pub fn is_uppercase(&self) -> bool {
        self.chars().all(|c| !c.is_lowercase())
    }

    /// Returns true if the string is all lowercase.
    pub fn is_lowercase(&self) -> bool {
        self.chars().all(|c| !c.is_uppercase())
    }

    /// Returns true if all characters are whitespace.
    pub fn is_whitespace(&self) -> bool {
        self.chars().all(|c| c.is_whitespace())
    }

    /// Converts the string to lowercase.
    ///
    /// Note: This allocates a new SharedString with the result.
    pub fn to_lowercase(&self, allocator: &FreeListAllocator) -> Option<Self> {
        let lower = self.as_str().to_lowercase();
        SharedString::from_string(lower, allocator)
    }

    /// Converts the string to uppercase.
    ///
    /// Note: This allocates a new SharedString with the result.
    pub fn to_uppercase(&self, allocator: &FreeListAllocator) -> Option<Self> {
        let upper = self.as_str().to_uppercase();
        SharedString::from_string(upper, allocator)
    }
}

impl Deref for SharedString {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl DerefMut for SharedString {
    fn deref_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl fmt::Debug for SharedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedString")
            .field("offset", &self.offset())
            .field("len", &self.len())
            .field("data", &self.as_str())
            .finish()
    }
}

impl fmt::Display for SharedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl PartialEq for SharedString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SharedString {}

impl PartialEq<str> for SharedString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SharedString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for SharedString {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Default for SharedString {
    fn default() -> Self {
        SharedString {
            inner: SharedVec::default(),
        }
    }
}

impl From<String> for SharedString {
    fn from(_s: String) -> Self {
        panic!("Use from_string(s, allocator) instead")
    }
}

// ============================================================================

/// A single-value wrapper that stores data in shared memory via an offset.
///
/// Unlike `SharedVec`, `SharedBox<T>` allocates exactly one element.
///
/// # Example
/// ```
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedBox;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let boxed: SharedBox<i32> = SharedBox::new_in(42, &allocator).unwrap();
/// assert_eq!(*boxed, 42);
/// # Ok(())
/// # }
/// ```
pub struct SharedBox<T: Copy> {
    allocator: *const FreeListAllocator,
    offset: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Copy> SharedBox<T> {
    /// Create a new `SharedBox` containing the given value.
    pub fn new_in(value: T, allocator: &FreeListAllocator) -> Option<Self> {
        let elem_size = std::mem::size_of::<T>();
        let elem_align = std::mem::align_of::<T>();
        let layout = Layout::from_size_align(elem_size, elem_align).ok()?;

        let allocated = allocator.allocate(layout).ok()?;
        let offset =
            allocated.as_ptr() as *const u8 as usize - allocator.as_slice().as_ptr() as usize;

        unsafe {
            (allocated.as_mut_ptr() as *mut T).write(value);
        }

        Some(SharedBox {
            allocator: allocator as *const FreeListAllocator,
            offset,
            _marker: std::marker::PhantomData,
        })
    }

    /// Create a `SharedBox` from a boxed value.
    pub fn from_box(boxed: Box<T>, allocator: &FreeListAllocator) -> Option<Self> {
        Self::new_in(*boxed, allocator)
    }

    /// Convert to a boxed value.
    pub fn into_box(self) -> Box<T> {
        Box::new(*self)
    }

    /// Get the offset in shared memory where data is stored.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get an immutable pointer to the stored value.
    unsafe fn get_ptr(&self) -> *const T {
        unsafe {
            (*self.allocator).offset_to_ptr(self.offset, std::mem::size_of::<T>()) as *const T
        }
    }

    /// Get a mutable pointer to the stored value.
    unsafe fn get_mut_ptr(&self) -> *mut T {
        unsafe {
            (*self.allocator).offset_to_mut_ptr(self.offset, std::mem::size_of::<T>()) as *mut T
        }
    }
}

impl<T: Copy> Deref for SharedBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.get_ptr() }
    }
}

impl<T: Copy> DerefMut for SharedBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.get_mut_ptr() }
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for SharedBox<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedBox")
            .field("offset", &self.offset)
            .field("value", &**self)
            .finish()
    }
}

impl<T: Copy + PartialEq> PartialEq for SharedBox<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: Copy + Eq> Eq for SharedBox<T> {}

impl<T: Copy + PartialEq> PartialEq<T> for SharedBox<T> {
    fn eq(&self, other: &T) -> bool {
        &**self == other
    }
}

impl<T: Copy + Default> Default for SharedBox<T> {
    fn default() -> Self {
        panic!("Use new_in(default_value, allocator) instead")
    }
}

// ============================================================================

/// A hash map that stores key-value pairs in shared memory via offsets.
///
/// `SharedHashMap<K, V>` is backed by a `SharedVec` and uses linear probing for collision resolution.
/// Both keys and values must be `Copy` types for cross-process safety.
///
/// # Example
/// ```
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedHashMap;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let mut map: SharedHashMap<i32, i32> = SharedHashMap::new_in(&allocator);
/// map.insert(1, 42);
/// assert_eq!(map.get(&1), Some(&42));
/// # Ok(())
/// # }
/// ```
pub struct SharedHashMap<K: Copy, V: Copy> {
    entries: SharedVec<Option<(K, V)>>,
}

impl<K: Copy + Eq + std::hash::Hash, V: Copy> SharedHashMap<K, V> {
    /// Create a new empty `SharedHashMap` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedHashMap {
            entries: SharedVec::new_in(allocator),
        }
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears the map, removing all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Inserts a key-value pair into the map and returns the old value if present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // Find existing entry first
        let mut found_index = None;
        for (i, entry) in self.entries.iter().enumerate() {
            if let Some((k, _)) = entry {
                if *k == key {
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(i) = found_index {
            if let Some((_, old_v)) = self.entries[i] {
                self.entries[i] = Some((key, value));
                return Some(old_v);
            }
        }

        self.entries.push(Some((key, value)));
        None
    }

    /// Returns a reference to the value associated with the given key.
    pub fn get(&self, key: &K) -> Option<&V> {
        for entry in self.entries.iter() {
            if let Some((k, v)) = entry {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Returns a mutable reference to the value associated with the given key.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        for i in 0..self.entries.len() {
            if let Some((k, _)) = self.entries[i] {
                if k == *key {
                    if let Some((_, v)) = &mut self.entries[i] {
                        // This is safe because we won't re-borrow
                        return Some(unsafe { &mut *(v as *mut V) });
                    }
                }
            }
        }
        None
    }

    /// Returns `true` if the map contains the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Removes the key-value pair and returns the value if present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let mut index_to_remove = None;
        for (i, entry) in self.entries.iter().enumerate() {
            if let Some((k, _)) = entry {
                if k == key {
                    index_to_remove = Some(i);
                    break;
                }
            }
        }

        if let Some(i) = index_to_remove {
            let old_entry = self.entries.remove(i);
            if let Some((_, v)) = old_entry {
                return Some(v);
            }
        }
        None
    }

    /// Returns an iterator over the entries as `(&K, &V)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries
            .iter()
            .filter_map(|e| e.as_ref().map(|(k, v)| (k, v)))
    }

    /// Returns an iterator over mutable references to values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries
            .iter()
            .filter_map(|e| e.as_ref().map(|(_, v)| v))
    }

    /// Returns an iterator over keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries
            .iter()
            .filter_map(|e| e.as_ref().map(|(k, _)| k))
    }

    /// Reserves capacity for at least `additional` more entries.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Returns the number of allocated slots (may include empty slots).
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Retains only the entries where the predicate returns true.
    pub fn retain<F: Fn(&K, &V) -> bool>(&mut self, f: F) {
        let mut write_idx = 0;
        for read_idx in 0..self.entries.len() {
            if let Some((k, v)) = self.entries[read_idx] {
                if f(&k, &v) {
                    if read_idx != write_idx {
                        self.entries[write_idx] = self.entries[read_idx];
                    }
                    write_idx += 1;
                }
            }
        }
        self.entries.len = write_idx;
    }

    /// Iterates over the map and calls the closure for each entry.
    pub fn for_each<F: Fn(&K, &V)>(&self, f: F) {
        for (k, v) in self.iter() {
            f(k, v);
        }
    }
}

impl<K: Copy + Eq + std::hash::Hash + fmt::Debug, V: Copy + fmt::Debug> fmt::Debug
    for SharedHashMap<K, V>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map()
            .entries(
                self.entries
                    .iter()
                    .filter_map(|e| e.as_ref().map(|(k, v)| (k, v))),
            )
            .finish()
    }
}

impl<K: Copy + Eq + std::hash::Hash, V: Copy> Default for SharedHashMap<K, V> {
    fn default() -> Self {
        panic!("Use new_in(allocator) instead")
    }
}

impl<K: Copy + Eq + std::hash::Hash + PartialEq, V: Copy + PartialEq> PartialEq
    for SharedHashMap<K, V>
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl<K: Copy + Eq + std::hash::Hash + PartialEq, V: Copy + Eq + PartialEq> Eq
    for SharedHashMap<K, V>
{
}

// ============================================================================

/// A hash set that stores unique values in shared memory via offsets.
///
/// `SharedHashSet<T>` is backed by a `SharedVec` and stores values uniquely.
/// Values must be `Copy` types for cross-process safety.
///
/// # Example
/// ```
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedHashSet;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let mut set: SharedHashSet<i32> = SharedHashSet::new_in(&allocator);
/// set.insert(42);
/// assert!(set.contains(&42));
/// # Ok(())
/// # }
/// ```
pub struct SharedHashSet<T: Copy> {
    values: SharedVec<T>,
}

impl<T: Copy + Eq + std::hash::Hash> SharedHashSet<T> {
    /// Create a new empty `SharedHashSet` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedHashSet {
            values: SharedVec::new_in(allocator),
        }
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Clears the set, removing all values.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Inserts a value and returns `true` if it was inserted, `false` if already present.
    pub fn insert(&mut self, value: T) -> bool {
        if self.contains(&value) {
            false
        } else {
            self.values.push(value);
            true
        }
    }

    /// Returns `true` if the set contains the given value.
    pub fn contains(&self, value: &T) -> bool {
        self.values.iter().any(|v| v == value)
    }

    /// Removes a value and returns `true` if it was present.
    pub fn remove(&mut self, value: &T) -> bool {
        let mut index_to_remove = None;
        for (i, v) in self.values.iter().enumerate() {
            if v == value {
                index_to_remove = Some(i);
                break;
            }
        }

        if let Some(i) = index_to_remove {
            self.values.remove(i);
            true
        } else {
            false
        }
    }

    /// Returns an iterator over the set.
    pub fn iter(&self) -> SharedVecIter<'_, T> {
        self.values.iter()
    }

    /// Reserves capacity for at least `additional` more entries.
    pub fn reserve(&mut self, additional: usize) {
        self.values.reserve(additional);
    }

    /// Returns the number of allocated slots.
    pub fn capacity(&self) -> usize {
        self.values.capacity()
    }

    /// Returns true if the set contains any element that satisfies the predicate.
    pub fn any<F: Fn(&T) -> bool>(&self, f: F) -> bool {
        self.values.iter().any(f)
    }

    /// Returns true if all elements satisfy the predicate.
    pub fn all<F: Fn(&T) -> bool>(&self, f: F) -> bool {
        self.values.iter().all(f)
    }

    /// Retains only the values where the predicate returns true.
    pub fn retain<F: Fn(&T) -> bool>(&mut self, f: F) {
        self.values.retain(f);
    }

    /// Iterates over the set and calls the closure for each value.
    pub fn for_each<F: Fn(&T)>(&self, f: F) {
        self.values.iter().for_each(f);
    }

    /// Returns true if this set is a subset of another.
    pub fn is_subset(&self, other: &Self) -> bool {
        self.values.iter().all(|v| other.contains(v))
    }

    /// Returns true if this set is a superset of another.
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Returns true if this set has no elements in common with another.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.values.iter().all(|v| !other.contains(v))
    }
}

impl<T: Copy + Eq + std::hash::Hash + fmt::Debug> fmt::Debug for SharedHashSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_set().entries(self.values.iter()).finish()
    }
}

impl<T: Copy + Eq + std::hash::Hash> Default for SharedHashSet<T> {
    fn default() -> Self {
        panic!("Use new_in(allocator) instead")
    }
}

impl<T: Copy + Eq + std::hash::Hash> PartialEq for SharedHashSet<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.values.iter().all(|v| other.contains(v))
    }
}

impl<T: Copy + Eq + std::hash::Hash> Eq for SharedHashSet<T> {}

// ============================================================================

/// A double-ended queue backed by a SharedVec with circular buffer semantics.
///
/// `SharedVecDeque<T>` provides O(1) amortized insertion and deletion at both ends.
///
/// # Example
/// ```no_run
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedVecDeque;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let mut deque: SharedVecDeque<i32> = SharedVecDeque::new_in(&allocator);
/// deque.push_back(1);
/// deque.push_back(2);
/// assert_eq!(deque.pop_front(), Some(1));
/// # Ok(())
/// # }
/// ```
pub struct SharedVecDeque<T: Copy> {
    data: SharedVec<T>,
    front: usize,
    len: usize,
}

impl<T: Copy> SharedVecDeque<T> {
    /// Create a new empty `SharedVecDeque` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedVecDeque {
            data: SharedVec::new_in(allocator),
            front: 0,
            len: 0,
        }
    }

    /// Returns the number of elements in the deque.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the deque is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the underlying buffer.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Pushes an element to the back of the deque.
    pub fn push_back(&mut self, value: T) {
        if self.len >= self.data.capacity() {
            self.reallocate();
        }
        // Always push to extend the underlying vector
        self.data.push(value);
        self.len += 1;
    }

    /// Pushes an element to the front of the deque.
    pub fn push_front(&mut self, value: T) {
        if self.len >= self.data.capacity() {
            self.reallocate();
        }
        if self.front == 0 {
            self.front = self.data.capacity().max(1) - 1;
        } else {
            self.front -= 1;
        }
        // Insert or push to ensure element exists
        if self.front < self.data.len() {
            self.data[self.front] = value;
        } else {
            self.data.push(value);
        }
        self.len += 1;
    }

    /// Removes and returns the element at the back of the deque.
    pub fn pop_back(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let idx = (self.front + self.len - 1) % self.data.capacity().max(1);
        let value = self.data[idx];
        self.len -= 1;
        Some(value)
    }

    /// Removes and returns the element at the front of the deque.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let value = self.data[self.front];
        self.front = (self.front + 1) % self.data.capacity().max(1);
        self.len -= 1;
        Some(value)
    }

    /// Returns a reference to the front element, or `None` if empty.
    pub fn front(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            Some(&self.data[self.front])
        }
    }

    /// Returns a reference to the back element, or `None` if empty.
    pub fn back(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            let idx = (self.front + self.len - 1) % self.data.capacity().max(1);
            Some(&self.data[idx])
        }
    }

    /// Clears the deque, removing all elements.
    pub fn clear(&mut self) {
        self.front = 0;
        self.len = 0;
    }

    /// Reallocates the internal buffer with circular buffer logic.
    fn reallocate(&mut self) {
        if self.data.capacity() == 0 {
            self.data.reserve(1);
            // Fill the reserved space with default values
            // We need actual elements to use indexing
            if self.data.len() == 0 {
                return; // Handle this in push methods
            }
        } else {
            let new_capacity = self.data.capacity() * 2;
            let mut new_data = SharedVec::new_in(self.data.allocator());
            new_data.reserve(new_capacity);

            // Copy elements in order
            for i in 0..self.len {
                let idx = (self.front + i) % self.data.capacity();
                new_data.push(self.data[idx]);
            }

            self.data = new_data;
            self.front = 0;
        }
    }

    /// Returns an iterator over the deque.
    pub fn iter(&self) -> SharedVecDequeIter<'_, T> {
        SharedVecDequeIter {
            deque: self,
            index: 0,
        }
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for SharedVecDeque<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedVecDeque")
            .field("front", &self.front)
            .field("len", &self.len)
            .field("capacity", &self.capacity())
            .field("data", &self.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl<T: Copy + PartialEq> PartialEq for SharedVecDeque<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl<T: Copy + Eq> Eq for SharedVecDeque<T> {}

impl<T: Copy> Default for SharedVecDeque<T> {
    fn default() -> Self {
        panic!("Use new_in(allocator) instead")
    }
}

/// Iterator for SharedVecDeque.
pub struct SharedVecDequeIter<'a, T: Copy> {
    deque: &'a SharedVecDeque<T>,
    index: usize,
}

impl<'a, T: Copy> Iterator for SharedVecDequeIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.deque.len {
            let idx = (self.deque.front + self.index) % self.deque.data.capacity().max(1);
            self.index += 1;
            Some(&self.deque.data[idx])
        } else {
            None
        }
    }
}

// ============================================================================

/// A tree-based map backed by sorted keys for ordered iteration.
///
/// Note: This is a simplified implementation using a sorted SharedVec.
/// For production use, consider a proper B-tree implementation.
pub struct SharedBTreeMap<K: Copy + Ord, V: Copy> {
    entries: SharedVec<(K, V)>,
}

impl<K: Copy + Ord, V: Copy> SharedBTreeMap<K, V> {
    /// Create a new empty `SharedBTreeMap` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedBTreeMap {
            entries: SharedVec::new_in(allocator),
        }
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears the map, removing all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Inserts a key-value pair and returns the old value if present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // Find position using binary search
        match self.entries.binary_search_by_key(&key, |&(k, _)| k) {
            Ok(idx) => {
                let old_value = self.entries[idx].1;
                self.entries[idx] = (key, value);
                Some(old_value)
            }
            Err(idx) => {
                self.entries.insert(idx, (key, value));
                None
            }
        }
    }

    /// Returns a reference to the value for the given key.
    pub fn get(&self, key: &K) -> Option<&V> {
        match self.entries.binary_search_by_key(key, |&(k, _)| k) {
            Ok(idx) => Some(&self.entries[idx].1),
            Err(_) => None,
        }
    }

    /// Returns a mutable reference to the value for the given key.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.entries.binary_search_by_key(key, |&(k, _)| k) {
            Ok(idx) => Some(unsafe { &mut *((&mut self.entries[idx].1) as *mut V) }),
            Err(_) => None,
        }
    }

    /// Returns `true` if the map contains the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.binary_search_by_key(key, |&(k, _)| k).is_ok()
    }

    /// Removes a key-value pair and returns the value.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        match self.entries.binary_search_by_key(key, |&(k, _)| k) {
            Ok(idx) => {
                let (_, v) = self.entries.remove(idx);
                Some(v)
            }
            Err(_) => None,
        }
    }

    /// Returns an iterator over the entries in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// Returns an iterator over the keys in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over the values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }

    /// Returns the first key-value pair.
    pub fn first(&self) -> Option<(&K, &V)> {
        self.entries.first().map(|(k, v)| (k, v))
    }

    /// Returns the last key-value pair.
    pub fn last(&self) -> Option<(&K, &V)> {
        self.entries.last().map(|(k, v)| (k, v))
    }

    /// Returns the first key.
    pub fn first_key(&self) -> Option<&K> {
        self.entries.first().map(|(k, _)| k)
    }

    /// Returns the last key.
    pub fn last_key(&self) -> Option<&K> {
        self.entries.last().map(|(k, _)| k)
    }

    /// Retains only entries where the predicate returns true.
    pub fn retain<F: Fn(&K, &V) -> bool>(&mut self, f: F) {
        let mut write_idx = 0;
        for read_idx in 0..self.entries.len() {
            let (k, v) = self.entries[read_idx];
            if f(&k, &v) {
                if read_idx != write_idx {
                    self.entries[write_idx] = self.entries[read_idx];
                }
                write_idx += 1;
            }
        }
        self.entries.len = write_idx;
    }
}

impl<K: Copy + Ord + fmt::Debug, V: Copy + fmt::Debug> fmt::Debug for SharedBTreeMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Copy + Ord, V: Copy> Default for SharedBTreeMap<K, V> {
    fn default() -> Self {
        panic!("Use new_in(allocator) instead")
    }
}

impl<K: Copy + Ord + PartialEq, V: Copy + PartialEq> PartialEq for SharedBTreeMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .all(|(k, v)| other.get(k).map_or(false, |ov| v == ov))
    }
}

impl<K: Copy + Ord + PartialEq, V: Copy + Eq + PartialEq> Eq for SharedBTreeMap<K, V> {}

// ============================================================================

/// A tree-based set backed by sorted values for ordered iteration.
///
/// Note: This is a simplified implementation using a sorted SharedVec.
pub struct SharedBTreeSet<T: Copy + Ord> {
    values: SharedVec<T>,
}

impl<T: Copy + Ord> SharedBTreeSet<T> {
    /// Create a new empty `SharedBTreeSet` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedBTreeSet {
            values: SharedVec::new_in(allocator),
        }
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Clears the set, removing all values.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Inserts a value and returns `true` if it was inserted.
    pub fn insert(&mut self, value: T) -> bool {
        match self.values.binary_search(&value) {
            Ok(_) => false, // Already exists
            Err(idx) => {
                self.values.insert(idx, value);
                true
            }
        }
    }

    /// Returns `true` if the set contains the given value.
    pub fn contains(&self, value: &T) -> bool {
        self.values.binary_search(value).is_ok()
    }

    /// Removes a value and returns `true` if it was present.
    pub fn remove(&mut self, value: &T) -> bool {
        match self.values.binary_search(value) {
            Ok(idx) => {
                self.values.remove(idx);
                true
            }
            Err(_) => false,
        }
    }

    /// Returns an iterator over the values in sorted order.
    pub fn iter(&self) -> SharedVecIter<'_, T> {
        self.values.iter()
    }

    /// Returns the first value.
    pub fn first(&self) -> Option<&T> {
        self.values.first()
    }

    /// Returns the last value.
    pub fn last(&self) -> Option<&T> {
        self.values.last()
    }

    /// Returns true if the set is a subset of another.
    pub fn is_subset(&self, other: &Self) -> bool {
        self.values.iter().all(|v| other.contains(v))
    }

    /// Returns true if the set is a superset of another.
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Returns true if the sets have no elements in common.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.values.iter().all(|v| !other.contains(v))
    }

    /// Retains only values where the predicate returns true.
    pub fn retain<F: Fn(&T) -> bool>(&mut self, f: F) {
        self.values.retain(f);
    }
}

impl<T: Copy + Ord + fmt::Debug> fmt::Debug for SharedBTreeSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_set().entries(self.values.iter()).finish()
    }
}

impl<T: Copy + Ord> Default for SharedBTreeSet<T> {
    fn default() -> Self {
        panic!("Use new_in(allocator) instead")
    }
}

impl<T: Copy + Ord> PartialEq for SharedBTreeSet<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.values.iter().all(|v| other.contains(v))
    }
}

impl<T: Copy + Ord> Eq for SharedBTreeSet<T> {}

// ============================================================================

/// A simple linked list backed by SharedVec for node storage.
///
/// Note: This is a simplified implementation. For production use, consider
/// more sophisticated data structures. Iteration is O(n).
///
/// # Example
/// ```
/// use commy::allocator::FreeListAllocator;
/// use commy::containers::SharedLinkedList;
/// use memmap2::MmapMut;
/// use std::fs::OpenOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = tempfile::NamedTempFile::new()?;
/// let path = file.path().to_owned();
/// let file = OpenOptions::new().read(true).write(true).open(&path)?;
/// file.set_len(1024 * 1024)?;
/// let mmap = unsafe { MmapMut::map_mut(&file)? };
/// let allocator = FreeListAllocator::new(mmap, &path);
/// let mut list: SharedLinkedList<i32> = SharedLinkedList::new_in(&allocator);
/// list.push_back(1);
/// list.push_back(2);
/// assert_eq!(list.pop_front(), Some(1));
/// # Ok(())
/// # }
/// ```
pub struct SharedLinkedList<T: Copy> {
    nodes: SharedVec<LinkedNode<T>>,
    head: Option<usize>,
    tail: Option<usize>,
    len: usize,
}

#[derive(Copy, Clone)]
struct LinkedNode<T: Copy> {
    value: T,
    next: Option<usize>,
}

impl<T: Copy> SharedLinkedList<T> {
    /// Create a new empty `SharedLinkedList` using the given allocator.
    pub fn new_in(allocator: &FreeListAllocator) -> Self {
        SharedLinkedList {
            nodes: SharedVec::new_in(allocator),
            head: None,
            tail: None,
            len: 0,
        }
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Pushes an element to the back of the list.
    pub fn push_back(&mut self, value: T) {
        let node_idx = self.nodes.len();
        self.nodes.push(LinkedNode { value, next: None });

        match self.tail {
            None => {
                self.head = Some(node_idx);
                self.tail = Some(node_idx);
            }
            Some(tail_idx) => {
                self.nodes[tail_idx].next = Some(node_idx);
                self.tail = Some(node_idx);
            }
        }

        self.len += 1;
    }

    /// Pushes an element to the front of the list.
    pub fn push_front(&mut self, value: T) {
        let node_idx = self.nodes.len();
        let old_head = self.head;

        self.nodes.push(LinkedNode {
            value,
            next: old_head,
        });

        self.head = Some(node_idx);
        if self.tail.is_none() {
            self.tail = Some(node_idx);
        }

        self.len += 1;
    }

    /// Removes and returns the element at the front of the list.
    pub fn pop_front(&mut self) -> Option<T> {
        self.head.and_then(|head_idx| {
            let node = self.nodes[head_idx];
            self.head = node.next;

            if self.head.is_none() {
                self.tail = None;
            }

            self.len -= 1;
            Some(node.value)
        })
    }

    /// Returns a reference to the front element, or `None` if empty.
    pub fn front(&self) -> Option<&T> {
        self.head.and_then(|idx| Some(&self.nodes[idx].value))
    }

    /// Clears the list, removing all elements.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.head = None;
        self.tail = None;
        self.len = 0;
    }

    /// Returns an iterator over the list.
    pub fn iter(&self) -> SharedLinkedListIter<'_, T> {
        SharedLinkedListIter {
            list: self,
            current: self.head,
        }
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for SharedLinkedList<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SharedLinkedList")
            .field("len", &self.len)
            .field("data", &self.iter().collect::<Vec<_>>())
            .finish()
    }
}

/// Iterator for SharedLinkedList.
pub struct SharedLinkedListIter<'a, T: Copy> {
    list: &'a SharedLinkedList<T>,
    current: Option<usize>,
}

impl<'a, T: Copy> Iterator for SharedLinkedListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.and_then(|idx| {
            let node = self.list.nodes[idx];
            self.current = node.next;
            Some(&self.list.nodes[idx].value)
        })
    }
}

impl<T: Copy + PartialEq> PartialEq for SharedLinkedList<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl<T: Copy + Eq> Eq for SharedLinkedList<T> {}

impl<T: Copy> Default for SharedLinkedList<T> {
    fn default() -> Self {
        panic!("Use new_in(allocator) instead")
    }
}
