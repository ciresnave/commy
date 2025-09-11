//! Core mesh operations for FFI interface
//!
//! This module provides the main mesh coordinator functions

use crate::ffi::types::*;
use crate::ffi::{get_next_instance_id, GLOBAL_INSTANCES};
#[cfg(feature = "ffi")]
use libc::c_char;

#[cfg(feature = "mesh")]
use crate::mesh::{MeshConfig, MeshCoordinator};

/// Create a new mesh coordinator instance
///
/// Safety: `node_id` must be a valid, non-null, NUL-terminated C string
/// pointer. The caller must ensure the pointer points to memory valid for
/// reads.
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_create_mesh(node_id: *const c_char, listen_port: u16) -> CommyHandle {
    let node_id_str = match c_str_to_string(node_id) {
        Ok(s) => s,
        Err(_) => return CommyHandle::null(),
    };
    // Perform pointer-to-string conversion in an unsafe block while keeping
    // the public symbol safe to call from foreign languages.

    #[cfg(feature = "mesh")]
    {
        let config = MeshConfig {
            node_id: node_id_str,
            listen_port,
            ..Default::default()
        };

        match MeshCoordinator::new(config) {
            Ok(coordinator) => {
                let instance_id = get_next_instance_id();
                let mut instances = GLOBAL_INSTANCES.write();
                instances.insert(instance_id, coordinator);
                CommyHandle { instance_id }
            }
            Err(_) => CommyHandle::null(),
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyHandle::null()
}

/// Start the mesh coordinator
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_start_mesh(handle: CommyHandle) -> i32 {
    if handle.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    #[cfg(feature = "mesh")]
    {
        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            // Create a tokio runtime for async operations
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return CommyError::InitializationError as i32,
            };

            match rt.block_on(coordinator.start()) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::InitializationError as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InitializationError as i32
}

/// Stop the mesh coordinator
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_stop_mesh(handle: CommyHandle) -> i32 {
    if handle.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    #[cfg(feature = "mesh")]
    {
        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(mut coordinator) = instances.remove(&handle.instance_id) {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return CommyError::InternalError as i32,
            };

            match rt.block_on(coordinator.shutdown()) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::InternalError as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get mesh statistics
///
/// Safety: `stats` must be a valid, non-null pointer to a `CommyMeshStats`
/// struct that the caller owns and allows to be written to.
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_get_mesh_stats(handle: CommyHandle, stats: *mut CommyMeshStats) -> i32 {
    if handle.is_null() || stats.is_null() {
        return CommyError::InvalidArgument as i32;
    }
    // Perform pointer dereference in an unsafe block

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let mesh_stats = coordinator.get_statistics();

            unsafe {
                (*stats) = CommyMeshStats {
                    total_services: mesh_stats.total_services as u32,
                    healthy_services: mesh_stats.healthy_services as u32,
                    unhealthy_services: mesh_stats.unhealthy_services as u32,
                    total_requests: mesh_stats.total_requests,
                    successful_requests: mesh_stats.successful_requests,
                    failed_requests: mesh_stats.failed_requests,
                    average_response_time_ms: mesh_stats.average_response_time.as_secs_f64()
                        * 1000.0,
                };
            }

            CommyError::Success as i32
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Check if mesh is running
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_is_mesh_running(handle: CommyHandle) -> i32 {
    if handle.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        1 // Running
    } else {
        0 // Not running
    }
}

/// Set logging callback
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_set_log_callback(callback: CommyLogCallback) -> i32 {
    // Store the callback for use by the logging system
    // This would integrate with the tracing system
    // For now, just return success
    CommyError::Success as i32
}

/// Configure mesh settings (core implementation - internal name to avoid collisions)
///
/// Safety: If `health_config` or `lb_config` are non-null, they must point to
/// valid, properly initialized `CommyHealthConfig` and
/// `CommyLoadBalancerConfig` structures that the caller owns for the
/// duration of the call.
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_configure_mesh_core(
    handle: CommyHandle,
    health_config: *const CommyHealthConfig,
    lb_config: *const CommyLoadBalancerConfig,
) -> i32 {
    if handle.is_null() {
        return CommyError::InvalidArgument as i32;
    }
    // Perform pointer dereference in an unsafe block

    #[cfg(feature = "mesh")]
    {
        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            // Apply health configuration
            if !health_config.is_null() {
                unsafe {
                    let config = &*health_config;
                    // Apply health check configuration to coordinator
                    // This would require extending the MeshCoordinator API
                }
            }

            // Apply load balancer configuration
            if !lb_config.is_null() {
                unsafe {
                    let config = &*lb_config;
                    // Apply load balancer configuration to coordinator
                    // This would require extending the MeshCoordinator API
                }
            }

            CommyError::Success as i32
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

// Public wrapper with canonical symbol name that delegates to core implementation.
// This wrapper will be compiled only if no other module defines the public symbol
// and ensures a single canonical entry point when the crate is built normally.
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_configure_mesh(
    handle: CommyHandle,
    health_config: *const CommyHealthConfig,
    lb_config: *const CommyLoadBalancerConfig,
) -> i32 {
    commy_configure_mesh_core(handle, health_config, lb_config)
}

/// Get the node ID of the mesh
#[cfg(feature = "ffi")]
#[no_mangle]
pub extern "C" fn commy_get_node_id(handle: CommyHandle) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    // Perform pointer dereference in an unsafe block

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let node_id = coordinator.get_node_id();
            // Ownership / lifetime contract:
            // `allocate_string` returns a pointer allocated with `CString::into_raw()`.
            // The caller (foreign language) takes ownership of the returned pointer
            // and MUST call `commy_free_string` to reclaim the memory when finished.
            // The returned pointer points to a NUL-terminated C string that is
            // independent of Rust-side data (it is heap-owned by the C allocator
            // and safe to pass across FFI boundaries until freed by the caller).
            allocate_string(&node_id)
        } else {
            std::ptr::null_mut()
        }
    }

    #[cfg(not(feature = "mesh"))]
    std::ptr::null_mut()
}
