# Commy: User Guide

Complete API reference and usage guide for the Commy shared memory library.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Core Concepts](#core-concepts)
3. [API Reference](#api-reference)
4. [Examples](#examples)
5. [Best Practices](#best-practices)
6. [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
commy = { path = "path/to/commy" }
memmap2 = "0.7"
```

### Prerequisites

- Windows OS (tested on Windows 10+)
- Rust 1.70+
- File system with adequate space for shared memory region

### Minimal Example

```rust
use std::fs;
use commy::{FreeListAllocator, SharedVec};

fn main() -> Result<()> {
    // 1. Create or open shared memory file
    let file_path = "shared.mmap";
    fs::write(file_path, vec![0u8; 10 * 1024 * 1024])?; // 10MB file
    
    // 2. Open file and create memory map
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(file_path)?;
    
    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    
    // 3. Initialize allocator
    let allocator = FreeListAllocator::new(mmap, file_path);
    
    // 4. Create shared container
    let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
    
    // 5. Use like normal Rust collections
    vec.push(1);
    vec.push(2);
    vec.push(3);
    
    assert_eq!(vec.len(), 3);
    assert_eq!(vec.get(0), Some(&1));
    
    // Data persists in file for other processes to access
    Ok(())
}
```

## Core Concepts

### Shared Memory Files

All data is stored in a memory-mapped file on disk:

```
File Structure:
┌─────────────────────────┐
│  MmapHeader (4KB)       │  ← Metadata and coordination
├─────────────────────────┤
│  Allocation Space       │  ← Your containers' data
│  (managed by allocator) │
└─────────────────────────┘
```

### Offset-Based Pointers

Commy uses offsets instead of raw pointers to enable cross-process access:

```rust
// Process A stores offset 4096 for a SharedVec
// Process B reads offset 4096 and calculates its own memory address
// Both processes access the same data!
```

### Multi-Process Coordination

The library automatically handles:

- **Heartbeat Mechanism**: Detects when processes die
- **Operation Timeouts**: Prevents hanging on failed operations
- **Automatic Cleanup**: Handles orphaned operations gracefully

## API Reference

### FreeListAllocator

The core memory allocator.

#### Methods

```rust
impl FreeListAllocator {
    /// Create a new allocator from a memory-mapped file
    pub fn new(mmap: MmapMut, file_path: &str) -> Self
    
    /// Allocate memory for a specific layout
    pub fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>>
    
    /// Deallocate previously allocated memory
    pub fn deallocate(&self, ptr: NonNull<u8>, layout: Layout)
    
    /// Get current allocated memory size
    pub fn size(&self) -> u64
    
    /// Get maximum allocation limit
    pub fn limit(&self) -> u64
    
    /// Resize the backing file
    pub fn resize_file(&self, new_size: u64) -> Result<()>
    
    /// Shrink file to minimal needed size
    pub fn shrink_to_usage(&self) -> Result<()>
    
    /// Update heartbeat timestamp for this process
    pub fn update_heartbeat(&mut self) -> Result<()>
    
    /// Check if a resize operation timed out
    pub fn check_operation_timeout() -> bool
    
    /// Cleanup after failed resize operation
    pub fn cleanup_failed_resize(&mut self) -> Result<()>
}
```

### SharedVec<T>

Dynamic array container.

#### Creation

```rust
// Create new
let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);

// From existing Vec
let rust_vec = vec![1, 2, 3];
let mut shared = SharedVec::from_iter_in(rust_vec.into_iter(), &allocator);
```

#### Common Methods

```rust
// Capacity and length
vec.len() -> usize
vec.capacity() -> usize
vec.is_empty() -> bool

// Access
vec.get(index) -> Option<&T>
vec.get_mut(index) -> Option<&mut T>
vec[index] -> T  // Panics if out of bounds

// Modification
vec.push(value)
vec.pop() -> Option<T>
vec.clear()

// Reserve and shrink
vec.reserve(additional) -> Result<()>
vec.shrink_to_fit() -> Result<()>

// Iteration
for item in &vec { println!("{:?}", item); }

// Conversion
let rust_vec: Vec<T> = vec.into_inner()
```

#### Reallocation Strategy

- Starts with capacity 0
- Grows by 1.5x when full
- May fail if insufficient file space

```rust
// Example: will trigger reallocation
let mut vec: SharedVec<i32> = SharedVec::new_in(&allocator);
vec.push(1);  // Allocates capacity 1
vec.push(2);  // Reallocates to capacity 2 (1.5x)
vec.push(3);  // Reallocates to capacity 3 (1.5x)
vec.push(4);  // Reallocates to capacity 5 (1.5x)
```

### SharedString

String container (optimized Vec<u8>).

#### Creation

```rust
// Create new
let mut string: SharedString = SharedString::new_in(&allocator);

// From &str
let mut string = SharedString::from_str_in("Hello, World!", &allocator)?;
```

#### Common Methods

```rust
// Basic operations
string.len() -> usize
string.is_empty() -> bool
string.clear()

// String operations
string.push_str("more text") -> Result<()>
string.from_utf8() -> Result<String>  // Convert to Rust String

// Conversion
let rust_string: String = string.into_inner()?
```

### SharedBox<T>

Single value wrapper.

#### Creation

```rust
let mut box_val: SharedBox<i32> = SharedBox::new_in(&allocator);
*box_val = 42;  // Deref and assign
```

#### Common Methods

```rust
box_val.deref() -> &T
box_val.deref_mut() -> &mut T
*box_val  // Deref sugar
```

### SharedHashMap<K, V>

Hash-based key-value map.

#### Creation

```rust
let mut map: SharedHashMap<i32, String> = SharedHashMap::new_in(&allocator);
```

#### Common Methods

```rust
// Insert/retrieve
map.insert(key, value)
map.get(&key) -> Option<&V>
map.get_mut(&key) -> Option<&mut V>

// Check/remove
map.contains_key(&key) -> bool
map.remove(&key) -> Option<V>

// Properties
map.len() -> usize
map.is_empty() -> bool
map.clear()

// Iteration
for (k, v) in &map { }
```

### SharedHashSet<T>

Hash-based set.

#### Creation

```rust
let mut set: SharedHashSet<i32> = SharedHashSet::new_in(&allocator);
```

#### Common Methods

```rust
// Insert/check
set.insert(value) -> bool
set.contains(&value) -> bool
set.remove(&value) -> bool

// Properties
set.len() -> usize
set.is_empty() -> bool
set.clear()

// Iteration
for item in &set { }
```

### SharedBTreeMap<K, V>

Ordered key-value map.

#### Creation

```rust
let mut map: SharedBTreeMap<i32, String> = SharedBTreeMap::new_in(&allocator);
```

#### Common Methods

```rust
// Same as HashMap, plus:
map.iter() -> ordered iterator
map.keys() -> ordered key iterator
map.values() -> ordered value iterator
```

### SharedBTreeSet<T>

Ordered set.

#### Creation

```rust
let mut set: SharedBTreeSet<i32> = SharedBTreeSet::new_in(&allocator);
```

#### Common Methods

```rust
// Same as HashSet, plus:
set.iter() -> ordered iterator
```

### SharedVecDeque<T>

Double-ended queue.

#### Creation

```rust
let mut deque: SharedVecDeque<i32> = SharedVecDeque::new_in(&allocator);
```

#### Common Methods

```rust
// Front operations
deque.push_front(value)
deque.pop_front() -> Option<T>
deque.front() -> Option<&T>

// Back operations
deque.push_back(value)
deque.pop_back() -> Option<T>
deque.back() -> Option<&T>

// Properties
deque.len() -> usize
deque.is_empty() -> bool
deque.clear()
```

## Examples

### Example 1: Multi-Process Counter

Process A (writer):

```rust
use std::fs;
use commy::{FreeListAllocator, SharedBox};

fn main() -> Result<()> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("counter.mmap")?;
    
    // Create 10MB file if needed
    if file.metadata()?.len() == 0 {
        fs::write("counter.mmap", vec![0u8; 10 * 1024 * 1024])?;
    }
    
    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let allocator = FreeListAllocator::new(mmap, "counter.mmap");
    
    let mut counter: SharedBox<i32> = SharedBox::new_in(&allocator);
    
    for i in 0..100 {
        *counter = i;
        println!("Incremented to {}", i);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    
    Ok(())
}
```

Process B (reader):

```rust
use std::fs;
use commy::{FreeListAllocator, SharedBox};

fn main() -> Result<()> {
    // Same allocator, same file
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("counter.mmap")?;
    
    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let allocator = FreeListAllocator::new(mmap, "counter.mmap");
    
    let counter: SharedBox<i32> = SharedBox::new_in(&allocator);
    
    for _ in 0..20 {
        println!("Current value: {}", *counter);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    
    Ok(())
}
```

### Example 2: Task Queue

```rust
use std::fs;
use commy::{FreeListAllocator, SharedVecDeque};

fn main() -> Result<()> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("tasks.mmap")?;
    
    if file.metadata()?.len() == 0 {
        fs::write("tasks.mmap", vec![0u8; 10 * 1024 * 1024])?;
    }
    
    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let allocator = FreeListAllocator::new(mmap, "tasks.mmap");
    
    // Producer: add tasks
    let mut queue: SharedVecDeque<i32> = SharedVecDeque::new_in(&allocator);
    queue.push_back(1);
    queue.push_back(2);
    queue.push_back(3);
    
    // Consumer: process tasks
    while let Some(task) = queue.pop_front() {
        println!("Processing task: {}", task);
        // Do work...
    }
    
    Ok(())
}
```

### Example 3: Configuration Map

```rust
use std::fs;
use commy::{FreeListAllocator, SharedHashMap, SharedString};

fn main() -> Result<()> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("config.mmap")?;
    
    if file.metadata()?.len() == 0 {
        fs::write("config.mmap", vec![0u8; 10 * 1024 * 1024])?;
    }
    
    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let allocator = FreeListAllocator::new(mmap, "config.mmap");
    
    let mut config: SharedHashMap<i32, SharedString> = 
        SharedHashMap::new_in(&allocator);
    
    // Store config
    let mut name = SharedString::new_in(&allocator);
    name.push_str("MyApp")?;
    config.insert(1, name);
    
    let mut version = SharedString::new_in(&allocator);
    version.push_str("1.0.0")?;
    config.insert(2, version);
    
    // Read config
    if let Some(app_name) = config.get(&1) {
        println!("App: {}", app_name.from_utf8()?);
    }
    
    Ok(())
}
```

## Best Practices

### 1. File Sizing

```rust
// Too small: frequent reallocations and errors
fs::write("shared.mmap", vec![0u8; 1024 * 1024])?;  // 1MB - risky

// Good: reasonable headroom
fs::write("shared.mmap", vec![0u8; 100 * 1024 * 1024])?;  // 100MB

// Overkill: wastes disk space
fs::write("shared.mmap", vec![0u8; 10 * 1024 * 1024 * 1024])?;  // 10GB
```

### 2. Handle Allocation Failures

```rust
// Bad: assumes allocation always succeeds
vec.push(item);

// Good: handle allocation failure
vec.push(item).or_else(|_| {
    allocator.resize_file(allocator.size() * 2)?;
    vec.push(item)
})?;
```

### 3. Heartbeat Updates

```rust
// For long-running operations, update heartbeat periodically
let allocator = FreeListAllocator::new(mmap, file_path);

loop {
    // Do work...
    allocator.update_heartbeat()?;  // Tell other processes we're alive
    std::thread::sleep(Duration::from_secs(2));
}
```

### 4. Graceful Cleanup

```rust
// In main or drop handler
if let Err(e) = allocator.cleanup_failed_resize() {
    eprintln!("Cleanup error: {}", e);
}
```

### 5. Type Consistency

```rust
// Bad: type mismatch between processes
// Process A:
let vec: SharedVec<i32> = ...;

// Process B:
let vec: SharedVec<u32> = ...;  // ❌ Type mismatch!

// Good: both processes use same type
// Both: 
let vec: SharedVec<i32> = ...;
```

### 6. Process Synchronization

```rust
// Use OS primitives for coordination:
use std::sync::atomic::{AtomicBool, Ordering};

// Shared flag stored in SharedBox
let ready: SharedBox<bool> = SharedBox::new_in(&allocator);

// Writer
*ready = true;

// Reader
while !*ready {
    std::thread::sleep(Duration::from_millis(10));
}
```

## Troubleshooting

### Issue: "Permission denied" when creating file

**Solution**: Ensure directory is writable and no other process has exclusive lock

```rust
// Check permissions before opening
let path = Path::new("shared.mmap");
if !path.parent().unwrap().metadata()?.permissions().readonly() {
    // Directory is writable
}
```

### Issue: Allocation fails with insufficient space

**Solution**: Increase file size before allocating large items

```rust
let current_size = allocator.size();
if current_size > allocator.limit() {
    allocator.resize_file(allocator.limit() * 2)?;
}
```

### Issue: Process A doesn't see updates from Process B

**Solution**: Ensure both processes use same file and allocator instance

```rust
// Correct: same file path
let alloc_a = FreeListAllocator::new(mmap_a, "shared.mmap");
let alloc_b = FreeListAllocator::new(mmap_b, "shared.mmap");  // ✓

// Wrong: different paths
let alloc_a = FreeListAllocator::new(mmap_a, "shared_a.mmap");
let alloc_b = FreeListAllocator::new(mmap_b, "shared_b.mmap");  // ❌
```

### Issue: "Operation timed out"

**Solution**: Check for dead processes and run cleanup

```rust
if allocator.check_operation_timeout() {
    eprintln!("Operation timed out, cleaning up...");
    allocator.cleanup_failed_resize()?;
}
```

### Issue: Memory leaks or growing file size

**Solution**: Explicitly deallocate containers

```rust
// Drop containers to release memory
drop(vec);
drop(map);
drop(string);

// Optionally shrink file
allocator.shrink_to_usage()?;
```

### Issue: Panic on Vec reallocation

**Solution**: Pre-allocate capacity or handle errors

```rust
// Proactive allocation
vec.reserve(1000)?;

// Or handle error
match vec.push(item) {
    Ok(()) => {},
    Err(_) => {
        allocator.resize_file(allocator.size() * 2)?;
        vec.push(item)?;
    }
}
```

---

**Status**: Production-Ready (v2.0)
**Last Updated**: Current Session
