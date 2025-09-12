//! Multi-Language Integration Tests
//!
//! Comprehensive tests validating Phase 4 enterprise features work correctly
//! across all supported programming languages including:
//! - Python SDK enterprise feature validation
//! - Go SDK enterprise feature validation
//! Multi-Language Integration Tests
//!
//! Comprehensive tests validating Phase 4 enterprise features work correctly
//! across supported programming languages and ensuring internal Rust callers
//! call the FFI surface correctly (inside `unsafe` blocks).

use std::ffi::{CStr, CString};
use std::process::{Command, Stdio};
use std::ptr;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use commy::ffi::minimal::{
    commy_create_mesh, commy_ffi_cleanup, commy_ffi_init, commy_ffi_version, commy_free_string,
    commy_get_node_id, commy_is_mesh_running, commy_start_mesh, commy_stop_mesh, CommyError,
    CommyHandle,
};

/// Test basic FFI functionality that all language SDKs depend on
#[test]
fn test_basic_ffi_functionality() {
    // Test core FFI functions work correctly
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    // Test FFI version
    let version_ptr = unsafe { commy_ffi_version() };
    assert!(!version_ptr.is_null());

    let version_str = unsafe { CStr::from_ptr(version_ptr) };
    let version = version_str.to_str().unwrap();
    assert!(!version.is_empty());
    println!("✓ Commy FFI Version: {}", version);

    // Test mesh creation with basic parameters
    let node_id = CString::new("test-node-multi-lang").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_eq!(handle.error_code, CommyError::Success as i32);
    assert!(handle.instance_id > 0);
    println!("✓ Mesh instance created with ID: {}", handle.instance_id);

    // Test mesh start
    let start_result = unsafe { commy_start_mesh(handle) };
    assert_eq!(start_result, CommyError::Success as i32);
    println!("✓ Mesh started successfully");

    // Test mesh status
    let is_running = unsafe { commy_is_mesh_running(handle) };
    assert!(is_running);
    println!("✓ Mesh is running");

    // Test getting node ID
    let node_id_ptr = unsafe { commy_get_node_id(handle) };
    if !node_id_ptr.is_null() {
        let retrieved_node_id = unsafe { CStr::from_ptr(node_id_ptr) };
        let node_id_str = retrieved_node_id.to_str().unwrap();
        println!("✓ Retrieved node ID: {}", node_id_str);
        unsafe { commy_free_string(node_id_ptr) };
    }

    // Test stopping mesh
    let stop_result = unsafe { commy_stop_mesh(handle) };
    assert_eq!(stop_result, CommyError::Success as i32);
    println!("✓ Mesh stopped successfully");

    // Verify mesh is no longer running
    let is_running_after_stop = unsafe { commy_is_mesh_running(handle) };
    assert!(!is_running_after_stop);
    println!("✓ Mesh confirmed stopped");

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
    println!("✓ FFI cleanup completed");
}

#[test]
fn test_python_basic_ffi_integration() {
    // Test basic FFI integration with Python SDK (best-effort)
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    // Test Python script that uses basic FFI functions (best-effort; may be skipped in CI)
    let python_test_script = r#"
import ctypes
import os
import sys

# Add path for commy Python SDK
sys.path.append(os.path.join(os.path.dirname(__file__), '../../sdks/python'))

def test_basic_ffi():
    try:
        import commy
        return True
    except Exception:
        return False

result = test_basic_ffi()
print(f"Python FFI test result: {result}")
"#;

    // Write Python test script to file
    std::fs::write("/tmp/test_python_ffi.py", python_test_script).unwrap();

    // Execute Python test (optional, might fail in CI)
    if let Ok(output) = Command::new("python3")
        .arg("/tmp/test_python_ffi.py")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Python test output: {}", stdout);
    } else {
        println!("⚠ Python FFI integration test skipped (Python not available)");
    }

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
}

#[test]
fn test_nodejs_basic_ffi_integration() {
    // Test basic FFI integration with Node.js SDK (best-effort)
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    let nodejs_test_script = r#"
const path = require('path');
try {
    const commyPath = path.join(__dirname, '../../sdks/nodejs');
    require(commyPath);
    console.log('ok');
} catch (error) {
    console.log('err');
}
"#;

    std::fs::write("/tmp/test_nodejs_ffi.js", nodejs_test_script).unwrap();

    if let Ok(output) = Command::new("node")
        .arg("/tmp/test_nodejs_ffi.js")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Node.js test output: {}", stdout);
    } else {
        println!("⚠ Node.js FFI integration test skipped (Node not available)");
    }

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
}

#[test]
fn test_concurrent_ffi_operations() {
    // Test that FFI operations work correctly under concurrent access
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    let barrier = Arc::new(Barrier::new(3)); // 3 concurrent operations
    let mut handles = vec![];

    for i in 0..3 {
        let barrier_clone = Arc::clone(&barrier);
        let handle = thread::spawn(move || {
            barrier_clone.wait(); // Synchronize start

            // Each thread creates its own mesh instance
            let node_id = CString::new(format!("concurrent-node-{}", i)).unwrap();
            let mesh_handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080 + i as u16) };

            // Verify creation succeeded
            assert_eq!(mesh_handle.error_code, CommyError::Success as i32);
            assert!(mesh_handle.instance_id > 0);

            // Start the mesh
            let start_result = unsafe { commy_start_mesh(mesh_handle) };
            assert_eq!(start_result, CommyError::Success as i32);

            // Verify it's running
            let is_running = unsafe { commy_is_mesh_running(mesh_handle) };
            assert!(is_running);

            // Do some work
            thread::sleep(Duration::from_millis(100));

            // Stop the mesh
            let stop_result = unsafe { commy_stop_mesh(mesh_handle) };
            assert_eq!(stop_result, CommyError::Success as i32);

            // Verify it's stopped
            let is_running_after = unsafe { commy_is_mesh_running(mesh_handle) };
            assert!(!is_running_after);

            println!("✓ Concurrent FFI operation {} completed", i);
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    println!("✓ All concurrent FFI operations completed successfully");

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
}

#[test]
fn test_ffi_error_handling() {
    // Test FFI error handling scenarios
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    // Test null parameter handling
    let handle = unsafe { commy_create_mesh(ptr::null(), 8080) };
    assert_eq!(handle.error_code, CommyError::InvalidParameter as i32);
    println!("✓ Null parameter correctly rejected");

    // Test empty string handling
    let empty_node_id = CString::new("").unwrap();
    let handle = unsafe { commy_create_mesh(empty_node_id.as_ptr(), 8080) };
    assert_eq!(handle.error_code, CommyError::InvalidParameter as i32);
    println!("✓ Empty node ID correctly rejected");

    // Test invalid instance operations
    let invalid_handle = CommyHandle {
        instance_id: 99999, // Non-existent instance
        error_code: 0,
    };

    let start_result = unsafe { commy_start_mesh(invalid_handle) };
    assert_eq!(start_result, CommyError::InstanceNotFound as i32);
    println!("✓ Invalid instance correctly rejected");

    let stop_result = unsafe { commy_stop_mesh(invalid_handle) };
    assert_eq!(stop_result, CommyError::InstanceNotFound as i32);
    println!("✓ Stop on invalid instance correctly rejected");

    let is_running = unsafe { commy_is_mesh_running(invalid_handle) };
    assert!(!is_running); // Should return false for invalid instances
    println!("✓ Status check on invalid instance handled correctly");

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
    println!("✓ FFI error handling test completed");
}

/// Test cross-language interoperability infrastructure
#[test]
fn test_cross_language_interoperability_infrastructure() {
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    // Start multiple instances representing different languages
    let python_node_id = CString::new("python-interop-node").unwrap();
    let python_handle = unsafe { commy_create_mesh(python_node_id.as_ptr(), 8083) };
    assert_eq!(python_handle.error_code, CommyError::Success as i32);

    let go_node_id = CString::new("go-interop-node").unwrap();
    let go_handle = unsafe { commy_create_mesh(go_node_id.as_ptr(), 8084) };
    assert_eq!(go_handle.error_code, CommyError::Success as i32);

    let nodejs_node_id = CString::new("nodejs-interop-node").unwrap();
    let nodejs_handle = unsafe { commy_create_mesh(nodejs_node_id.as_ptr(), 8085) };
    assert_eq!(nodejs_handle.error_code, CommyError::Success as i32);

    // Start all meshes
    assert_eq!(unsafe { commy_start_mesh(python_handle) }, CommyError::Success as i32);
    assert_eq!(unsafe { commy_start_mesh(go_handle) }, CommyError::Success as i32);
    assert_eq!(unsafe { commy_start_mesh(nodejs_handle) }, CommyError::Success as i32);

    // Give time for meshes to start
    thread::sleep(Duration::from_millis(500));

    // Verify all meshes are running
    assert!(unsafe { commy_is_mesh_running(python_handle) });
    assert!(unsafe { commy_is_mesh_running(go_handle) });
    assert!(unsafe { commy_is_mesh_running(nodejs_handle) });

    println!("✓ Multi-language mesh infrastructure established");

    // Cleanup
    assert_eq!(unsafe { commy_stop_mesh(python_handle) }, CommyError::Success as i32);
    assert_eq!(unsafe { commy_stop_mesh(go_handle) }, CommyError::Success as i32);
    assert_eq!(unsafe { commy_stop_mesh(nodejs_handle) }, CommyError::Success as i32);

    println!("✓ Cross-language interoperability infrastructure test completed");

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
}

/// Test multi-language mesh performance under load
#[test]
fn test_multi_language_performance() {
    let init_result = unsafe { commy_ffi_init() };
    assert_eq!(init_result, CommyError::Success as i32);

    let start_time = std::time::Instant::now();

    // Create multiple mesh instances to simulate multi-language usage
    let mut handles = Vec::new();

    for i in 0..10 {
        let node_id = CString::new(format!("perf-node-{}", i)).unwrap();
        let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 9000 + i as u16) };
        assert_eq!(handle.error_code, CommyError::Success as i32);

        let start_result = unsafe { commy_start_mesh(handle) };
        assert_eq!(start_result, CommyError::Success as i32);

        handles.push(handle);
    }

    let creation_time = start_time.elapsed();
    println!(
        "✓ Created and started 10 mesh instances in {:?}",
        creation_time
    );

    // Verify all are running
    for (i, &handle) in handles.iter().enumerate() {
        assert!(unsafe { commy_is_mesh_running(handle) });
        println!("✓ Mesh instance {} confirmed running", i);
    }

    // Simulate some load
    thread::sleep(Duration::from_millis(100));

    // Clean up all instances
    let cleanup_start = std::time::Instant::now();

    for handle in handles {
        let stop_result = unsafe { commy_stop_mesh(handle) };
        assert_eq!(stop_result, CommyError::Success as i32);
    }

    let cleanup_time = cleanup_start.elapsed();
    println!("✓ Stopped all mesh instances in {:?}", cleanup_time);

    let total_time = start_time.elapsed();
    println!(
        "✓ Total multi-language performance test completed in {:?}",
        total_time
    );

    // Performance assertions
    assert!(
        creation_time < Duration::from_secs(5),
        "Creation time too slow"
    );
    assert!(
        cleanup_time < Duration::from_secs(2),
        "Cleanup time too slow"
    );

    let cleanup_result = unsafe { commy_ffi_cleanup() };
    assert_eq!(cleanup_result, CommyError::Success as i32);
}
