//! Real FFI implementation with actual mesh coordinator functionality
//!
//! This module provides a working FFI layer that uses the real Commy mesh
//! components instead of mock implementations.

use crate::ffi::types::*;
use libc::c_char;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use crate::config::MeshConfiguration;
#[cfg(feature = "mesh")]
use crate::mesh::{MeshCoordinator, MeshCoordinatorConfig, NodeStatus};

// Global storage for mesh coordinators
static GLOBAL_MESH_INSTANCES: Lazy<RwLock<HashMap<u64, Arc<MeshCoordinator>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

// Global runtime for async operations
static GLOBAL_RUNTIME: Lazy<Mutex<Runtime>> =
    Lazy::new(|| Mutex::new(Runtime::new().expect("Failed to create Tokio runtime for FFI")));

// Instance ID counter
static NEXT_INSTANCE_ID: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(1));

fn get_next_instance_id() -> u64 {
    let mut next_id = NEXT_INSTANCE_ID.lock().unwrap();
    let id = *next_id;
    *next_id += 1;
    id
}

fn c_str_to_string(ptr: *const c_char) -> Result<String, std::ffi::IntoStringError> {
    if ptr.is_null() {
        return Err(std::ffi::IntoStringError::from(CString::new("").unwrap()));
    }

    unsafe { CStr::from_ptr(ptr).to_owned().into_string() }
}

/// Initialize the FFI layer
#[no_mangle]
pub extern "C" fn commy_ffi_init() -> i32 {
    // Initialize logging
    if let Err(_) = tracing_subscriber::fmt::try_init() {
        // Already initialized, that's fine
    }

    // Verify runtime is available
    match GLOBAL_RUNTIME.try_lock() {
        Some(_) => 0, // Success
        None => -1,   // Runtime locked/unavailable
    }
}

/// Get FFI version
#[no_mangle]
pub extern "C" fn commy_ffi_version() -> *const c_char {
    b"0.1.0-real\0".as_ptr() as *const c_char
}

/// Create a real mesh coordinator instance
///
/// # Safety
///
/// `node_id` must be a valid, non-null, NUL-terminated C string pointer. The
/// caller must ensure the pointer points to memory valid for reads.
#[no_mangle]
pub unsafe extern "C" fn commy_create_mesh(
    node_id: *const c_char,
    listen_port: u16,
) -> CommyHandle {
    // Validate parameters
    if node_id.is_null() {
        return CommyHandle {
            instance_id: 0,
            error_code: CommyError::InvalidParameter as i32,
        };
    }

    let node_id_str = match c_str_to_string(node_id) {
        Ok(s) if !s.is_empty() => s,
        _ => {
            return CommyHandle {
                instance_id: 0,
                error_code: CommyError::InvalidParameter as i32,
            };
        }
    };

    if listen_port == 0 {
        return CommyHandle {
            instance_id: 0,
            error_code: CommyError::InvalidParameter as i32,
        };
    }

    #[cfg(feature = "mesh")]
    {
        // Get the runtime and create the mesh coordinator
        let runtime = match GLOBAL_RUNTIME.try_lock() {
            Some(rt) => rt,
            None => {
                return CommyHandle {
                    instance_id: 0,
                    error_code: CommyError::InitializationError as i32,
                };
            }
        };

        // Create mesh configuration
        let config = MeshConfig {
            node_id: node_id_str,
            listen_port,
            ..Default::default()
        };

        // Create the mesh coordinator using the runtime
        let coordinator_result = runtime.block_on(async { MeshCoordinator::new(config).await });

        match coordinator_result {
            Ok(coordinator) => {
                let instance_id = get_next_instance_id();
                let coordinator_arc = Arc::new(coordinator);

                // Store the coordinator
                {
                    let mut instances = GLOBAL_MESH_INSTANCES.write();
                    instances.insert(instance_id, coordinator_arc);
                }

                CommyHandle {
                    instance_id,
                    error_code: CommyError::Success as i32,
                }
            }
            Err(_) => CommyHandle {
                instance_id: 0,
                error_code: CommyError::InitializationError as i32,
            },
        }
    }

    #[cfg(not(feature = "mesh"))]
    {
        CommyHandle {
            instance_id: 0,
            error_code: CommyError::InitializationError as i32,
        }
    }
}

/// Start the mesh coordinator
///
/// # Safety
///
/// `handle` must refer to a valid mesh instance previously returned by
/// `commy_create_mesh`. The caller must ensure the instance is not
/// concurrently destroyed while this call executes. Passing an invalid or
/// stale handle is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_start_mesh(handle: CommyHandle) -> i32 {
    if handle.instance_id == 0 {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_MESH_INSTANCES.read();
    let coordinator = match instances.get(&handle.instance_id) {
        Some(coord) => coord.clone(),
        None => return CommyError::InstanceNotFound as i32,
    };
    drop(instances);

    // Start the coordinator using the runtime
    let runtime = match GLOBAL_RUNTIME.try_lock() {
        Some(rt) => rt,
        None => return CommyError::InitializationError as i32,
    };

    match runtime.block_on(async { coordinator.start().await }) {
        Ok(_) => CommyError::Success as i32,
        Err(_) => CommyError::InitializationError as i32,
    }
}

/// Stop the mesh coordinator
///
/// # Safety
///
/// `handle` must refer to a valid mesh instance previously returned by
/// `commy_create_mesh`. Callers must ensure the instance is not accessed
/// concurrently while this call executes. Passing an invalid or stale handle
/// is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_stop_mesh(handle: CommyHandle) -> i32 {
    if handle.instance_id == 0 {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_MESH_INSTANCES.read();
    let coordinator = match instances.get(&handle.instance_id) {
        Some(coord) => coord.clone(),
        None => return CommyError::InstanceNotFound as i32,
    };
    drop(instances);

    // Stop the coordinator using the runtime
    let runtime = match GLOBAL_RUNTIME.try_lock() {
        Some(rt) => rt,
        None => return CommyError::InitializationError as i32,
    };

    match runtime.block_on(async { coordinator.stop().await }) {
        Ok(_) => CommyError::Success as i32,
        Err(_) => CommyError::InitializationError as i32,
    }
}

/// Check if mesh is running
///
/// # Safety
///
/// `handle` must refer to a valid mesh instance previously returned by
/// `commy_create_mesh`. The caller must ensure the handle remains valid for
/// the duration of the call; racing with instance teardown may produce
/// undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_is_mesh_running(handle: CommyHandle) -> i32 {
    if handle.instance_id == 0 {
        return -1; // Invalid handle
    }

    let instances = GLOBAL_MESH_INSTANCES.read();
    let coordinator = match instances.get(&handle.instance_id) {
        Some(coord) => coord.clone(),
        None => return -1, // Instance not found
    };
    drop(instances);

    // Check if running using the runtime
    let runtime = match GLOBAL_RUNTIME.try_lock() {
        Some(rt) => rt,
        None => return -1,
    };

    match runtime.block_on(async { coordinator.is_running().await }) {
        Ok(true) => 1,  // Running
        Ok(false) => 0, // Not running
        Err(_) => -1,   // Error
    }
}

/// Get node ID
///
/// # Safety
///
/// `handle` must refer to a valid, non-null `CommyHandle` previously returned
/// by `commy_create_mesh`. The returned pointer, if non-null, points to a
/// NUL-terminated C string allocated by this library and the caller is
/// responsible for freeing it with `commy_free_string` when no longer needed.
#[no_mangle]
pub unsafe extern "C" fn commy_get_node_id(handle: CommyHandle) -> *mut c_char {
    if handle.instance_id == 0 {
        return std::ptr::null_mut();
    }

    let instances = GLOBAL_MESH_INSTANCES.read();
    let coordinator = match instances.get(&handle.instance_id) {
        Some(coord) => coord.clone(),
        None => return std::ptr::null_mut(),
    };
    drop(instances);

    // Get node ID
    let node_id = coordinator.get_node_id();

    match CString::new(node_id) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get mesh statistics
///
/// # Safety
///
/// `stats` must be a valid, non-null pointer to a `CommyMeshStats` struct that
/// the caller owns and allows to be written to. Passing a null or invalid
/// pointer is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_get_mesh_stats(
    handle: CommyHandle,
    stats: *mut CommyMeshStats,
) -> i32 {
    if handle.instance_id == 0 || stats.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_MESH_INSTANCES.read();
    let coordinator = match instances.get(&handle.instance_id) {
        Some(coord) => coord.clone(),
        None => return CommyError::InstanceNotFound as i32,
    };
    drop(instances);

    // Get statistics using the runtime
    let runtime = match GLOBAL_RUNTIME.try_lock() {
        Some(rt) => rt,
        None => return CommyError::InitializationError as i32,
    };

    let mesh_stats = match runtime.block_on(async { coordinator.get_stats().await }) {
        Ok(stats) => stats,
        Err(_) => return CommyError::InitializationError as i32,
    };

    // Convert to FFI stats
    unsafe {
        (*stats).total_services = mesh_stats.total_nodes as u32;
        (*stats).healthy_services = mesh_stats.active_nodes as u32;
        (*stats).unhealthy_services = (mesh_stats.total_nodes - mesh_stats.active_nodes) as u32;
        (*stats).total_requests = mesh_stats.total_requests;
        (*stats).successful_requests = mesh_stats.successful_requests;
        (*stats).failed_requests = mesh_stats.failed_requests;
        (*stats).average_response_time_ms = mesh_stats.average_response_time_ms;
    }

    CommyError::Success as i32
}

/// Cleanup FFI layer
#[no_mangle]
pub extern "C" fn commy_ffi_cleanup() -> i32 {
    // Stop all mesh instances
    let mut instances = GLOBAL_MESH_INSTANCES.write();

    let runtime = match GLOBAL_RUNTIME.try_lock() {
        Some(rt) => rt,
        None => return CommyError::InitializationError as i32,
    };

    for (_id, coordinator) in instances.drain() {
        let _ = runtime.block_on(async { coordinator.stop().await });
    }

    CommyError::Success as i32
}

/// Free a string allocated by FFI
///
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by an FFI
/// allocation routine. Passing arbitrary pointers is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
