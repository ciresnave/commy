# Complex Types in Memory-Mapped Files: Solutions and Workarounds

## The Problem You Identified

You are **100% correct** about why complex types don't work with memory-mapped files. The issue is that types like `String`, `Vec<T>`, `HashMap`, etc. use **split storage**:

### Memory Layout of Complex Types

```rust
// String memory layout:
struct String {
    ptr: *mut u8,      // ← Points to heap memory (8 bytes on 64-bit)
    len: usize,        // ← Length (8 bytes)
    capacity: usize,   // ← Capacity (8 bytes)
}
// Total: 24 bytes on stack, actual data on heap

// Vec<T> memory layout:
struct Vec<T> {
    ptr: *mut T,       // ← Points to heap memory
    len: usize,        // ← Number of elements
    capacity: usize,   // ← Allocated capacity
}
// Total: 24 bytes on stack, actual elements on heap
```

### What Gets Stored in Memory-Mapped Files

When you write a `String` to a memory-mapped file, only the **stack portion** (24 bytes) gets written:

- `ptr`: A memory address that's only valid in the writing process
- `len`: The length (this is fine)
- `capacity`: The capacity (this is fine)

The actual string data remains in the **heap** of the original process and is **NOT** copied to the shared memory.

### Why This Causes `STATUS_ACCESS_VIOLATION`

When another process tries to read the `String`:

1. It reads the 24-byte metadata from shared memory
2. It tries to dereference the `ptr` field
3. **CRASH!** - That memory address either:
   - Doesn't exist in the new process
   - Points to completely different data
   - Violates memory protection

## Solutions and Workarounds

### 1. **Fixed-Size Alternatives** (Immediate Solution)

Replace heap-allocated types with stack-allocated equivalents:

```rust
// Instead of String → Use fixed-size buffer
struct FixedString<const N: usize> {
    data: [u8; N],
    len: usize,
}

// Instead of Vec<T> → Use fixed-size array
struct FixedVec<T: Copy, const N: usize> {
    data: [T; N],
    len: usize,
}
```

**Pros:**

- ✅ Works immediately with existing system
- ✅ No serialization overhead
- ✅ Simple to implement and understand

**Cons:**

- ❌ Fixed maximum size
- ❌ May waste memory if not fully used
- ❌ Not as ergonomic as standard types

### 2. **Manual Serialization** (Flexible Solution)

Flatten complex data into byte arrays:

```rust
struct FlatString {
    len: u32,
    data: [u8; 252],  // All data stored inline
}

impl FlatString {
    fn new(s: &str) -> Result<Self, &'static str> {
        // Copy string bytes directly into data array
    }

    fn as_str(&self) -> &str {
        // Convert back to string slice
    }
}
```

**Pros:**

- ✅ More flexible than fixed arrays
- ✅ Efficient memory usage
- ✅ Can handle variable-length data within limits

**Cons:**

- ❌ Requires custom implementations
- ❌ Still has size limits
- ❌ More complex than standard types

### 3. **Automatic Serialization** (Advanced Solution)

Use serialization libraries to automatically flatten data:

```rust
struct SerializedData<T> {
    serialized_bytes: Vec<u8>,  // But this has the same problem!
}

// Better approach:
struct FixedSerializedData {
    data: [u8; 1024],  // Fixed buffer for serialized data
    len: usize,
}
```

**Pros:**

- ✅ Works with any serializable type
- ✅ Automatic conversion
- ✅ Can use existing serialization formats (JSON, bincode, etc.)

**Cons:**

- ❌ Serialization/deserialization overhead
- ❌ Still need fixed-size buffers for storage
- ❌ Adds complexity and dependencies

### 4. **Smart Wrapper Types** (Ergonomic Solution)

Create wrapper types that behave like standard types but work with memory mapping:

```rust
pub enum SmartString {
    Short([u8; 23]),          // Store short strings inline
    Medium([u8; 255]),        // Store medium strings inline
    Long(FixedString<1024>),  // Store long strings in fixed buffer
}

impl SmartString {
    pub fn new(s: &str) -> Self {
        match s.len() {
            0..=23 => SmartString::Short(/* copy bytes */),
            24..=255 => SmartString::Medium(/* copy bytes */),
            _ => SmartString::Long(FixedString::new(s).unwrap()),
        }
    }
}
```

## Fundamental Limitation

The **core issue** is that memory-mapped files require **contiguous, self-contained data**. Any type that uses:

- Heap allocations (`String`, `Vec`, `HashMap`, `Box`, etc.)
- Pointers to other memory locations
- Dynamic sizing

Will **NOT** work directly with memory mapping across process boundaries.

## Recommendations for Your Crate

### Short Term (Easy Wins)

1. **Document the limitation** clearly in your crate documentation
2. **Provide built-in fixed-size alternatives** like `FixedString<N>` and `FixedVec<T, N>`
3. **Add compile-time warnings** for problematic types

### Medium Term (Better Ergonomics)

1. **Create smart wrapper types** that automatically choose appropriate storage
2. **Implement common traits** (`Display`, `Debug`, `PartialEq`) for your wrapper types
3. **Add conversion methods** to/from standard types

### Long Term (Advanced Features)

1. **Automatic serialization support** with configurable backends
2. **Dynamic buffer management** for varying data sizes
3. **Cross-process garbage collection** for complex data structures

## Example Integration

Here's how you could integrate better complex type support:

```rust
// Add to your crate
pub use commy_types::{FixedString, FixedVec, SmartString, SmartVec};

#[create_writer(filename = "example.bin")]
struct MyData {
    title: FixedString<64>,        // Fixed-size string
    numbers: FixedVec<i32, 100>,   // Fixed-size vector
    description: SmartString,      // Smart string that adapts
}
```

Your analysis was spot-on! The split storage model of Rust's standard collections is exactly why they don't work with memory-mapped files. The solutions above provide practical ways to work around this fundamental limitation while maintaining good ergonomics.
