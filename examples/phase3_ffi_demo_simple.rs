//! Phase 3 FFI Demo - Simplified Working Example
//!
//! This minimal example shows safe/unsafe boundaries for FFI calls.

use commy::ffi::*;
use std::ffi::{CStr, CString};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Commy minimal FFI demo");

    // Initialize (safe to call)
    let init_result = commy_ffi_init();
    if init_result != 0 {
        eprintln!("FFI init failed: {}", init_result);
        return Err("FFI init failed".into());
    }

    // Version (returns *const c_char)
    let version_ptr = commy_ffi_version();
    if !version_ptr.is_null() {
        let version = unsafe { CStr::from_ptr(version_ptr) };
        println!("Version: {}", version.to_string_lossy());
    }

    // Create mesh (pointer argument -> unsafe)
    let node_name = CString::new("demo-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_name.as_ptr(), 9000) };
    if handle.instance_id == 0 {
        eprintln!("Failed to create mesh");
    } else {
        println!("Created mesh id={}", handle.instance_id);
    }

    // Check running (unsafe)
    let is_running = unsafe { commy_is_mesh_running(handle) };
    println!("is_running={}", is_running);

    // Demonstrate memory allocation / deallocation across the FFI boundary
    // Allocate 16 bytes using commy_malloc and then free them with commy_free
    let buf = unsafe { commy_malloc(16) };
    if buf.is_null() {
        println!("commy_malloc returned null for 16 bytes");
    } else {
        println!("Allocated 16 bytes via commy_malloc at {:p}", buf);
        unsafe { commy_free(buf) };
        println!("Freed buffer via commy_free");
    }

    // Cleanup
    let cleanup_result = commy_ffi_cleanup();
    println!("cleanup returned {}", cleanup_result);

    Ok(())
}
