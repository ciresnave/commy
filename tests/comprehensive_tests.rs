#![feature(allocator_api)]
#![feature(slice_ptr_get)]

use std::alloc::Allocator;
use std::fs;

#[path = "../src/allocator.rs"]
mod allocator;

#[path = "../src/containers.rs"]
mod containers;

use allocator::FreeListAllocator;
use containers::{
    SharedBTreeMap, SharedBTreeSet, SharedBox, SharedHashMap, SharedHashSet, SharedString,
    SharedVec, SharedVecDeque,
};

fn setup_test_allocator() -> (String, FreeListAllocator) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let test_dir = std::env::temp_dir();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let test_file = test_dir.join(format!(
        "test_comprehensive_{}_{}.mmap",
        std::process::id(),
        counter
    ));
    let test_file_str = test_file.to_string_lossy().to_string();

    let _ = fs::remove_file(&test_file_str);
    fs::write(&test_file_str, vec![0u8; 10 * 1024 * 1024]).expect("Failed to create test file");

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&test_file_str)
        .expect("Failed to open test file");

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file).expect("Failed to mmap") };
    let allocator = FreeListAllocator::new(mmap, &test_file_str);

    (test_file_str, allocator)
}

// ============================================================================
// FreeListAllocator Tests
// ============================================================================

#[test]
fn test_allocator_basic_allocation() {
    let (test_file, allocator) = setup_test_allocator();

    let layout = std::alloc::Layout::new::<[u64; 16]>();
    let result = allocator.allocate(layout);
    assert!(result.is_ok(), "Basic allocation should succeed");

    if let Ok(ptr) = result {
        unsafe {
            allocator.deallocate(ptr.as_non_null_ptr(), layout);
        }
    }

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_allocator_multiple_allocations() {
    let (test_file, allocator) = setup_test_allocator();

    let layout = std::alloc::Layout::new::<u64>();
    let mut ptrs = vec![];

    for _ in 0..100 {
        match allocator.allocate(layout) {
            Ok(ptr) => ptrs.push(ptr),
            Err(_) => panic!("Allocation failed"),
        }
    }

    assert_eq!(ptrs.len(), 100, "Should have 100 allocations");

    for ptr in ptrs {
        unsafe {
            allocator.deallocate(ptr.as_non_null_ptr(), layout);
        }
    }

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_allocator_size_and_limit() {
    let (test_file, allocator) = setup_test_allocator();

    let size = allocator.size();
    assert!(size > 0, "Allocator should have non-zero size");

    let limit = allocator.allocation_limit();
    assert_eq!(limit, size, "Allocation limit should equal size initially");

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_allocator_offset_to_ptr() {
    let (test_file, allocator) = setup_test_allocator();

    let layout = std::alloc::Layout::new::<u64>();
    let ptr = allocator.allocate(layout).expect("Allocation failed");
    let offset = ptr.as_ptr() as *const u8 as usize - allocator.as_slice().as_ptr() as usize;

    let reconstructed = allocator.offset_to_ptr(offset, 8);
    assert_eq!(
        ptr.as_ptr() as *const u8,
        reconstructed,
        "Pointer reconstruction should match"
    );

    unsafe {
        allocator.deallocate(ptr.as_non_null_ptr(), layout);
    }

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedVec Tests
// ============================================================================

#[test]
fn test_shared_vec_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
    assert_eq!(vec.len(), 0, "Empty vec should have len 0");
    assert!(vec.is_empty(), "Empty vec should report is_empty true");

    vec.push(42);
    assert_eq!(vec.len(), 1, "Vec should have len 1");
    assert_eq!(vec[0], 42, "Element should be 42");

    let popped = vec.pop();
    assert_eq!(popped, Some(42), "Popped element should be 42");
    assert!(vec.is_empty(), "Vec should be empty after pop");

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_vec_operations() {
    let (test_file, allocator) = setup_test_allocator();

    let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);

    // Test multiple pushes
    for i in 0..10 {
        vec.push(i);
    }
    assert_eq!(vec.len(), 10, "Vec should have 10 elements");

    // Test access
    assert_eq!(vec[0], 0, "First element should be 0");
    assert_eq!(vec[9], 9, "Last element should be 9");

    // Test insert
    vec.insert(5, 99);
    assert_eq!(vec.len(), 11, "Vec should have 11 elements after insert");
    assert_eq!(vec[5], 99, "Inserted element should be at correct position");

    // Test remove
    let removed = vec.remove(5);
    assert_eq!(removed, 99, "Removed element should be 99");
    assert_eq!(vec.len(), 10, "Vec should have 10 elements after remove");

    // Test clear
    vec.clear();
    assert!(vec.is_empty(), "Vec should be empty after clear");

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_vec_reserve_and_capacity() {
    let (test_file, allocator) = setup_test_allocator();

    let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
    assert_eq!(vec.capacity(), 0, "New vec should have 0 capacity");

    vec.reserve(50);
    assert!(
        vec.capacity() >= 50,
        "Capacity should be at least 50 after reserve"
    );

    for i in 0..100 {
        vec.push(i);
    }

    vec.shrink_to_fit();
    assert_eq!(
        vec.capacity(),
        vec.len(),
        "Capacity should equal length after shrink_to_fit"
    );

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_vec_from_and_into() {
    let (test_file, allocator) = setup_test_allocator();

    let original = vec![1, 2, 3, 4, 5];
    let shared = SharedVec::from_vec(original, &allocator).expect("from_vec failed");

    assert_eq!(shared.len(), 5, "SharedVec should have 5 elements");
    assert_eq!(shared[0], 1, "First element should be 1");
    assert_eq!(shared[4], 5, "Last element should be 5");

    let back_to_vec = shared.into_vec();
    assert_eq!(
        back_to_vec,
        vec![1, 2, 3, 4, 5],
        "Converted vec should match original"
    );

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_vec_iterator() {
    let (test_file, allocator) = setup_test_allocator();

    let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
    for i in 0..10 {
        vec.push(i);
    }

    let sum: i32 = vec.iter().sum();
    assert_eq!(sum, 45, "Sum of 0..10 should be 45");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedString Tests
// ============================================================================

#[test]
fn test_shared_string_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut s: SharedString = SharedString::new_in(&allocator);
    assert!(s.is_empty(), "New string should be empty");

    s.push_str("hello");
    assert_eq!(s.as_str(), "hello", "String should contain 'hello'");

    s.push_char('!');
    assert_eq!(s.as_str(), "hello!", "String should contain 'hello!'");

    let popped = s.pop();
    assert_eq!(popped, Some('!'), "Popped char should be '!'");
    assert_eq!(
        s.as_str(),
        "hello",
        "String should contain 'hello' after pop"
    );

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_string_operations() {
    let (test_file, allocator) = setup_test_allocator();

    let mut s: SharedString = SharedString::new_in(&allocator);
    s.push_str("hello world");

    assert!(s.starts_with("hello"), "Should start with 'hello'");
    assert!(s.ends_with("world"), "Should end with 'world'");
    assert!(s.contains_substring("lo wo"), "Should contain 'lo wo'");

    let chars: Vec<char> = s.chars().collect();
    assert_eq!(chars.len(), 11, "Should have 11 characters");

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_string_from_and_into() {
    let (test_file, allocator) = setup_test_allocator();

    let original = String::from("hello");
    let shared = SharedString::from_string(original, &allocator).expect("from_string failed");

    assert_eq!(
        shared.as_str(),
        "hello",
        "SharedString should contain 'hello'"
    );

    let back_to_string = shared.into_string().expect("into_string failed");
    assert_eq!(
        back_to_string, "hello",
        "Converted string should match original"
    );

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedBox Tests
// ============================================================================

#[test]
fn test_shared_box_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let boxed = SharedBox::new_in(42i32, &allocator).expect("new_in failed");
    assert_eq!(*boxed, 42, "Boxed value should be 42");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedHashMap Tests
// ============================================================================

#[test]
fn test_shared_hash_map_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut map: SharedHashMap<i32, i32> = SharedHashMap::new_in(&allocator);
    assert!(map.is_empty(), "New map should be empty");

    map.insert(1, 10);
    map.insert(2, 20);

    assert_eq!(map.len(), 2, "Map should have 2 entries");
    assert_eq!(map.get(&1), Some(&10), "Should find key 1");
    assert_eq!(map.get(&2), Some(&20), "Should find key 2");
    assert_eq!(map.get(&3), None, "Should not find key 3");

    assert!(map.contains_key(&1), "Should contain key 1");
    assert!(!map.contains_key(&3), "Should not contain key 3");

    // Test remove
    map.remove(&1);
    assert!(!map.contains_key(&1), "Key 1 should be removed");
    assert_eq!(map.len(), 1, "Map should have 1 entry");

    map.clear();
    assert!(map.is_empty(), "Map should be empty after clear");

    let _ = fs::remove_file(test_file);
}

#[test]
fn test_shared_hash_map_iteration() {
    let (test_file, allocator) = setup_test_allocator();

    let mut map: SharedHashMap<i32, i32> = SharedHashMap::new_in(&allocator);
    map.insert(1, 10);
    map.insert(2, 20);
    map.insert(3, 30);

    let sum: i32 = map.iter().map(|(_, v)| v).sum();
    assert_eq!(sum, 60, "Sum of values should be 60");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedHashSet Tests
// ============================================================================

#[test]
fn test_shared_hash_set_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut set: SharedHashSet<i32> = SharedHashSet::new_in(&allocator);
    assert!(set.is_empty(), "New set should be empty");

    assert!(set.insert(1), "Insert should return true for new value");
    assert!(!set.insert(1), "Insert should return false for duplicate");
    assert!(set.contains(&1), "Set should contain 1");

    assert!(set.remove(&1), "Remove should return true");
    assert!(!set.contains(&1), "Set should not contain 1 after remove");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedBTreeMap Tests
// ============================================================================

#[test]
fn test_shared_btree_map_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut map: SharedBTreeMap<i32, i32> = SharedBTreeMap::new_in(&allocator);
    assert!(map.is_empty(), "New map should be empty");

    map.insert(3, 30);
    map.insert(1, 10);
    map.insert(2, 20);

    assert_eq!(map.len(), 3, "Map should have 3 entries");
    assert_eq!(map.get(&2), Some(&20), "Should find key 2");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedBTreeSet Tests
// ============================================================================

#[test]
fn test_shared_btree_set_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut set: SharedBTreeSet<i32> = SharedBTreeSet::new_in(&allocator);
    assert!(set.is_empty(), "New set should be empty");

    assert!(set.insert(3), "Insert 3 should succeed");
    assert!(set.insert(1), "Insert 1 should succeed");
    assert!(set.insert(2), "Insert 2 should succeed");

    assert_eq!(set.len(), 3, "Set should have 3 elements");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// SharedVecDeque Tests
// ============================================================================

#[test]
fn test_shared_vec_deque_basic() {
    let (test_file, allocator) = setup_test_allocator();

    let mut deque: SharedVecDeque<i32> = SharedVecDeque::new_in(&allocator);
    assert!(deque.is_empty(), "New deque should be empty");

    deque.push_back(1);
    deque.push_back(2);

    assert_eq!(deque.len(), 2, "Deque should have 2 elements");
    assert_eq!(deque.front(), Some(&1), "Front element should be 1");
    assert_eq!(deque.back(), Some(&2), "Back element should be 2");

    assert_eq!(deque.pop_front(), Some(1), "First element should be 1");
    assert_eq!(deque.len(), 1, "Deque should have 1 element after pop");
    assert_eq!(deque.pop_front(), Some(2), "Second element should be 2");
    assert!(deque.is_empty(), "Deque should be empty after popping all");

    let _ = fs::remove_file(test_file);
}

// ============================================================================
// Multi-container stress test
// ============================================================================

#[test]
fn test_all_containers_stress() {
    let (test_file, allocator) = setup_test_allocator();

    // SharedVec
    let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
    for i in 0..50 {
        vec.push(i);
    }

    // SharedString
    let mut string: SharedString = SharedString::new_in(&allocator);
    string.push_str("test string");

    // SharedHashMap
    let mut map: SharedHashMap<i32, i32> = SharedHashMap::new_in(&allocator);
    for i in 0..20 {
        map.insert(i, i * 10);
    }

    // SharedHashSet
    let mut set: SharedHashSet<i32> = SharedHashSet::new_in(&allocator);
    for i in 0..15 {
        set.insert(i);
    }

    assert_eq!(vec.len(), 50, "Vec should have 50 elements");
    assert_eq!(string.len(), 11, "String should have 11 characters");
    assert_eq!(map.len(), 20, "Map should have 20 entries");
    assert_eq!(set.len(), 15, "Set should have 15 elements");

    let _ = fs::remove_file(test_file);
}
