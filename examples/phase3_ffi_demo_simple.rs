//! Phase 3 FFI Demo - Simplified Working Example
//!
//! This example demonstrates the C FFI interface foundation that works
//! with our current mesh implementation. It shows how to:
//!
//! 1. Initialize the FFI layer
//! 2. Create a mesh coordinator instance
//! 3. Test basic FFI functionality
//! 4. Clean up resources properly

use commy::ffi::*;
use std::ffi::{CStr, CString};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Commy Phase 3 FFI Demo - Simplified Working Example");

    // Initialize the FFI layer
    println!("📡 Initializing FFI layer...");
    let init_result = commy_ffi_init();
    if init_result != 0 {
        eprintln!("❌ Failed to initialize FFI layer: {}", init_result);
        return Err("FFI initialization failed".into());
    }
    println!("✅ FFI layer initialized successfully");

    // Get and display version
    let version_ptr = commy_ffi_version();
    if !version_ptr.is_null() {
        let version = unsafe { CStr::from_ptr(version_ptr) };
        println!("📖 Commy version: {}", version.to_string_lossy());
    }

    // Create a mesh coordinator
    println!("\n🌐 Creating mesh coordinator...");
    let node_id = CString::new("demo-node-1").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };

    if handle.instance_id == 0 {
        eprintln!("❌ Failed to create mesh coordinator");
        return Err("Mesh creation failed".into());
    }
    println!(
        "✅ Mesh coordinator created with ID: {}",
        handle.instance_id
    );

    // Test basic functionality that we know works
    println!("\n🔍 Testing basic FFI functionality...");

    // Check if mesh is running (should be false initially)
    let is_running = commy_is_mesh_running(handle);
    match is_running {
        0 => println!("✅ Mesh is not running (expected)"),
        1 => println!("⚠️ Mesh is unexpectedly running"),
        code => println!("⚠️ Mesh status check returned code: {}", code),
    }

    // Get node ID
    println!("\n🏷️ Getting node ID...");
    let node_id_ptr = commy_get_node_id(handle);
    if !node_id_ptr.is_null() {
        let node_id_str = unsafe { CStr::from_ptr(node_id_ptr) };
        println!("✅ Node ID: {}", node_id_str.to_string_lossy());
        unsafe {
            commy_free_string(node_id_ptr);
        }
    } else {
        println!("⚠️ Failed to get node ID");
    }

    // Test memory management functions
    println!("\n🧠 Testing memory management...");

    // Test string duplication
    let test_string = CString::new("Hello from FFI!").unwrap();
    let duplicated = unsafe { commy_strdup(test_string.as_ptr()) };
    if !duplicated.is_null() {
        let dup_str = unsafe { CStr::from_ptr(duplicated) };
        println!("✅ String duplication: {}", dup_str.to_string_lossy());
        unsafe {
            commy_free(duplicated as *mut std::ffi::c_void);
        }
        println!("✅ String freed successfully");
    } else {
        println!("❌ String duplication failed");
    }

    // Test service info array allocation
    println!("\n📋 Testing service info array allocation...");
    let test_array = unsafe { commy_alloc_service_info_array(3) };
    if !test_array.is_null() {
        println!("✅ Allocated service info array for 3 services");
        unsafe {
            commy_free_service_info_array(test_array, 3);
        }
        println!("✅ Freed service info array");
    } else {
        println!("❌ Service info array allocation failed");
    }

    // Test mesh statistics (this will return default values)
    println!("\n📊 Testing mesh statistics...");
    let mut stats = CommyMeshStats {
        total_services: 0,
        healthy_services: 0,
        unhealthy_services: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        average_response_time_ms: 0.0,
    };

    let stats_result = unsafe { commy_get_mesh_stats(handle, &mut stats as *mut _) };
    if stats_result == 0 {
        println!("✅ Mesh Statistics (default values):");
        println!("  📈 Total Services: {}", stats.total_services);
        println!("  💚 Healthy Services: {}", stats.healthy_services);
        println!("  ❤️ Unhealthy Services: {}", stats.unhealthy_services);
        println!("  📨 Total Requests: {}", stats.total_requests);
        println!("  ✅ Successful Requests: {}", stats.successful_requests);
        println!("  ❌ Failed Requests: {}", stats.failed_requests);
        println!(
            "  ⏱️ Average Response Time: {:.2}ms",
            stats.average_response_time_ms
        );
    } else {
        println!("⚠️ Get mesh stats returned code: {}", stats_result);
    }

    // Test error handling
    println!("\n🚨 Testing error handling...");

    // Try to create a mesh with invalid parameters
    let invalid_node_id = CString::new("").unwrap(); // Empty node ID
    let invalid_handle = unsafe { commy_create_mesh(invalid_node_id.as_ptr(), 0) }; // Port 0

    if invalid_handle.instance_id == 0 {
        println!("✅ Error handling works: Invalid mesh creation properly rejected");
    } else {
        println!("⚠️ Unexpected: Invalid mesh creation succeeded");
    }

    // Cleanup FFI layer
    println!("\n🧹 Cleaning up FFI layer...");
    let cleanup_result = commy_ffi_cleanup();
    if cleanup_result != 0 {
        println!("⚠️ FFI cleanup returned code: {}", cleanup_result);
    } else {
        println!("✅ FFI layer cleaned up successfully");
    }

    println!("\n🎉 Phase 3 FFI demo completed successfully!");
    println!("\n🌟 Key Achievements:");
    println!("   ✅ FFI layer initialization and cleanup");
    println!("   ✅ Mesh coordinator instance creation");
    println!("   ✅ Memory management functions");
    println!("   ✅ Error handling and validation");
    println!("   ✅ Basic status and information retrieval");

    println!("\n📚 This demonstrates the foundation for multi-language SDKs:");
    println!("   🐍 Python SDK - Ready to implement with ctypes");
    println!("   📦 Node.js SDK - Ready to implement with ffi-napi");
    println!("   🐹 Go SDK - Ready to implement with cgo");
    println!("   ☕ Java SDK - Ready to implement with JNI");
    println!("   🔷 .NET SDK - Ready to implement with P/Invoke");
    println!("   🔧 C/C++ - Can use the header file directly");

    println!("\n🎯 Next Steps:");
    println!("   • Implement full mesh functionality in the coordinator");
    println!("   • Add service registration/discovery to FFI");
    println!("   • Complete health monitoring integration");
    println!("   • Build and test multi-language SDKs");

    Ok(())
}
