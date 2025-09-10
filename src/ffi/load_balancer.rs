//! Load balancer configuration for FFI interface
//!
//! This module provides load balancing configuration and selection functions

use crate::ffi::types::*;
use crate::ffi::GLOBAL_INSTANCES;
#[cfg(feature = "ffi")]
use libc::c_char;

#[cfg(feature = "mesh")]
use crate::mesh::load_balancer::{CircuitBreakerConfig, LoadBalancerAlgorithm, LoadBalancerConfig};

/// Configure load balancer
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `config` must be a valid, non-null pointer to a `CommyLoadBalancerConfig`
/// structure. Any pointers inside the config must be valid, NUL-terminated
/// C strings that the caller owns for the duration of the call.
pub unsafe extern "C" fn commy_configure_load_balancer(
    handle: CommyHandle,
    config: *const CommyLoadBalancerConfig,
) -> i32 {
    if handle.is_null() || config.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    #[cfg(feature = "mesh")]
    {
        let lb_config = unsafe { &*config };

        let algorithm = match lb_config.algorithm {
            CommyLoadBalancerAlgorithm::RoundRobin => LoadBalancerAlgorithm::RoundRobin,
            CommyLoadBalancerAlgorithm::LeastConnections => LoadBalancerAlgorithm::LeastConnections,
            CommyLoadBalancerAlgorithm::WeightedRoundRobin => {
                LoadBalancerAlgorithm::WeightedRoundRobin
            }
            CommyLoadBalancerAlgorithm::PerformanceBased => LoadBalancerAlgorithm::PerformanceBased,
            CommyLoadBalancerAlgorithm::Random => LoadBalancerAlgorithm::Random,
            CommyLoadBalancerAlgorithm::ConsistentHash => LoadBalancerAlgorithm::ConsistentHash,
        };

        let circuit_breaker = if lb_config.enable_circuit_breaker {
            Some(CircuitBreakerConfig {
                failure_threshold: lb_config.circuit_breaker_threshold,
                timeout: std::time::Duration::from_millis(lb_config.circuit_breaker_timeout_ms),
                recovery_timeout: std::time::Duration::from_millis(
                    lb_config.circuit_breaker_timeout_ms,
                ),
            })
        } else {
            None
        };

        let load_balancer_config = LoadBalancerConfig {
            algorithm,
            circuit_breaker,
            health_check_enabled: true,
            performance_weight: 1.0,
        };

        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            match coordinator.configure_load_balancer(load_balancer_config) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::LoadBalancerError as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Select a service using load balancer
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_name` must be a valid, non-null, NUL-terminated C string pointer.
/// `client_id`, if non-null, must also be a valid NUL-terminated C string
/// pointer. `selected_service` must be a valid, non-null pointer to a
/// `CommyServiceInfo` struct that the caller owns and allows to be written to.
pub unsafe extern "C" fn commy_select_service(
    handle: CommyHandle,
    service_name: *const c_char,
    client_id: *const c_char,
    selected_service: *mut CommyServiceInfo,
) -> i32 {
    if handle.is_null() || service_name.is_null() || selected_service.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_name_str = match c_str_to_string(service_name) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    let client_id_str = c_str_to_option_string(client_id);

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            match coordinator.select_service(&service_name_str, client_id_str.as_deref()) {
                Ok(Some(service)) => {
                    unsafe {
                        *selected_service = crate::ffi::memory::create_service_info_with_alloc(
                            &service.service_name,
                            &service.service_id,
                            &service.endpoint,
                            service.port,
                            CommyServiceStatus::Healthy, // Would need to check actual status
                            service.weight,
                            0, // Would need to get actual response time
                        );
                    }
                    CommyError::Success as i32
                }
                Ok(None) => CommyError::ServiceNotFound as i32,
                Err(_) => CommyError::LoadBalancerError as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get load balancer statistics
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_name` must be a valid, non-null, NUL-terminated C string pointer.
/// Any output pointers provided (total_requests, successful_requests, failed_requests,
/// average_response_time_ms) must be valid writable pointers or null.
pub unsafe extern "C" fn commy_get_load_balancer_stats(
    handle: CommyHandle,
    service_name: *const c_char,
    total_requests: *mut u64,
    successful_requests: *mut u64,
    failed_requests: *mut u64,
    average_response_time_ms: *mut f64,
) -> i32 {
    if handle.is_null() || service_name.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_name_str = match c_str_to_string(service_name) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let stats = coordinator.get_load_balancer_stats(&service_name_str);

            if !total_requests.is_null() {
                unsafe {
                    *total_requests = stats.total_requests;
                }
            }
            if !successful_requests.is_null() {
                unsafe {
                    *successful_requests = stats.successful_requests;
                }
            }
            if !failed_requests.is_null() {
                unsafe {
                    *failed_requests = stats.failed_requests;
                }
            }
            if !average_response_time_ms.is_null() {
                unsafe {
                    *average_response_time_ms = stats.average_response_time.as_secs_f64() * 1000.0;
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

/// Report service performance (for performance-based load balancing)
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
pub unsafe extern "C" fn commy_report_service_performance(
    handle: CommyHandle,
    service_id: *const c_char,
    response_time_ms: u64,
    success: bool,
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
            let response_time = std::time::Duration::from_millis(response_time_ms);

            match coordinator.report_service_performance(&service_id_str, response_time, success) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::LoadBalancerError as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get circuit breaker status
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
/// `is_open` and `failure_count` may be null; if non-null they must be valid
/// writable pointers.
pub unsafe extern "C" fn commy_get_circuit_breaker_status(
    handle: CommyHandle,
    service_id: *const c_char,
    is_open: *mut bool,
    failure_count: *mut u32,
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
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            if let Some(status) = coordinator.get_circuit_breaker_status(&service_id_str) {
                if !is_open.is_null() {
                    unsafe {
                        *is_open = status.is_open;
                    }
                }
                if !failure_count.is_null() {
                    unsafe {
                        *failure_count = status.failure_count;
                    }
                }
                CommyError::Success as i32
            } else {
                CommyError::ServiceNotFound as i32
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Reset circuit breaker
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
pub unsafe extern "C" fn commy_reset_circuit_breaker(
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
            match coordinator.reset_circuit_breaker(&service_id_str) {
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

/// Get service weights (for weighted load balancing)
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_name` must be a valid, non-null, NUL-terminated C string pointer.
/// `count` must be a valid, non-null pointer. Output pointer parameters may be
/// null; if non-null they must point to writable memory and must be freed by the
/// caller using the appropriate free functions.
pub unsafe extern "C" fn commy_get_service_weights(
    handle: CommyHandle,
    service_name: *const c_char,
    service_ids: *mut *mut *mut c_char,
    weights: *mut *mut u32,
    count: *mut usize,
) -> i32 {
    if handle.is_null() || service_name.is_null() || count.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_name_str = match c_str_to_string(service_name) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let service_weights = coordinator.get_service_weights(&service_name_str);

            let weight_count = service_weights.len();
            if weight_count == 0 {
                unsafe {
                    *count = 0;
                    if !service_ids.is_null() {
                        *service_ids = std::ptr::null_mut();
                    }
                    if !weights.is_null() {
                        *weights = std::ptr::null_mut();
                    }
                }
                return CommyError::Success as i32;
            }

            // Allocate arrays
            let ids_array = unsafe {
                libc::malloc(weight_count * std::mem::size_of::<*mut c_char>()) as *mut *mut c_char
            };
            let weights_array =
                unsafe { libc::malloc(weight_count * std::mem::size_of::<u32>()) as *mut u32 };

            if ids_array.is_null() || weights_array.is_null() {
                if !ids_array.is_null() {
                    unsafe { libc::free(ids_array as *mut libc::c_void) };
                }
                if !weights_array.is_null() {
                    unsafe { libc::free(weights_array as *mut libc::c_void) };
                }
                return CommyError::OutOfMemory as i32;
            }

            unsafe {
                for (i, (service_id, weight)) in service_weights.iter().enumerate() {
                    // Allocate and copy service ID
                    let id_cstring = std::ffi::CString::new(service_id.as_str()).unwrap();
                    let id_ptr = crate::ffi::memory::commy_strdup(id_cstring.as_ptr());
                    *ids_array.add(i) = id_ptr;

                    // Set weight
                    *weights_array.add(i) = *weight;
                }

                *count = weight_count;
                if !service_ids.is_null() {
                    *service_ids = ids_array;
                }
                if !weights.is_null() {
                    *weights = weights_array;
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

/// Free service weights arrays
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `service_ids` and `weights` must either be null or pointers previously
/// returned by `commy_get_service_weights` for the same `count` value.
pub unsafe extern "C" fn commy_free_service_weights_arrays(
    count: usize,
    service_ids: *mut *mut c_char,
    weights: *mut u32,
) {
    if count == 0 {
        return;
    }

    if !service_ids.is_null() {
        unsafe {
            for i in 0..count {
                let id_ptr = *service_ids.add(i);
                if !id_ptr.is_null() {
                    libc::free(id_ptr as *mut libc::c_void);
                }
            }
            libc::free(service_ids as *mut libc::c_void);
        }
    }

    if !weights.is_null() {
        unsafe {
            libc::free(weights as *mut libc::c_void);
        }
    }
}
