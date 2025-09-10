//! Working FFI implementation with real MeshCoordinator using synchronous interface
//!
//! This module provides a functional FFI layer that uses the MeshCoordinator's
//! new synchronous interface, eliminating the need for runtime.block_on() calls
//! and making C/C++ interop much more reliable.

use std::collections::HashMap;
use std::ffi::{c_char, c_uint, CStr, CString};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use uuid::Uuid;

use crate::config::MeshConfiguration;
use crate::manager::core::ManagerConfig;
use crate::mesh::{
    HealthMonitorConfig, LoadBalancerConfig, MeshCoordinator, MeshCoordinatorConfig,
};

// Use canonical FFI types from types.rs for compatibility with examples/tests
use crate::ffi::types::{
    CommyHandle as FfiCommyHandle, CommyHealthConfig as FfiCommyHealthConfig,
    CommyLoadBalancerConfig as FfiCommyLoadBalancerConfig, CommyMeshStats as FfiCommyMeshStats,
    CommyServiceConfig as FfiCommyServiceConfig, CommyServiceInfo as FfiCommyServiceInfo,
};

// Alias local names to the canonical ones used by examples
type CommyHandle = FfiCommyHandle;
type CommyMeshStats = FfiCommyMeshStats;
type CommyServiceInfo = FfiCommyServiceInfo;
type CommyServiceConfig = FfiCommyServiceConfig;
type CommyHealthConfig = FfiCommyHealthConfig;
type CommyLoadBalancerConfig = FfiCommyLoadBalancerConfig;

/// Legacy compatibility error codes used by older examples/tests
#[repr(i32)]
pub enum LegacyCommyError {
    Success = 0,
    InvalidParameter = 1,
    InstanceNotFound = 2,
    MeshError = 3,
    RuntimeError = 4,
}

// Global FFI state
static FFI_INSTANCES: OnceLock<Mutex<HashMap<u32, Arc<MeshCoordinator>>>> = OnceLock::new();
static FFI_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// FFI error codes
#[repr(C)]
pub enum FFIError {
    Success = 0,
    InvalidInput = 1,
    InstanceNotFound = 2,
    MeshError = 3,
    RuntimeError = 4,
}

/// Initialize the FFI subsystem - MUST be called before any other FFI functions
#[no_mangle]
pub extern "C" fn commy_ffi_init() -> c_uint {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    // Initialize instances map (will only happen once)
    let _instances = FFI_INSTANCES.get_or_init(|| Mutex::new(HashMap::new()));

    tracing::info!("Commy FFI initialized successfully");
    FFIError::Success as c_uint
}

/// Create a new mesh coordinator instance using synchronous interface
///
/// # Safety
///
/// `node_id` must be a valid, non-null, NUL-terminated C string pointer.
/// The caller must ensure the pointer points to a valid UTF-8 sequence if a
/// valid node id is expected. Invalid pointers or non-UTF-8 input may lead to
/// undefined behavior or an error return (null handle).
#[no_mangle]
pub unsafe extern "C" fn commy_create_mesh(
    node_id: *const c_char,
    listen_port: u16,
) -> CommyHandle {
    // Validate node_id pointer before converting to Rust string
    if node_id.is_null() {
        tracing::warn!("commy_create_mesh called with null node_id");
        return CommyHandle::null();
    }

    let node_id_str = match unsafe { CStr::from_ptr(node_id) }.to_str() {
        Ok(s) if !s.is_empty() => s,
        _ => return CommyHandle::null(), // Invalid input or empty
    };

    let bind_address_str = format!("127.0.0.1:{}", listen_port);

    // Create mesh configuration with proper structure
    let config = MeshCoordinatorConfig {
        node_id: Uuid::new_v4(),
        node_name: node_id_str.to_string(),
        mesh_config: MeshConfiguration::default(),
        manager_config: ManagerConfig {
            bind_address: bind_address_str,
            listen_port,
            ..Default::default()
        },
        load_balancer_config: LoadBalancerConfig::default(),
        health_monitor_config: HealthMonitorConfig::default(),
        sync_interval: Duration::from_secs(10),
        node_timeout: Duration::from_secs(30),
    };

    // Create mesh coordinator using SYNCHRONOUS interface
    let mesh_coordinator = match MeshCoordinator::new_sync(config) {
        Ok(coordinator) => Arc::new(coordinator),
        Err(e) => {
            tracing::error!("Failed to create mesh coordinator: {}", e);
            return CommyHandle::null();
        }
    };

    // Get instances map
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => {
            tracing::error!("FFI not initialized - call commy_ffi_init() first");
            return CommyHandle::null();
        }
    };

    // Store the instance and return its ID
    let instance_id = FFI_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    {
        let mut instances_guard = instances.lock().unwrap();
        instances_guard.insert(instance_id, mesh_coordinator);
    }

    tracing::info!("Created mesh coordinator with instance ID: {}", instance_id);
    CommyHandle {
        instance_id: instance_id as u64,
    }
}

/// Start a mesh coordinator using synchronous interface
#[no_mangle]
pub extern "C" fn commy_start_mesh(handle: CommyHandle) -> i32 {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return LegacyCommyError::RuntimeError as i32,
    };

    let id = handle.instance_id as u32;
    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&id) {
        Some(coordinator) => coordinator.clone(),
        None => return LegacyCommyError::InstanceNotFound as i32,
    };
    drop(instances_guard);

    // Use synchronous start method - no async runtime needed!
    match coordinator.start_sync() {
        Ok(()) => {
            tracing::info!("Mesh coordinator {} started successfully (sync)", id);
            LegacyCommyError::Success as i32
        }
        Err(_e) => {
            tracing::error!("Failed to start mesh coordinator {}", id);
            LegacyCommyError::MeshError as i32
        }
    }
}

/// Stop a mesh coordinator using synchronous interface
#[no_mangle]
pub extern "C" fn commy_stop_mesh(handle: CommyHandle) -> i32 {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return LegacyCommyError::RuntimeError as i32,
    };

    let id = handle.instance_id as u32;
    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&id) {
        Some(coordinator) => coordinator.clone(),
        None => return LegacyCommyError::InstanceNotFound as i32,
    };
    drop(instances_guard);

    // Use synchronous stop method - no async runtime needed!
    match coordinator.stop_sync() {
        Ok(()) => {
            tracing::info!("Mesh coordinator {} stopped successfully (sync)", id);
            LegacyCommyError::Success as i32
        }
        Err(_e) => {
            tracing::error!("Failed to stop mesh coordinator {}", id);
            LegacyCommyError::MeshError as i32
        }
    }
}

/// Check if mesh is running using synchronous interface
#[no_mangle]
pub extern "C" fn commy_is_mesh_running(handle: CommyHandle) -> c_uint {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return 0,
    };

    let id = handle.instance_id as u32;
    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&id) {
        Some(coordinator) => coordinator.clone(),
        None => return 0,
    };
    drop(instances_guard);

    // Use synchronous is_running check
    if coordinator.is_running_sync() {
        1
    } else {
        0
    }
}

/// Get node ID using synchronous interface
#[no_mangle]
pub extern "C" fn commy_get_node_id(handle: CommyHandle) -> *mut c_char {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return ptr::null_mut(),
    };

    let id = handle.instance_id as u32;
    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&id) {
        Some(coordinator) => coordinator.clone(),
        None => return ptr::null_mut(),
    };
    drop(instances_guard);

    // Use synchronous get_node_id
    let node_id = coordinator.get_node_id_sync();
    match CString::new(node_id) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// FFI-compatible stats structure
#[repr(C)]
pub struct FFIStats {
    pub total_nodes: c_uint,
    pub active_nodes: c_uint,
    pub total_services: c_uint,
    pub healthy_services: c_uint,
    pub total_requests: c_uint,
    pub uptime_seconds: c_uint,
}

/// Get mesh statistics using synchronous interface
///
/// # Safety
///
/// `stats` must be a valid, non-null pointer to a `CommyMeshStats` structure
/// that the caller owns and is allowed to be written to. The function will write
/// fields into the provided struct.
#[no_mangle]
/// # Safety
///
/// `node_id` must be a valid, non-null, NUL-terminated C string pointer. The
/// caller must ensure the pointer points to a valid UTF-8 sequence if a valid
/// node id is expected. Invalid pointers or non-UTF-8 input may lead to undefined
/// behavior or an error return (null handle).
pub unsafe extern "C" fn commy_get_mesh_stats(
    handle: CommyHandle,
    stats: *mut CommyMeshStats,
) -> i32 {
    if stats.is_null() {
        return LegacyCommyError::InvalidParameter as i32;
    }

    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return LegacyCommyError::RuntimeError as i32,
    };

    // Safety: `stats` must be a valid, non-null pointer to a `CommyMeshStats` structure
    // that the caller owns and is allowed to be written to. The function will write
    // fields into the provided struct.
    let id = handle.instance_id as u32;
    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&id) {
        Some(coordinator) => coordinator.clone(),
        None => return LegacyCommyError::InstanceNotFound as i32,
    };
    drop(instances_guard);

    // Use synchronous get_stats - no async runtime needed!
    let mesh_stats = coordinator.get_stats_sync();

    unsafe {
        (*stats).total_services = mesh_stats.total_services as u32;
        (*stats).healthy_services = mesh_stats.healthy_services as u32;
        // MeshCoordinatorStats doesn't track unhealthy_services separately - infer
        (*stats).unhealthy_services = (mesh_stats
            .total_services
            .saturating_sub(mesh_stats.healthy_services))
            as u32;
        (*stats).total_requests = mesh_stats.total_requests;
        // Successful/failed requests are not tracked independently yet - set to 0
        (*stats).successful_requests = 0;
        (*stats).failed_requests = 0;
        // avg_response_time_us stored in microseconds - convert to milliseconds as f64
        (*stats).average_response_time_ms = mesh_stats.avg_response_time_us / 1000.0;
    }

    LegacyCommyError::Success as i32
}

/// Destroy a mesh coordinator instance
#[no_mangle]
pub extern "C" fn commy_destroy_mesh(handle: CommyHandle) -> i32 {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return LegacyCommyError::RuntimeError as i32,
    };

    let id = handle.instance_id as u32;
    let mut instances_guard = instances.lock().unwrap();
    match instances_guard.remove(&id) {
        Some(_) => {
            tracing::info!("Destroyed mesh coordinator instance: {}", id);
            LegacyCommyError::Success as i32
        }
        None => LegacyCommyError::InstanceNotFound as i32,
    }
}

/// Configure mesh (legacy compatibility stub)
///
/// # Safety
///
/// If `_health` or `_lb` are non-null they must point to valid
/// `CommyHealthConfig` / `CommyLoadBalancerConfig` structures that the
/// caller owns for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn commy_configure_mesh(
    _handle: CommyHandle,
    _health: *const CommyHealthConfig,
    _lb: *const CommyLoadBalancerConfig,
) -> i32 {
    if _health.is_null() || _lb.is_null() {
        return LegacyCommyError::InvalidParameter as i32;
    }
    // Currently a stub that accepts configuration and logs it
    LegacyCommyError::Success as i32
}

/// Select a service (legacy compatibility stub)
///
/// # Safety
///
/// `_service_name` and `_client_id`, if non-null, must be valid
/// NUL-terminated C string pointers. `_out_service` must be a valid non-null
/// pointer to a `CommyServiceInfo` struct that the caller owns and allows to
/// be written to.
#[no_mangle]
pub unsafe extern "C" fn commy_select_service(
    _handle: CommyHandle,
    _service_name: *const c_char,
    _client_id: *const c_char,
    _out_service: *mut CommyServiceInfo,
) -> i32 {
    if _out_service.is_null() || _service_name.is_null() {
        return LegacyCommyError::InvalidParameter as i32;
    }

    // For now, return InvalidParameter to indicate not-implemented selection logic
    LegacyCommyError::InvalidParameter as i32
}

/// Cleanup FFI resources
#[no_mangle]
pub extern "C" fn commy_ffi_cleanup() -> c_uint {
    if let Some(instances) = FFI_INSTANCES.get() {
        let mut instances_guard = instances.lock().unwrap();
        instances_guard.clear();
    }

    tracing::info!("Commy FFI cleaned up");
    FFIError::Success as c_uint
}

// =============================================================================
// Service Registration Functions for C++ SDK compatibility
// =============================================================================

use libc::c_char as libc_c_char;

/// Register a service using handle-based API (for C++ SDK compatibility)
///
/// # Safety
///
/// `config` must be a valid, non-null pointer to a `CommyServiceConfig`
/// structure. Any pointer fields inside the config that are dereferenced must
/// also obey the usual C string invariants (non-null, NUL-terminated, valid
/// UTF-8 when interpreted as Rust strings).
#[no_mangle]
pub unsafe extern "C" fn commy_register_service(
    _handle: CommyHandle,
    config: *const CommyServiceConfig,
) -> i32 {
    if config.is_null() {
        return LegacyCommyError::InvalidParameter as i32;
    }

    unsafe {
        let service_name = if (*config).service_name.is_null() {
            "unnamed_service"
        } else {
            match CStr::from_ptr((*config).service_name).to_str() {
                Ok(s) => s,
                Err(_) => return LegacyCommyError::InvalidParameter as i32,
            }
        };

        let service_id = if (*config).service_id.is_null() {
            "unknown_id"
        } else {
            match CStr::from_ptr((*config).service_id).to_str() {
                Ok(s) => s,
                Err(_) => return LegacyCommyError::InvalidParameter as i32,
            }
        };

        tracing::info!(
            "Service registration request: {} (id: {}) on port {}",
            service_name,
            service_id,
            (*config).port
        );
    }

    // For now, just log the registration - real implementation would use coordinator
    LegacyCommyError::Success as i32
}

/// Unregister a service using handle-based API (for C++ SDK compatibility)
///
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string
/// pointer. The caller must ensure it points to valid memory for the duration
/// of the call.
#[no_mangle]
pub unsafe extern "C" fn commy_unregister_service(
    _handle: CommyHandle,
    service_id: *const c_char,
) -> i32 {
    if service_id.is_null() {
        return LegacyCommyError::InvalidParameter as i32;
    }

    unsafe {
        let service_id_str = match CStr::from_ptr(service_id).to_str() {
            Ok(s) => s,
            Err(_) => return LegacyCommyError::InvalidParameter as i32,
        };

        tracing::info!("Service unregistration request: {}", service_id_str);
    }

    // For now, just log the unregistration - real implementation would use coordinator
    LegacyCommyError::Success as i32
}

// =============================================================================
// Legacy compatibility functions for existing tests
// =============================================================================

/// Legacy compatibility function that maps to new interface
#[no_mangle]
#[allow(clippy::manual_c_str_literals)]
pub extern "C" fn commy_ffi_version() -> *const libc_c_char {
    b"0.2.0-sync\0".as_ptr() as *const libc_c_char
}

/// Legacy compatibility - maps old handle-based API to new ID-based API
///
/// # Safety
///
/// `services` and `count` must be valid, non-null pointers that the
/// caller owns and allows to be written to. If `_service_name` is dereferenced
/// it must be a valid, non-null, NUL-terminated C string pointer.
#[no_mangle]
pub unsafe extern "C" fn commy_discover_services(
    _handle: CommyHandle,
    _service_name: *const libc_c_char,
    services: *mut *mut CommyServiceInfo,
    count: *mut usize,
) -> i32 {
    if services.is_null() || count.is_null() {
        return LegacyCommyError::InvalidParameter as i32;
    }

    // For now, return empty list (can be expanded to use sync service discovery)
    unsafe {
        *services = ptr::null_mut();
        *count = 0;
    }

    LegacyCommyError::Success as i32
}
