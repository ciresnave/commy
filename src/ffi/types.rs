//! Common types and error handling for FFI interface
//!
//! This module defines all the C-compatible types used across the FFI interface

#[cfg(feature = "ffi")]
use libc::c_char;

/// FFI Result codes
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommyError {
    Success = 0,
    InvalidArgument = -1,
    OutOfMemory = -2,
    NetworkError = -3,
    SerializationError = -4,
    ServiceNotFound = -5,
    ServiceAlreadyExists = -6,
    InstanceNotFound = -7,
    InitializationError = -8,
    ConfigurationError = -9,
    HealthCheckFailed = -10,
    LoadBalancerError = -11,
    TimeoutError = -12,
    SecurityError = -13,
    PermissionDenied = -14,
    InternalError = -99,
}

impl From<anyhow::Error> for CommyError {
    fn from(_: anyhow::Error) -> Self {
        CommyError::InternalError
    }
}

impl From<std::io::Error> for CommyError {
    fn from(_: std::io::Error) -> Self {
        CommyError::NetworkError
    }
}

impl From<serde_json::Error> for CommyError {
    fn from(_: serde_json::Error) -> Self {
        CommyError::SerializationError
    }
}

/// Opaque handle to a mesh coordinator instance
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyHandle {
    pub instance_id: u64,
}

impl CommyHandle {
    pub const fn null() -> Self {
        Self { instance_id: 0 }
    }

    pub fn is_null(&self) -> bool {
        self.instance_id == 0
    }
}

/// Service configuration for FFI
#[repr(C)]
#[derive(Debug)]
pub struct CommyServiceConfig {
    pub service_name: *const c_char,
    pub service_id: *const c_char,
    pub endpoint: *const c_char,
    pub port: u16,
    pub weight: u32,
    pub metadata: *const c_char, // JSON string
}

/// Health check configuration for FFI
#[repr(C)]
#[derive(Debug)]
pub struct CommyHealthConfig {
    pub check_interval_ms: u64,
    pub timeout_ms: u64,
    pub max_failures: u32,
    pub recovery_checks: u32,
}

/// Load balancer algorithms
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyLoadBalancerAlgorithm {
    RoundRobin = 0,
    LeastConnections = 1,
    WeightedRoundRobin = 2,
    PerformanceBased = 3,
    Random = 4,
    ConsistentHash = 5,
}

/// Load balancer configuration for FFI
#[repr(C)]
#[derive(Debug)]
pub struct CommyLoadBalancerConfig {
    pub algorithm: CommyLoadBalancerAlgorithm,
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_ms: u64,
}

/// Service status
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyServiceStatus {
    Unknown = 0,
    Healthy = 1,
    Unhealthy = 2,
    Degraded = 3,
}

/// Service information for FFI
#[repr(C)]
#[derive(Debug)]
pub struct CommyServiceInfo {
    pub service_name: *const c_char,
    pub service_id: *const c_char,
    pub endpoint: *const c_char,
    pub port: u16,
    pub status: CommyServiceStatus,
    pub weight: u32,
    pub response_time_ms: u64,
}

/// Mesh statistics for FFI
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

/// Callback function types
pub type CommyLogCallback = extern "C" fn(level: i32, message: *const c_char);
pub type CommyHealthCallback = extern "C" fn(service_id: *const c_char, status: CommyServiceStatus);
pub type CommyServiceCallback = extern "C" fn(service_info: *const CommyServiceInfo);

/// Memory utilities for FFI
#[cfg(feature = "ffi")]
#[no_mangle]
/// # Safety
///
/// `ptr` must be a pointer previously returned by an FFI allocation function
/// from this crate (for example `allocate_string` or `commy_strdup`). The
/// pointer must be non-null and uniquely owned by the caller; double-free or
/// using a pointer that wasn't allocated by the matching allocator is undefined
/// behavior.
pub unsafe extern "C" fn commy_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(std::ffi::CString::from_raw(ptr));
        }
    }
}

/// Allocate memory for strings (for returning values to other languages)
#[cfg(feature = "ffi")]
pub fn allocate_string(s: &str) -> *mut c_char {
    let c_string = std::ffi::CString::new(s).unwrap();
    c_string.into_raw()
}

// Ownership / lifetime notes for FFI consumers:
// - `allocate_string` returns a pointer created via `CString::into_raw()`. The
//   caller (foreign code) becomes the owner and must call `commy_free_string`
//   to reclaim the memory. Do NOT use `commy_free` to free pointers produced
//   by `allocate_string` as that mismatches allocation/deallocation semantics.
// - `commy_strdup` allocates memory via the C allocator (malloc) and should be
//   freed with `commy_free` (or equivalent C free). The contract for string
//   allocation functions is documented alongside each symbol to avoid misuse.
// - For arrays of `CommyServiceInfo` produced by `commy_alloc_service_info_array`,
//   each string pointer inside the struct is separately allocated and the
//   caller must use `commy_free_service_info_array` to free both the inner
//   strings and the array itself.

/// Convert C string to Rust string safely
#[cfg(feature = "ffi")]
/// # Safety
///
/// `ptr` must be a valid, non-null, NUL-terminated C string pointer. The
/// caller is responsible for ensuring the pointer points to valid UTF-8 data if
/// they expect a successful conversion; otherwise the function will return
/// CommyError::InvalidArgument.
pub unsafe fn c_str_to_string(ptr: *const c_char) -> Result<String, CommyError> {
    if ptr.is_null() {
        return Err(CommyError::InvalidArgument);
    }

    unsafe {
        let c_str = std::ffi::CStr::from_ptr(ptr);
        c_str
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| CommyError::InvalidArgument)
    }
}

/// Convert optional C string to Rust Option<String>
#[cfg(feature = "ffi")]
#[allow(dead_code)]
pub(crate) fn c_str_to_option_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        // Safe wrapper that delegates to unsafe c_str_to_string
        unsafe { c_str_to_string(ptr) }.ok()
    }
}
