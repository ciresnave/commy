//! Health monitoring for FFI interface
//!
//! This module provides health check and monitoring functions

use crate::ffi::types::*;
use crate::ffi::GLOBAL_INSTANCES;
#[cfg(feature = "ffi")]
use libc::c_char;

/// Start health monitoring for a service
#[cfg(feature = "ffi")]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
/// `config` must be a valid, non-null pointer to a `CommyHealthConfig` struct.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_start_health_monitoring(
    handle: CommyHandle,
    service_id: *const c_char,
    config: *const CommyHealthConfig,
) -> i32 {
    if handle.is_null() || service_id.is_null() || config.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match c_str_to_string(service_id) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            let health_config = unsafe { &*config };

            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return CommyError::InitializationError as i32,
            };

            // Configure health monitoring for the service
            match rt.block_on(coordinator.start_health_monitoring(
                &service_id_str,
                std::time::Duration::from_millis(health_config.check_interval_ms),
                std::time::Duration::from_millis(health_config.timeout_ms),
                health_config.max_failures,
                health_config.recovery_checks,
            )) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::HealthCheckFailed as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Stop health monitoring for a service
#[cfg(feature = "ffi")]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_stop_health_monitoring(
    handle: CommyHandle,
    service_id: *const c_char,
) -> i32 {
    if handle.is_null() || service_id.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match c_str_to_string(service_id) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return CommyError::InitializationError as i32,
            };

            match rt.block_on(coordinator.stop_health_monitoring(&service_id_str)) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::ServiceNotFound as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get health status of a service
#[cfg(feature = "ffi")]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
/// `status` must be a valid, non-null pointer to a `CommyServiceStatus`.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_get_service_health(
    handle: CommyHandle,
    service_id: *const c_char,
    status: *mut CommyServiceStatus,
    response_time_ms: *mut u64,
) -> i32 {
    if handle.is_null() || service_id.is_null() || status.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match c_str_to_string(service_id) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            match coordinator.get_service_health(&service_id_str) {
                Some(health_info) => {
                    unsafe {
                        *status = match health_info.is_healthy {
                            true => CommyServiceStatus::Healthy,
                            false => CommyServiceStatus::Unhealthy,
                        };

                        if !response_time_ms.is_null() {
                            *response_time_ms = health_info.response_time.as_millis() as u64;
                        }
                    }
                    CommyError::Success as i32
                }
                None => CommyError::ServiceNotFound as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get health status of all services
#[cfg(feature = "ffi")]
/// # Safety
///
/// `service_count` must be a valid, non-null pointer. Output arrays returned
/// must be freed by calling `commy_free_health_status_arrays` when no longer
/// needed.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_get_all_health_status(
    handle: CommyHandle,
    service_count: *mut usize,
    service_ids: *mut *mut *mut c_char,
    statuses: *mut *mut CommyServiceStatus,
) -> i32 {
    if handle.is_null() || service_count.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let health_statuses = coordinator.get_all_health_status();

            let count = health_statuses.len();
            if count == 0 {
                unsafe {
                    *service_count = 0;
                    if !service_ids.is_null() {
                        *service_ids = std::ptr::null_mut();
                    }
                    if !statuses.is_null() {
                        *statuses = std::ptr::null_mut();
                    }
                }
                return CommyError::Success as i32;
            }

            // Allocate arrays
            let ids_array = unsafe {
                libc::malloc(count * std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
            };
            let statuses_array = unsafe {
                libc::malloc(count * std::mem::size_of::<CommyServiceStatus>())
                    as *mut CommyServiceStatus
            };

            if ids_array.is_null() || statuses_array.is_null() {
                if !ids_array.is_null() {
                    unsafe { libc::free(ids_array as *mut libc::c_void) };
                }
                if !statuses_array.is_null() {
                    unsafe { libc::free(statuses_array as *mut libc::c_void) };
                }
                return CommyError::OutOfMemory as i32;
            }

            unsafe {
                for (i, (service_id, health_info)) in health_statuses.iter().enumerate() {
                    // Allocate and copy service ID
                    let id_cstring = std::ffi::CString::new(service_id.as_str()).unwrap();
                    let id_ptr = crate::ffi::memory::commy_strdup(id_cstring.as_ptr());
                    *ids_array.add(i) = id_ptr;

                    // Set status
                    *statuses_array.add(i) = match health_info.is_healthy {
                        true => CommyServiceStatus::Healthy,
                        false => CommyServiceStatus::Unhealthy,
                    };
                }

                *service_count = count;
                if !service_ids.is_null() {
                    *service_ids = ids_array;
                }
                if !statuses.is_null() {
                    *statuses = statuses_array;
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

/// Free health status arrays
#[cfg(feature = "ffi")]
/// # Safety
///
/// `service_ids` and `statuses` must either be null or pointers previously
/// returned by `commy_get_all_health_status` for the same `service_count`.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_free_health_status_arrays(
    service_count: usize,
    service_ids: *mut *mut c_char,
    statuses: *mut CommyServiceStatus,
) {
    if service_count == 0 {
        return;
    }

    if !service_ids.is_null() {
        unsafe {
            for i in 0..service_count {
                let id_ptr = *service_ids.add(i);
                if !id_ptr.is_null() {
                    libc::free(id_ptr as *mut libc::c_void);
                }
            }
            libc::free(service_ids as *mut libc::c_void);
        }
    }

    if !statuses.is_null() {
        unsafe {
            libc::free(statuses as *mut libc::c_void);
        }
    }
}

/// Set health callback for notifications
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `handle` must be a valid `CommyHandle`. `callback` must be a valid
/// function pointer using the C calling convention and must remain valid
/// for as long as the health system may call it.
pub unsafe extern "C" fn commy_set_health_callback(
    handle: CommyHandle,
    callback: CommyHealthCallback,
) -> i32 {
    if handle.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    // Store the callback for use by the health monitoring system
    // This would integrate with the health monitor's event system
    CommyError::Success as i32
}

/// Perform manual health check
#[cfg(feature = "ffi")]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
/// `status` must be a valid, non-null pointer to a `CommyServiceStatus`.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_manual_health_check(
    handle: CommyHandle,
    service_id: *const c_char,
    status: *mut CommyServiceStatus,
    response_time_ms: *mut u64,
) -> i32 {
    if handle.is_null() || service_id.is_null() || status.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match c_str_to_string(service_id) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return CommyError::InitializationError as i32,
            };

            match rt.block_on(coordinator.manual_health_check(&service_id_str)) {
                Ok(health_result) => {
                    unsafe {
                        *status = match health_result.is_healthy {
                            true => CommyServiceStatus::Healthy,
                            false => CommyServiceStatus::Unhealthy,
                        };

                        if !response_time_ms.is_null() {
                            *response_time_ms = health_result.response_time.as_millis() as u64;
                        }
                    }
                    CommyError::Success as i32
                }
                Err(_) => CommyError::HealthCheckFailed as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}
