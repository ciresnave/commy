#![cfg(feature = "ffi_legacy")]
//! Working FFI implementation with real MeshCoordinator using synchronous interface
//!
//! This module provides a functional FFI layer that uses the MeshCoordinator's
//! new synchronous interface, eliminating the need for runtime.block_on() calls
//! and making C/C++ interop much more reliable.

use std::collections::HashMap;
use std::ffi::{c_char, c_uint, CStr, CString};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

use crate::error::Result;
use crate::mesh::{MeshCoordinator, MeshCoordinatorConfig};

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
/// # Safety
///
/// `node_name` and `bind_address` must be valid, non-null, NUL-terminated C
/// string pointers. The caller must ensure they point to memory valid for
/// reads and that the content is valid UTF-8 for conversion where required.
#[no_mangle]
pub unsafe extern "C" fn commy_create_mesh(
    node_name: *const c_char,
    bind_address: *const c_char,
) -> c_uint {
    let node_name_str = match unsafe { CStr::from_ptr(node_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0, // Invalid input
    };

    let bind_address_str = match unsafe { CStr::from_ptr(bind_address) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0, // Invalid input
    };

    // Create mesh configuration
    let config = MeshCoordinatorConfig {
        node_name: node_name_str.to_string(),
        node_id: Uuid::new_v4().to_string(),
        bind_address: bind_address_str.to_string(),
        ..Default::default()
    };

    // Create mesh coordinator using SYNCHRONOUS interface
    let mesh_coordinator = match MeshCoordinator::new_sync(config) {
        Ok(coordinator) => Arc::new(coordinator),
        Err(e) => {
            tracing::error!("Failed to create mesh coordinator: {}", e);
            return 0;
        }
    };

    // Get instances map
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => {
            tracing::error!("FFI not initialized - call commy_ffi_init() first");
            return 0;
        }
    };

    // Store the instance and return its ID
    let instance_id = FFI_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    {
        let mut instances_guard = instances.lock().unwrap();
        instances_guard.insert(instance_id, mesh_coordinator);
    }

    tracing::info!("Created mesh coordinator with instance ID: {}", instance_id);
    instance_id
}

/// Start a mesh coordinator using synchronous interface
#[no_mangle]
pub extern "C" fn commy_start_mesh(instance_id: c_uint) -> c_uint {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return FFIError::RuntimeError as c_uint,
    };

    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&instance_id) {
        Some(coordinator) => coordinator.clone(),
        None => return FFIError::InstanceNotFound as c_uint,
    };
    drop(instances_guard);

    // Use synchronous start method - no async runtime needed!
    match coordinator.start_sync() {
        Ok(()) => {
            tracing::info!(
                "Mesh coordinator {} started successfully (sync)",
                instance_id
            );
            FFIError::Success as c_uint
        }
        Err(e) => {
            tracing::error!("Failed to start mesh coordinator {}: {}", instance_id, e);
            FFIError::MeshError as c_uint
        }
    }
}

/// Stop a mesh coordinator using synchronous interface
#[no_mangle]
pub extern "C" fn commy_stop_mesh(instance_id: c_uint) -> c_uint {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return FFIError::RuntimeError as c_uint,
    };

    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&instance_id) {
        Some(coordinator) => coordinator.clone(),
        None => return FFIError::InstanceNotFound as c_uint,
    };
    drop(instances_guard);

    // Use synchronous stop method - no async runtime needed!
    match coordinator.stop_sync() {
        Ok(()) => {
            tracing::info!(
                "Mesh coordinator {} stopped successfully (sync)",
                instance_id
            );
            FFIError::Success as c_uint
        }
        Err(e) => {
            tracing::error!("Failed to stop mesh coordinator {}: {}", instance_id, e);
            FFIError::MeshError as c_uint
        }
    }
}

/// Check if mesh is running using synchronous interface
#[no_mangle]
pub extern "C" fn commy_is_mesh_running(instance_id: c_uint) -> c_uint {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return 0,
    };

    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&instance_id) {
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
pub extern "C" fn commy_get_node_id(instance_id: c_uint) -> *mut c_char {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return ptr::null_mut(),
    };

    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&instance_id) {
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
    pub total_messages: c_uint,
    pub messages_per_second: c_uint,
    pub average_latency_ms: c_uint,
    pub uptime_seconds: c_uint,
}

/// Get mesh statistics using synchronous interface
///
/// Safety: `stats` must be a valid, non-null pointer to an `FFIStats` struct
/// that the caller owns and allows to be written to. Passing a null or
/// invalid pointer is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_get_mesh_stats(instance_id: c_uint, stats: *mut FFIStats) -> c_uint {
    if stats.is_null() {
        return FFIError::InvalidInput as c_uint;
    }

    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return FFIError::RuntimeError as c_uint,
    };

    let instances_guard = instances.lock().unwrap();
    let coordinator = match instances_guard.get(&instance_id) {
        Some(coordinator) => coordinator.clone(),
        None => return FFIError::InstanceNotFound as c_uint,
    };
    drop(instances_guard);

    // Use synchronous get_stats - no async runtime needed!
    let mesh_stats = coordinator.get_stats_sync();

    unsafe {
        (*stats).total_nodes = mesh_stats.total_nodes as c_uint;
        (*stats).active_nodes = mesh_stats.active_nodes as c_uint;
        (*stats).total_messages = mesh_stats.total_messages as c_uint;
        (*stats).messages_per_second = mesh_stats.messages_per_second as c_uint;
        (*stats).average_latency_ms = mesh_stats.average_latency.as_millis() as c_uint;
        (*stats).uptime_seconds = mesh_stats.uptime.as_secs() as c_uint;
    }

    FFIError::Success as c_uint
}

/// Destroy a mesh coordinator instance
#[no_mangle]
pub extern "C" fn commy_destroy_mesh(instance_id: c_uint) -> c_uint {
    let instances = match FFI_INSTANCES.get() {
        Some(instances) => instances,
        None => return FFIError::RuntimeError as c_uint,
    };

    let mut instances_guard = instances.lock().unwrap();
    match instances_guard.remove(&instance_id) {
        Some(_) => {
            tracing::info!("Destroyed mesh coordinator instance: {}", instance_id);
            FFIError::Success as c_uint
        }
        None => FFIError::InstanceNotFound as c_uint,
    }
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

/// Free a string allocated by the FFI
///
/// Safety: `ptr` must either be null or a pointer previously returned by
/// an FFI allocation routine (e.g., `commy_strdup`). Passing arbitrary
/// pointers is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

// =============================================================================
// Legacy compatibility functions for existing tests
// =============================================================================

use libc::c_char as libc_c_char;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyHandle {
    pub instance_id: u64,
    pub error_code: i32,
}

#[repr(C)]
#[derive(Debug)]
pub struct CommyMeshStats {
    pub total_services: u32,
    pub healthy_services: u32,
    pub unhealthy_services: u32,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyError {
    Success = 0,
    InitializationError = 1,
    InvalidParameter = 2,
    InstanceNotFound = 3,
    AllocError = 4,
    NetworkError = 5,
}

/// Legacy compatibility function that maps to new interface
#[no_mangle]
pub extern "C" fn commy_ffi_version() -> *const libc_c_char {
    b"0.2.0-sync\0".as_ptr() as *const libc_c_char
}

/// Legacy compatibility - maps old handle-based API to new ID-based API
/// Safety: `ptr` must be a valid, non-null, NUL-terminated C string pointer.
/// The caller must ensure the pointer points to memory valid for reads.
#[no_mangle]
pub unsafe extern "C" fn commy_strdup(ptr: *const libc_c_char) -> *mut libc_c_char {
    if ptr.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        match CStr::from_ptr(ptr).to_str() {
            Ok(s) => match CString::new(s) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => ptr::null_mut(),
            },
            Err(_) => ptr::null_mut(),
        }
    }
}

/// Safety: `ptr` must either be null or a pointer previously returned from
/// an FFI allocation function owned by the caller. Converting arbitrary
/// pointers is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn commy_free(ptr: *mut std::ffi::c_void) {
    // No-op for compatibility - actual memory management handled by specific free functions
}

#[repr(C)]
pub struct CommyServiceInfo {
    pub port: u16,
}

#[no_mangle]
pub extern "C" fn commy_alloc_service_info_array(count: usize) -> *mut CommyServiceInfo {
    if count == 0 {
        return ptr::null_mut();
    }

    let layout = std::alloc::Layout::array::<CommyServiceInfo>(count).unwrap();
    unsafe {
        let ptr = std::alloc::alloc_zeroed(layout) as *mut CommyServiceInfo;
        ptr
    }
}

#[no_mangle]
pub extern "C" fn commy_free_service_info_array(ptr: *mut CommyServiceInfo, count: usize) {
    if ptr.is_null() || count == 0 {
        return;
    }

    let layout = std::alloc::Layout::array::<CommyServiceInfo>(count).unwrap();
    unsafe {
        std::alloc::dealloc(ptr as *mut u8, layout);
    }
}

#[no_mangle]
pub extern "C" fn commy_discover_services(
    _handle: CommyHandle,
    _service_name: *const libc_c_char,
    services: *mut *mut CommyServiceInfo,
    count: *mut usize,
) -> i32 {
    if services.is_null() || count.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    // For now, return empty list (can be expanded to use sync service discovery)
    unsafe {
        *services = ptr::null_mut();
        *count = 0;
    }

    CommyError::Success as i32
}
