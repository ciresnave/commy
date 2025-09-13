//! Service management for FFI interface
//!
//! This module provides service registration, discovery, and management functions

use crate::ffi::memory::*;
use crate::ffi::types::*;
use crate::ffi::GLOBAL_INSTANCES;
#[cfg(feature = "ffi")]
use libc::c_char;

#[cfg(feature = "mesh")]
use crate::mesh::service_discovery::{SecurityLevel, ServiceCapabilities, ServiceRegistration};

/// Register a service with the mesh
///
/// # Safety
///
/// `config` must be a valid, non-null pointer to a `CommyServiceConfig`
/// structure and any C string pointers inside must be valid, NUL-terminated
/// pointers that the caller owns for the duration of the call.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_register_service(
    handle: CommyHandle,
    config: *const CommyServiceConfig,
) -> i32 {
    if handle.is_null() || config.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    #[cfg(feature = "mesh")]
    {
        let service_config = unsafe { &*config };

        let service_name = match unsafe { c_str_to_string(service_config.service_name) } {
            Ok(s) => s,
            Err(_) => return CommyError::InvalidArgument as i32,
        };

        let service_id = match unsafe { c_str_to_string(service_config.service_id) } {
            Ok(s) => s,
            Err(_) => return CommyError::InvalidArgument as i32,
        };

        let endpoint = match unsafe { c_str_to_string(service_config.endpoint) } {
            Ok(s) => s,
            Err(_) => return CommyError::InvalidArgument as i32,
        };

        let metadata =
            c_str_to_option_string(service_config.metadata).unwrap_or_else(|| "{}".to_string());

        let registration = ServiceRegistration {
            service_id: service_id.clone(),
            service_name,
            endpoint,
            port: service_config.port,
            capabilities: ServiceCapabilities::default(),
            security_level: SecurityLevel::Standard,
            weight: service_config.weight,
            metadata: serde_json::from_str(&metadata).unwrap_or_default(),
        };

        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            // Create a tokio runtime for async operations
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return CommyError::InitializationError as i32,
            };

            match rt.block_on(coordinator.register_service(registration)) {
                Ok(_) => CommyError::Success as i32,
                Err(_) => CommyError::ServiceAlreadyExists as i32,
            }
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Unregister a service from the mesh
///
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer
/// that the caller owns for the duration of the call.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_unregister_service(
    handle: CommyHandle,
    service_id: *const c_char,
) -> i32 {
    if handle.is_null() || service_id.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match unsafe { c_str_to_string(service_id) } {
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

            match rt.block_on(coordinator.unregister_service(&service_id_str)) {
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

/// Discover services by name
///
/// # Safety
///
/// `service_name`, if non-null, must be a valid NUL-terminated C string
/// pointer. `services` and `count` must be valid, non-null pointers that the
/// caller owns and allows to be written to.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_discover_services(
    handle: CommyHandle,
    service_name: *const c_char,
    services: *mut *mut CommyServiceInfo,
    count: *mut usize,
) -> i32 {
    if handle.is_null() || services.is_null() || count.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_name_str = match unsafe { c_str_to_string(service_name) } {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let discovered_services = coordinator.discover_services(&service_name_str);

            let service_count = discovered_services.len();
            if service_count == 0 {
                unsafe {
                    *services = std::ptr::null_mut();
                    *count = 0;
                }
                return CommyError::Success as i32;
            }

            let service_array = unsafe { commy_alloc_service_info_array(service_count) };
            if service_array.is_null() {
                return CommyError::OutOfMemory as i32;
            }

            unsafe {
                for (i, service) in discovered_services.iter().enumerate() {
                    let service_info = service_array.add(i);
                    *service_info = create_service_info_with_alloc(
                        &service.service_name,
                        &service.service_id,
                        &service.endpoint,
                        service.port,
                        CommyServiceStatus::Healthy, // Would need to check actual status
                        service.weight,
                        0, // Would need to get actual response time
                    );
                }

                *services = service_array;
                *count = service_count;
            }

            CommyError::Success as i32
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get all registered services
///
/// # Safety
///
/// `services` and `count` must be valid, non-null pointers that the caller
/// owns and allows to be written to.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_get_all_services(
    handle: CommyHandle,
    services: *mut *mut CommyServiceInfo,
    count: *mut usize,
) -> i32 {
    if handle.is_null() || services.is_null() || count.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            let all_services = coordinator.get_all_services();

            let service_count = all_services.len();
            if service_count == 0 {
                unsafe {
                    *services = std::ptr::null_mut();
                    *count = 0;
                }
                return CommyError::Success as i32;
            }

            let service_array = unsafe { commy_alloc_service_info_array(service_count) };
            if service_array.is_null() {
                return CommyError::OutOfMemory as i32;
            }

            unsafe {
                for (i, service) in all_services.iter().enumerate() {
                    let service_info = service_array.add(i);
                    *service_info = create_service_info_with_alloc(
                        &service.service_name,
                        &service.service_id,
                        &service.endpoint,
                        service.port,
                        CommyServiceStatus::Healthy, // Would need to check actual status
                        service.weight,
                        0, // Would need to get actual response time
                    );
                }

                *services = service_array;
                *count = service_count;
            }

            CommyError::Success as i32
        } else {
            CommyError::InstanceNotFound as i32
        }
    }

    #[cfg(not(feature = "mesh"))]
    CommyError::InternalError as i32
}

/// Get service by ID
///
/// # Safety
///
/// `service_id` must be a valid, non-null, NUL-terminated C string pointer.
/// `service_info` must be a valid, non-null pointer to a `CommyServiceInfo`
/// struct that the caller owns and allows to be written to.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_get_service(
    handle: CommyHandle,
    service_id: *const c_char,
    service_info: *mut CommyServiceInfo,
) -> i32 {
    if handle.is_null() || service_id.is_null() || service_info.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match unsafe { c_str_to_string(service_id) } {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    #[cfg(feature = "mesh")]
    {
        let instances = GLOBAL_INSTANCES.read();
        if let Some(coordinator) = instances.get(&handle.instance_id) {
            if let Some(service) = coordinator.get_service(&service_id_str) {
                unsafe {
                    *service_info = create_service_info_with_alloc(
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

/// Set service callback for notifications
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `handle` must be a valid `CommyHandle`. `callback` must be a valid
/// function pointer using the C calling convention and must remain valid
/// for as long as the service discovery system may call it.
pub unsafe extern "C" fn commy_set_service_callback(
    handle: CommyHandle,
    callback: CommyServiceCallback,
) -> i32 {
    if handle.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    // Store the callback for use by the service discovery system
    // This would integrate with the mesh coordinator's event system
    CommyError::Success as i32
}

/// Update service metadata
///
/// # Safety
///
/// `service_id` and `metadata`, if non-null, must be valid NUL-terminated C
/// string pointers owned by the caller for the duration of the call.
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn commy_update_service_metadata(
    handle: CommyHandle,
    service_id: *const c_char,
    metadata: *const c_char,
) -> i32 {
    if handle.is_null() || service_id.is_null() {
        return CommyError::InvalidArgument as i32;
    }

    let service_id_str = match c_str_to_string(service_id) {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidArgument as i32,
    };

    let metadata_str = c_str_to_option_string(metadata).unwrap_or_else(|| "{}".to_string());

    #[cfg(feature = "mesh")]
    {
        let mut instances = GLOBAL_INSTANCES.write();
        if let Some(coordinator) = instances.get_mut(&handle.instance_id) {
            let metadata_obj: serde_json::Value = match serde_json::from_str(&metadata_str) {
                Ok(obj) => obj,
                Err(_) => return CommyError::SerializationError as i32,
            };

            match coordinator.update_service_metadata(&service_id_str, metadata_obj) {
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
