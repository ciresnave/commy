#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
//! Phase 3 FFI Demo - Multi-language SDK Foundation
//!
//! This example demonstrates the C FFI interface that serves as the foundation
//! for multi-language SDKs. It shows how to:
//!
//! 1. Initialize the FFI layer
//! 2. Create and configure a mesh
//! 3. Register and discover services
//! 4. Configure health monitoring
//! 5. Set up load balancing
//! 6. Get statistics and status information
//! 7. Clean up resources properly

use commy::ffi::*;
use std::ffi::{CStr, CString};
use std::ptr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Commy Phase 3 FFI Demo - Multi-language SDK Foundation");

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
    let handle = commy_create_mesh(node_id.as_ptr(), 8080);

    if handle.instance_id == 0 {
        eprintln!("❌ Failed to create mesh coordinator");
        return Err("Mesh creation failed".into());
    }
    println!(
        "✅ Mesh coordinator created with ID: {}",
        handle.instance_id
    );

    // Configure health monitoring
    println!("\n💓 Configuring health monitoring...");
    let health_config = CommyHealthConfig {
        check_interval_ms: 5000, // 5 seconds
        timeout_ms: 1000,        // 1 second
        max_failures: 3,
        recovery_checks: 2,
    };

    // Configure load balancer
    println!("⚖️ Configuring load balancer...");
    let lb_config = CommyLoadBalancerConfig {
        algorithm: CommyLoadBalancerAlgorithm::PerformanceBased,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 5,
        circuit_breaker_timeout_ms: 30000, // 30 seconds
    };

    let config_result =
        commy_configure_mesh(handle, &health_config as *const _, &lb_config as *const _);

    if config_result != 0 {
        println!("⚠️ Mesh configuration returned code: {}", config_result);
    } else {
        println!("✅ Mesh configured successfully");
    }

    // Start the mesh (this may fail if we don't have the mesh feature enabled)
    println!("\n🏃 Starting mesh coordinator...");
    let start_result = commy_start_mesh(handle);
    if start_result != 0 {
        println!(
            "⚠️ Mesh start returned code: {} (may be expected if mesh feature disabled)",
            start_result
        );
    } else {
        println!("✅ Mesh coordinator started successfully");
    }

    // Register a demo service
    println!("\n📋 Registering demo services...");

    let service_name = CString::new("user-service").unwrap();
    let service_id = CString::new("user-service-1").unwrap();
    let endpoint = CString::new("127.0.0.1").unwrap();
    let metadata = CString::new(r#"{"version": "1.0", "region": "us-east-1"}"#).unwrap();

    let service_config = CommyServiceConfig {
        service_name: service_name.as_ptr(),
        service_id: service_id.as_ptr(),
        endpoint: endpoint.as_ptr(),
        port: 8081,
        weight: 100,
        metadata: metadata.as_ptr(),
    };

    let register_result = commy_register_service(handle, &service_config as *const _);
    if register_result != 0 {
        println!("⚠️ Service registration returned code: {}", register_result);
    } else {
        println!("✅ Service 'user-service-1' registered successfully");
    }

    // Register another service instance
    let service_id2 = CString::new("user-service-2").unwrap();
    let service_config2 = CommyServiceConfig {
        service_name: service_name.as_ptr(),
        service_id: service_id2.as_ptr(),
        endpoint: endpoint.as_ptr(),
        port: 8082,
        weight: 150,
        metadata: metadata.as_ptr(),
    };

    let register_result2 = commy_register_service(handle, &service_config2 as *const _);
    if register_result2 != 0 {
        println!(
            "⚠️ Service registration returned code: {}",
            register_result2
        );
    } else {
        println!("✅ Service 'user-service-2' registered successfully");
    }

    // Discover services
    println!("\n🔍 Discovering services...");
    let mut services_ptr: *mut CommyServiceInfo = ptr::null_mut();
    let mut service_count: usize = 0;

    let discover_result = commy_discover_services(
        handle,
        service_name.as_ptr(),
        &mut services_ptr as *mut _,
        &mut service_count as *mut _,
    );

    if discover_result == 0 && service_count > 0 {
        println!("✅ Discovered {} service(s):", service_count);

        unsafe {
            for i in 0..service_count {
                let service = &*services_ptr.add(i);
                let name = if !service.service_name.is_null() {
                    CStr::from_ptr(service.service_name).to_string_lossy()
                } else {
                    "N/A".into()
                };
                let id = if !service.service_id.is_null() {
                    CStr::from_ptr(service.service_id).to_string_lossy()
                } else {
                    "N/A".into()
                };
                let endpoint_str = if !service.endpoint.is_null() {
                    CStr::from_ptr(service.endpoint).to_string_lossy()
                } else {
                    "N/A".into()
                };

                println!(
                    "  📍 {} ({}) at {}:{} - Weight: {}",
                    name, id, endpoint_str, service.port, service.weight
                );
            }

            // Free the services array
            commy_free_service_info_array(services_ptr, service_count);
        }
    } else {
        println!(
            "⚠️ Service discovery returned code: {}, count: {}",
            discover_result, service_count
        );
    }

    // Get mesh statistics
    println!("\n📊 Getting mesh statistics...");
    let mut stats = CommyMeshStats {
        total_services: 0,
        healthy_services: 0,
        unhealthy_services: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        average_response_time_ms: 0.0,
    };

    let stats_result = commy_get_mesh_stats(handle, &mut stats as *mut _);
    if stats_result == 0 {
        println!("✅ Mesh Statistics:");
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

    // Test service selection
    println!("\n🎯 Testing service selection...");
    let mut selected_service = CommyServiceInfo {
        service_name: ptr::null() as *const _,
        service_id: ptr::null() as *const _,
        endpoint: ptr::null() as *const _,
        port: 0,
        status: CommyServiceStatus::Unknown,
        weight: 0,
        response_time_ms: 0,
    };

    let select_result = commy_select_service(
        handle,
        service_name.as_ptr(),
        ptr::null(), // No client ID
        &mut selected_service as *mut _,
    );

    if select_result == 0 {
        let selected_id = if !selected_service.service_id.is_null() {
            unsafe { CStr::from_ptr(selected_service.service_id).to_string_lossy() }
        } else {
            "N/A".into()
        };
        let selected_endpoint = if !selected_service.endpoint.is_null() {
            unsafe { CStr::from_ptr(selected_service.endpoint).to_string_lossy() }
        } else {
            "N/A".into()
        };

        println!(
            "✅ Selected service: {} at {}:{}",
            selected_id, selected_endpoint, selected_service.port
        );

        // Free the allocated strings
        unsafe {
            if !selected_service.service_name.is_null() {
                commy_free_string(selected_service.service_name as *mut _);
            }
            if !selected_service.service_id.is_null() {
                commy_free_string(selected_service.service_id as *mut _);
            }
            if !selected_service.endpoint.is_null() {
                commy_free_string(selected_service.endpoint as *mut _);
            }
        }
    } else {
        println!("⚠️ Service selection returned code: {}", select_result);
    }

    // Check if mesh is running
    println!("\n🔍 Checking mesh status...");
    let is_running = commy_is_mesh_running(handle);
    match is_running {
        1 => println!("✅ Mesh is running"),
        0 => println!("⏹️ Mesh is not running"),
        _ => println!("⚠️ Mesh status check returned code: {}", is_running),
    }

    // Get node ID
    println!("\n🏷️ Getting node ID...");
    let node_id_ptr = commy_get_node_id(handle);
    if !node_id_ptr.is_null() {
        let node_id_str = unsafe { CStr::from_ptr(node_id_ptr) };
        println!("✅ Node ID: {}", node_id_str.to_string_lossy());
        unsafe { commy_free_string(node_id_ptr) };
    } else {
        println!("⚠️ Failed to get node ID");
    }

    // Demonstrate memory management
    println!("\n🧠 Testing memory management...");
    let test_string = CString::new("Hello from FFI!").unwrap();
    let duplicated = unsafe { commy_strdup(test_string.as_ptr()) };
    if !duplicated.is_null() {
        let dup_str = unsafe { CStr::from_ptr(duplicated) };
        println!("✅ String duplication: {}", dup_str.to_string_lossy());
        unsafe { commy_free(duplicated as *mut std::ffi::c_void) };
    } else {
        println!("❌ String duplication failed");
    }

    // Test service info array allocation
    println!("\n📋 Testing service info array allocation...");
    let test_array = unsafe { commy_alloc_service_info_array(3) };
    if !test_array.is_null() {
        println!("✅ Allocated service info array for 3 services");
        unsafe { commy_free_service_info_array(test_array, 3) };
        println!("✅ Freed service info array");
    } else {
        println!("❌ Service info array allocation failed");
    }

    // Stop the mesh
    println!("\n⏹️ Stopping mesh coordinator...");
    let stop_result = commy_stop_mesh(handle);
    if stop_result != 0 {
        println!(
            "⚠️ Mesh stop returned code: {} (may be expected)",
            stop_result
        );
    } else {
        println!("✅ Mesh coordinator stopped successfully");
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
    println!("\n📚 This demo shows the foundation for multi-language SDKs:");
    println!("   🐍 Python SDK can use ctypes/cffi to bind to this interface");
    println!("   📦 Node.js SDK can use node-ffi-napi for bindings");
    println!("   🐹 Go SDK can use cgo for integration");
    println!("   ☕ Java SDK can use JNI for native integration");
    println!("   🔷 .NET SDK can use P/Invoke for interop");
    println!("   🔧 C/C++ can use the header file directly");

    Ok(())
}
