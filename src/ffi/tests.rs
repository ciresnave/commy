//! Tests for the FFI implementation
//!
//! These tests validate the FFI layer functionality including:
//! - Basic initialization and cleanup
//! - Handle management
//! - Error handling
//! - Memory management
//! - Service registration and discovery APIs

use super::working_sync::*;
use std::ffi::CString;
use std::ptr;

/// Helper function to clear global state between tests
fn clear_global_state() {
    commy_ffi_reset();
}

#[test]
fn test_ffi_initialization() {
    clear_global_state();
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);
    assert_eq!(commy_ffi_init(), CommyError::Success as i32); // Should handle double init
    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_version_info() {
    let version_ptr = commy_ffi_version();
    assert!(!version_ptr.is_null());

    unsafe {
        let version_str = std::ffi::CStr::from_ptr(version_ptr);
        let version = version_str.to_string_lossy();
        assert!(!version.is_empty());
        assert!(version.contains("0.1.0"));
    }
}

#[test]
fn test_mesh_creation_and_destruction() {
    clear_global_state();
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    // Test successful creation
    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);
    assert_eq!(handle.error_code, CommyError::Success as i32);

    // Test invalid parameters
    let invalid_handle = unsafe { commy_create_mesh(ptr::null(), 8080) };
    assert_eq!(invalid_handle.instance_id, 0);
    assert_eq!(
        invalid_handle.error_code,
        CommyError::InvalidParameter as i32
    );

    let invalid_handle2 = unsafe { commy_create_mesh(node_id.as_ptr(), 0) };
    assert_eq!(invalid_handle2.instance_id, 0);
    assert_eq!(
        invalid_handle2.error_code,
        CommyError::InvalidParameter as i32
    );

    // Additional tests suggested by reviewers: ensure zero port and null pointer handling
    let invalid_handle_null = unsafe { commy_create_mesh(ptr::null(), 0) };
    assert_eq!(invalid_handle_null.instance_id, 0);
    assert_eq!(invalid_handle_null.error_code, CommyError::InvalidParameter as i32);

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_mesh_start_stop() {
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    // Test start
    assert_eq!(
        unsafe { commy_start_mesh(handle) },
        CommyError::Success as i32
    );

    // Test is_running
    let running_status = unsafe { commy_is_mesh_running(handle) };
    assert_eq!(running_status, 1); // Should return true (1) after starting

    // Test stop
    assert_eq!(
        unsafe { commy_stop_mesh(handle) },
        CommyError::Success as i32
    );

    // Test with invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    assert_eq!(
        unsafe { commy_start_mesh(invalid_handle) },
        CommyError::InstanceNotFound as i32
    );
    assert_eq!(
        unsafe { commy_stop_mesh(invalid_handle) },
        CommyError::InstanceNotFound as i32
    );
    assert_eq!(unsafe { commy_is_mesh_running(invalid_handle) }, -1);

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_mesh_configuration() {
    clear_global_state();
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    // Test valid configuration
    let health_config = CommyHealthConfig {
        check_interval_ms: 5000,
        timeout_ms: 1000,
        max_failures: 3,
        recovery_checks: 2,
    };

    let lb_config = CommyLoadBalancerConfig {
        algorithm: CommyLoadBalancerAlgorithm::RoundRobin,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 5,
        circuit_breaker_timeout_ms: 30000,
    };

    assert_eq!(
        commy_configure_mesh(handle, &health_config, &lb_config),
        CommyError::Success as i32
    );

    // Test with null configs (should be allowed)
    assert_eq!(
        commy_configure_mesh(handle, ptr::null(), ptr::null()),
        CommyError::Success as i32
    );

    // Test invalid health config
    let invalid_health_config = CommyHealthConfig {
        check_interval_ms: 0, // Invalid
        timeout_ms: 1000,
        max_failures: 3,
        recovery_checks: 2,
    };

    assert_eq!(
        commy_configure_mesh(handle, &invalid_health_config, ptr::null()),
        CommyError::InvalidParameter as i32
    );

    // Test invalid load balancer config
    let invalid_lb_config = CommyLoadBalancerConfig {
        algorithm: CommyLoadBalancerAlgorithm::RoundRobin,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 0, // Invalid
        circuit_breaker_timeout_ms: 30000,
    };

    assert_eq!(
        commy_configure_mesh(handle, ptr::null(), &invalid_lb_config),
        CommyError::InvalidParameter as i32
    );

    // Test with invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    assert_eq!(
        commy_configure_mesh(invalid_handle, &health_config, &lb_config),
        CommyError::InstanceNotFound as i32
    );

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_service_registration() {
    clear_global_state();
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    // Test valid service registration
    let service_name = CString::new("test-service").unwrap();
    let service_id = CString::new("test-service-1").unwrap();
    let endpoint = CString::new("127.0.0.1").unwrap();
    let metadata = CString::new(r#"{"version": "1.0"}"#).unwrap();

    let service_config = CommyServiceConfig {
        service_name: service_name.as_ptr(),
        service_id: service_id.as_ptr(),
        endpoint: endpoint.as_ptr(),
        port: 8081,
        weight: 100,
        metadata: metadata.as_ptr(),
    };

    assert_eq!(
        unsafe { commy_register_service(handle, &service_config) },
        CommyError::Success as i32
    );

    // Test null config
    assert_eq!(
        unsafe { commy_register_service(handle, ptr::null()) },
        CommyError::InvalidParameter as i32
    );

    // Test invalid service config (null service name)
    let invalid_config = CommyServiceConfig {
        service_name: ptr::null(),
        service_id: service_id.as_ptr(),
        endpoint: endpoint.as_ptr(),
        port: 8081,
        weight: 100,
        metadata: metadata.as_ptr(),
    };

    assert_eq!(
        unsafe { commy_register_service(handle, &invalid_config) },
        CommyError::InvalidParameter as i32
    );

    // Test invalid port
    let invalid_port_config = CommyServiceConfig {
        service_name: service_name.as_ptr(),
        service_id: service_id.as_ptr(),
        endpoint: endpoint.as_ptr(),
        port: 0, // Invalid
        weight: 100,
        metadata: metadata.as_ptr(),
    };

    assert_eq!(
        unsafe { commy_register_service(handle, &invalid_port_config) },
        CommyError::InvalidParameter as i32
    );

    // Test invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    assert_eq!(
        unsafe { commy_register_service(invalid_handle, &service_config) },
        CommyError::InstanceNotFound as i32
    );

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_service_discovery() {
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    let service_name = CString::new("test-service").unwrap();
    let mut services: *mut CommyServiceInfo = ptr::null_mut();
    let mut count: usize = 0;

    // Test valid discovery
    assert_eq!(
        unsafe {
            commy_discover_services(handle, service_name.as_ptr(), &mut services, &mut count)
        },
        CommyError::Success as i32
    );
    assert_eq!(count, 0); // Should be 0 for now since we haven't implemented actual discovery
    assert!(services.is_null());

    // Test null parameters
    assert_eq!(
        unsafe { commy_discover_services(handle, ptr::null(), &mut services, &mut count) },
        CommyError::InvalidParameter as i32
    );

    assert_eq!(
        unsafe {
            commy_discover_services(handle, service_name.as_ptr(), ptr::null_mut(), &mut count)
        },
        CommyError::InvalidParameter as i32
    );

    assert_eq!(
        unsafe {
            commy_discover_services(
                handle,
                service_name.as_ptr(),
                &mut services,
                ptr::null_mut(),
            )
        },
        CommyError::InvalidParameter as i32
    );

    // Test invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    assert_eq!(
        unsafe {
            commy_discover_services(
                invalid_handle,
                service_name.as_ptr(),
                &mut services,
                &mut count,
            )
        },
        CommyError::InstanceNotFound as i32
    );

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_service_selection() {
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    let service_name = CString::new("test-service").unwrap();
    let mut selected_service = CommyServiceInfo {
        service_name: ptr::null_mut(),
        service_id: ptr::null_mut(),
        endpoint: ptr::null_mut(),
        port: 0,
        status: 0,
        weight: 0,
        response_time_ms: 0.0,
    };

    // Test valid selection
    assert_eq!(
        unsafe {
            commy_select_service(
                handle,
                service_name.as_ptr(),
                ptr::null(),
                &mut selected_service,
            )
        },
        CommyError::Success as i32
    );

    // Test null service name
    assert_eq!(
        unsafe { commy_select_service(handle, ptr::null(), ptr::null(), &mut selected_service) },
        CommyError::InvalidParameter as i32
    );

    // Test null selected service
    assert_eq!(
        unsafe {
            commy_select_service(handle, service_name.as_ptr(), ptr::null(), ptr::null_mut())
        },
        CommyError::InvalidParameter as i32
    );

    // Test invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    assert_eq!(
        unsafe {
            commy_select_service(
                invalid_handle,
                service_name.as_ptr(),
                ptr::null(),
                &mut selected_service,
            )
        },
        CommyError::InstanceNotFound as i32
    );

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_mesh_statistics() {
    clear_global_state();
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    let mut stats = CommyMeshStats {
        total_services: 0,
        healthy_services: 0,
        unhealthy_services: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        average_response_time_ms: 0.0,
    };

    // Test valid statistics retrieval
    assert_eq!(
        unsafe { commy_get_mesh_stats(handle, &mut stats) },
        CommyError::Success as i32
    );

    // Test null stats
    assert_eq!(
        unsafe { commy_get_mesh_stats(handle, ptr::null_mut()) },
        CommyError::InvalidParameter as i32
    );

    // Test invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    assert_eq!(
        unsafe { commy_get_mesh_stats(invalid_handle, &mut stats) },
        CommyError::InstanceNotFound as i32
    );

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_node_id_retrieval() {
    assert_eq!(commy_ffi_init(), CommyError::Success as i32);

    let node_id = CString::new("test-node-123").unwrap();
    let handle = unsafe { commy_create_mesh(node_id.as_ptr(), 8080) };
    assert_ne!(handle.instance_id, 0);

    // Test valid node ID retrieval
    let node_id_ptr = unsafe { commy_get_node_id(handle) };
    assert!(!node_id_ptr.is_null());

    unsafe {
        let retrieved_node_id = std::ffi::CStr::from_ptr(node_id_ptr);
        let node_id_str = retrieved_node_id.to_string_lossy();
        assert_eq!(node_id_str, "test-node-123");

        // Free the allocated string
        commy_free_string(node_id_ptr);
    }

    // Test invalid handle
    let invalid_handle = CommyHandle {
        instance_id: 99999,
        error_code: 0,
    };
    let invalid_node_id_ptr = unsafe { commy_get_node_id(invalid_handle) };
    assert!(invalid_node_id_ptr.is_null());

    assert_eq!(commy_ffi_cleanup(), CommyError::Success as i32);
}

#[test]
fn test_memory_management() {
    // Test memory allocation and deallocation
    let ptr = unsafe { commy_malloc(1024) };
    assert!(!ptr.is_null());
    unsafe { commy_free(ptr) };

    // Test zero allocation
    let zero_ptr = unsafe { commy_malloc(0) };
    assert!(zero_ptr.is_null());

    // Test string duplication
    let test_string = CString::new("Hello, World!").unwrap();
    let duplicated = unsafe { commy_strdup(test_string.as_ptr()) };
    assert!(!duplicated.is_null());

    unsafe {
        let dup_str = std::ffi::CStr::from_ptr(duplicated);
        assert_eq!(dup_str.to_string_lossy(), "Hello, World!");
        commy_free_string(duplicated);
    }

    // Test null string duplication
    let null_dup = unsafe { commy_strdup(ptr::null()) };
    assert!(null_dup.is_null());
}

#[test]
fn test_service_info_array_management() {
    // Test array allocation
    let array_ptr = unsafe { commy_alloc_service_info_array(5) };
    assert!(!array_ptr.is_null());
    unsafe { commy_free_service_info_array(array_ptr, 5) };

    // Test zero allocation
    let zero_array = unsafe { commy_alloc_service_info_array(0) };
    assert!(zero_array.is_null());

    // Test freeing null pointer (should not crash)
    unsafe { commy_free_service_info_array(ptr::null_mut(), 0) };
    unsafe { commy_free_service_info_array(ptr::null_mut(), 5) };
}

#[test]
fn test_error_codes() {
    // Verify error code values match expected constants
    assert_eq!(CommyError::Success as i32, 0);
    assert_eq!(CommyError::InitializationError as i32, 1);
    assert_eq!(CommyError::InvalidParameter as i32, 2);
    assert_eq!(CommyError::InstanceNotFound as i32, 3);
    assert_eq!(CommyError::AllocError as i32, 4);
}

#[test]
fn test_enums() {
    // Test service status enum
    assert_eq!(CommyServiceStatus::Unknown as i32, 0);
    assert_eq!(CommyServiceStatus::Healthy as i32, 1);
    assert_eq!(CommyServiceStatus::Unhealthy as i32, 2);
    assert_eq!(CommyServiceStatus::Degraded as i32, 3);

    // Test load balancer algorithm enum
    assert_eq!(CommyLoadBalancerAlgorithm::RoundRobin as i32, 0);
    assert_eq!(CommyLoadBalancerAlgorithm::LeastConnections as i32, 1);
    assert_eq!(CommyLoadBalancerAlgorithm::WeightedRoundRobin as i32, 2);
    assert_eq!(CommyLoadBalancerAlgorithm::PerformanceBased as i32, 3);
    assert_eq!(CommyLoadBalancerAlgorithm::Random as i32, 4);
    assert_eq!(CommyLoadBalancerAlgorithm::ConsistentHash as i32, 5);
}
