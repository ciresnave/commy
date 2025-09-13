//! Memory management utilities for FFI
//!
//! This module provides safe memory management functions for the FFI interface

use crate::ffi::types::*;
#[cfg(feature = "ffi")]
use libc::{c_char, c_void, free, malloc, size_t};

/// Allocate memory using the C allocator
/// This is useful for allocating memory that will be freed by other languages
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// Allocates `size` bytes using the C allocator. The caller is responsible for
/// freeing the returned pointer with `commy_free` when it is no longer needed.
pub unsafe extern "C" fn commy_malloc(size: size_t) -> *mut c_void {
    if size == 0 {
        return std::ptr::null_mut();
    }
    unsafe { malloc(size) }
}

/// Allocate memory using the C allocator
/// This is useful for allocating memory that will be freed by other languages
// The existing memory FFI functions already include # Safety docs stating
// pointer ownership and null handling requirements. No functional change.

/// Free memory allocated by commy_malloc
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by
/// `commy_malloc`. Passing arbitrary pointers is undefined behavior.
pub unsafe extern "C" fn commy_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { free(ptr) }
    }
}

/// Allocate and copy a string to C-managed memory
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `src` must be a valid, non-null, NUL-terminated C string pointer. The
/// returned pointer must be freed with `commy_free_string` when no longer
/// needed.
pub unsafe extern "C" fn commy_strdup(src: *const c_char) -> *mut c_char {
    if src.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let len = libc::strlen(src);
        let dst = malloc(len + 1) as *mut c_char;
        if dst.is_null() {
            return std::ptr::null_mut();
        }
        libc::strcpy(dst, src);
        dst
    }
}

/// Get the length of a C string
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `s` must be a valid, non-null, NUL-terminated C string pointer.
pub unsafe extern "C" fn commy_strlen(s: *const c_char) -> size_t {
    if s.is_null() {
        0
    } else {
        unsafe { libc::strlen(s) }
    }
}

/// Copy memory safely
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `dst` and `src` must be valid pointers with at least `size` bytes of
/// accessible memory. Behavior is undefined for overlapping regions.
pub unsafe extern "C" fn commy_memcpy(
    dst: *mut c_void,
    src: *const c_void,
    size: size_t,
) -> *mut c_void {
    if dst.is_null() || src.is_null() || size == 0 {
        return dst;
    }
    unsafe { libc::memcpy(dst, src, size) }
}

/// Set memory to a specific value
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `ptr` must be a valid pointer to at least `size` bytes of writable memory.
pub unsafe extern "C" fn commy_memset(ptr: *mut c_void, value: i32, size: size_t) -> *mut c_void {
    if ptr.is_null() || size == 0 {
        return ptr;
    }
    unsafe { libc::memset(ptr, value, size) }
}

/// Allocate an array of CommyServiceInfo structs
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// Allocates an array of `count` CommyServiceInfo structs. The returned
/// pointer must be freed with `commy_free_service_info_array` when no longer
/// needed.
pub unsafe extern "C" fn commy_alloc_service_info_array(count: size_t) -> *mut CommyServiceInfo {
    if count == 0 {
        return std::ptr::null_mut();
    }

    let size = count * std::mem::size_of::<CommyServiceInfo>();
    let ptr = unsafe { malloc(size) } as *mut CommyServiceInfo;

    if !ptr.is_null() {
        // Initialize all structs to safe defaults
        unsafe {
            for i in 0..count {
                let service_info = ptr.add(i);
                std::ptr::write(
                    service_info,
                    CommyServiceInfo {
                        service_name: std::ptr::null(),
                        service_id: std::ptr::null(),
                        endpoint: std::ptr::null(),
                        port: 0,
                        status: CommyServiceStatus::Unknown,
                        weight: 0,
                        response_time_ms: 0,
                    },
                );
            }
        }
    }

    ptr
}

/// Free an array of CommyServiceInfo structs
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `ptr` must be a pointer previously returned by
/// `commy_alloc_service_info_array` with the same `count` value. Passing
/// mismatched pointers/counts is undefined behavior.
pub unsafe extern "C" fn commy_free_service_info_array(ptr: *mut CommyServiceInfo, count: size_t) {
    if ptr.is_null() || count == 0 {
        return;
    }

    unsafe {
        // Free all string pointers first
        for i in 0..count {
            let service_info = ptr.add(i);
            if !(*service_info).service_name.is_null() {
                free((*service_info).service_name as *mut c_void);
            }
            if !(*service_info).service_id.is_null() {
                free((*service_info).service_id as *mut c_void);
            }
            if !(*service_info).endpoint.is_null() {
                free((*service_info).endpoint as *mut c_void);
            }
        }

        // Free the array itself
        free(ptr as *mut c_void);
    }
}

/// Create a CommyServiceInfo with allocated strings
#[cfg(feature = "ffi")]
pub fn create_service_info_with_alloc(
    service_name: &str,
    service_id: &str,
    endpoint: &str,
    port: u16,
    status: CommyServiceStatus,
    weight: u32,
    response_time_ms: u64,
) -> CommyServiceInfo {
    CommyServiceInfo {
        service_name: unsafe {
            commy_strdup(std::ffi::CString::new(service_name).unwrap().as_ptr())
        },
        service_id: unsafe { commy_strdup(std::ffi::CString::new(service_id).unwrap().as_ptr()) },
        endpoint: unsafe { commy_strdup(std::ffi::CString::new(endpoint).unwrap().as_ptr()) },
        port,
        status,
        weight,
        response_time_ms,
    }
}

/// Memory pool for frequently allocated objects
#[cfg(feature = "ffi")]
pub struct MemoryPool {
    // Simple bump allocator for demonstration
    // In production, you might want a more sophisticated allocator
}

#[cfg(feature = "ffi")]
impl MemoryPool {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(feature = "ffi")]
impl Default for MemoryPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Global memory pool instance
#[cfg(feature = "ffi")]
use once_cell::sync::Lazy;
#[cfg(feature = "ffi")]
static MEMORY_POOL: Lazy<parking_lot::Mutex<MemoryPool>> =
    Lazy::new(|| parking_lot::Mutex::new(MemoryPool::new()));

/// Initialize the memory pool
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// This function initializes global memory pool state. It must not be
/// called concurrently with other FFI memory operations that access the
/// pool during initialization. Callers must ensure any required runtime
/// initialization ordering guarantees.
pub unsafe extern "C" fn commy_memory_pool_init() -> i32 {
    // Initialize the memory pool
    let _pool = &*MEMORY_POOL;
    0 // Success
}

/// Cleanup the memory pool
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_memory_pool_cleanup() -> i32 {
    // Pool cleanup happens automatically when the program exits
    0 // Success
}
