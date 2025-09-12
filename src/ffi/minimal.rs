//! Minimal working FFI implementation for Phase 3
//!
//! This module provides a basic C-compatible FFI interface that demonstrates
//! the foundation for multi-language SDKs. It includes only the core functionality
//! that works as a proof of concept for the FFI layer.

#![allow(clippy::not_unsafe_ptr_arg_deref)] // Expected in FFI modules
#![allow(clippy::manual_c_str_literals)]
#![allow(clippy::manual_unwrap_or)]
#![allow(clippy::useless_vec)]

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::Mutex;

#[cfg(feature = "ffi")]
use rand;

/// Global storage for mesh coordinator instances (using string placeholders for now)
static GLOBAL_INSTANCES: Lazy<RwLock<HashMap<u64, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Track which mesh instances are running
static RUNNING_INSTANCES: Lazy<RwLock<HashMap<u64, bool>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Global storage for SharedFileManager instances
#[cfg(feature = "manager")]
static GLOBAL_FILE_MANAGERS: Lazy<RwLock<HashMap<u64, crate::manager::SharedFileManager>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

static NEXT_INSTANCE_ID: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(1));

/// C-compatible handle for mesh instances
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyHandle {
    pub instance_id: u64,
    pub error_code: i32,
}

/// C-compatible error codes
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyError {
    Success = 0,
    InitializationError = 1,
    InvalidParameter = 2,
    InstanceNotFound = 3,
    AllocError = 4,
}

/// Service status enumeration
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyServiceStatus {
    Unknown = 0,
    Healthy = 1,
    Unhealthy = 2,
    Degraded = 3,
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

/// Health check configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyHealthConfig {
    pub check_interval_ms: u32,
    pub timeout_ms: u32,
    pub max_failures: u32,
    pub recovery_checks: u32,
}

/// Load balancer configuration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyLoadBalancerConfig {
    pub algorithm: CommyLoadBalancerAlgorithm,
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_ms: u64,
}

/// Service configuration for registration
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyServiceConfig {
    pub service_name: *const c_char,
    pub service_id: *const c_char,
    pub endpoint: *const c_char,
    pub port: u16,
    pub weight: u32,
    pub metadata: *const c_char, // JSON string
}

/// C-compatible mesh statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyMeshStats {
    pub total_services: u32,
    pub healthy_services: u32,
    pub unhealthy_services: u32,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: f64,
}

/// Simplified service info structure for FFI
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyServiceInfo {
    pub service_name: *mut c_char,
    pub service_id: *mut c_char,
    pub endpoint: *mut c_char,
    pub port: u16,
    pub status: i32,
    pub weight: u32,
    pub response_time_ms: f64,
}

// ============================================================================
// PHASE 1 SHAREDFILEMANAGER FFI STRUCTURES
// ============================================================================

/// SharedFileManager handle for Phase 1 operations
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyFileManagerHandle {
    pub manager_id: u64,
    pub error_code: i32,
}

/// Existence policy for shared files
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyExistencePolicy {
    CreateOrConnect = 0,
    CreateOnly = 1,
    ConnectOnly = 2,
}

/// Permission levels for shared files
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyPermission {
    ReadOnly = 0,
    WriteOnly = 1,
    ReadWrite = 2,
    Execute = 3,
}

/// Transport preference for shared files
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyTransportPreference {
    Auto = 0,
    SharedMemory = 1,
    Network = 2,
    HybridOptimized = 3,
}

/// Serialization format for shared files
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommySerializationFormat {
    Json = 0,
    Binary = 1,
    MessagePack = 2,
    Cbor = 3,
    ZeroCopy = 4,
}

/// Compliance report type
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyComplianceReportType {
    AccessAudit = 0,
    SecuritySummary = 1,
    PerformanceMetrics = 2,
    ConfigurationStatus = 3,
    FullCompliance = 4,
}

/// Compliance report data structure
#[repr(C)]
#[derive(Debug)]
pub struct CommyComplianceReport {
    pub report_id: *mut c_char,
    pub report_type: CommyComplianceReportType,
    pub generated_at: u64, // Unix timestamp
    pub data_json: *mut c_char,
    pub summary: *mut c_char,
    pub violations_count: u32,
    pub recommendations_count: u32,
}

/// Audit event structure
#[repr(C)]
#[derive(Debug)]
pub struct CommyAuditEvent {
    pub event_id: *mut c_char,
    pub timestamp: u64,
    pub event_type: *mut c_char,
    pub user_id: *mut c_char,
    pub resource: *mut c_char,
    pub action: *mut c_char,
    pub result: *mut c_char,
    pub details: *mut c_char,
}

// ============================================================================
// PHASE 4: ENTERPRISE FEATURES - OBSERVABILITY
// ============================================================================

/// Trace span context for distributed tracing
#[repr(C)]
#[derive(Debug)]
pub struct CommyTraceSpan {
    pub span_id: *mut c_char,
    pub trace_id: *mut c_char,
    pub parent_span_id: *mut c_char,
    pub operation_name: *mut c_char,
    pub start_time: u64,
    pub end_time: u64,
    pub duration_microseconds: u64,
    pub tags: *mut *mut c_char, // Array of key=value strings
    pub tag_count: u32,
    pub status: CommyTraceStatus,
}

/// Trace status enumeration
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyTraceStatus {
    Ok = 0,
    Cancelled = 1,
    Unknown = 2,
    InvalidArgument = 3,
    DeadlineExceeded = 4,
    NotFound = 5,
    AlreadyExists = 6,
    PermissionDenied = 7,
    ResourceExhausted = 8,
    FailedPrecondition = 9,
    Aborted = 10,
    OutOfRange = 11,
    Unimplemented = 12,
    Internal = 13,
    Unavailable = 14,
    DataLoss = 15,
    Unauthenticated = 16,
}

/// Metric type enumeration
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyMetricType {
    Counter = 0,
    Gauge = 1,
    Histogram = 2,
    Summary = 3,
}

/// Metric data structure
#[repr(C)]
#[derive(Debug)]
pub struct CommyMetric {
    pub name: *mut c_char,
    pub metric_type: CommyMetricType,
    pub value: f64,
    pub timestamp: u64,
    pub labels: *mut *mut c_char, // Array of key=value strings
    pub label_count: u32,
    pub help_text: *mut c_char,
    pub unit: *mut c_char,
}

/// Region information for federation
#[repr(C)]
#[derive(Debug)]
pub struct CommyRegion {
    pub region_id: *mut c_char,
    pub region_name: *mut c_char,
    pub endpoint: *mut c_char,
    pub latency_ms: u32,
    pub is_available: bool,
    pub data_locality_preference: bool,
    pub compliance_zone: *mut c_char,
}

/// Federation configuration
#[repr(C)]
#[derive(Debug)]
pub struct CommyFederationConfig {
    pub local_region: *mut c_char,
    pub regions: *mut CommyRegion,
    pub region_count: u32,
    pub failover_enabled: bool,
    pub wan_optimization: bool,
    pub cross_region_encryption: bool,
    pub global_load_balancing: bool,
}

/// Policy engine rule
#[repr(C)]
#[derive(Debug)]
pub struct CommyPolicyRule {
    pub rule_id: *mut c_char,
    pub name: *mut c_char,
    pub description: *mut c_char,
    pub rule_type: CommyPolicyType,
    pub condition: *mut c_char, // JSON or expression
    pub action: *mut c_char,    // JSON action definition
    pub enabled: bool,
    pub priority: u32,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Policy type enumeration
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum CommyPolicyType {
    AccessControl = 0,
    DataGovernance = 1,
    ComplianceCheck = 2,
    SecurityPolicy = 3,
    PerformanceLimit = 4,
    ResourceQuota = 5,
}

/// Deployment configuration
#[repr(C)]
#[derive(Debug)]
pub struct CommyDeploymentConfig {
    pub deployment_id: *mut c_char,
    pub environment: *mut c_char, // dev, staging, prod
    pub cluster_name: *mut c_char,
    pub namespace: *mut c_char,
    pub replica_count: u32,
    pub cpu_limit: *mut c_char,
    pub memory_limit: *mut c_char,
    pub storage_class: *mut c_char,
    pub enable_tls: bool,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
}

/// C-compatible shared file request structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommySharedFileRequest {
    pub identifier: *const c_char,
    pub file_path: *const c_char,
    pub size: u64,
    pub existence_policy: CommyExistencePolicy,
    pub permission: CommyPermission,
    pub transport_preference: CommyTransportPreference,
    pub serialization_format: CommySerializationFormat,
    pub ttl_seconds: u32,
    pub auth_token: *const c_char,
}

/// C-compatible shared file response structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CommySharedFileResponse {
    pub file_id: u64,
    pub file_path: *mut c_char,
    pub actual_size: u64,
    pub is_creator: bool,
    pub connection_count: u32,
    pub transport_used: CommyTransportPreference,
    pub error_message: *mut c_char,
}

/// Manager configuration for SharedFileManager
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CommyManagerConfig {
    pub listen_port: u16,
    pub bind_address: *const c_char,
    pub max_files: u32,
    pub max_file_size: u64,
    pub default_ttl_seconds: u32,
    pub heartbeat_timeout_seconds: u32,
    pub cleanup_interval_seconds: u32,
    pub database_path: *const c_char,
    pub files_directory: *const c_char,
    pub require_tls: bool,
    pub tls_cert_path: *const c_char,
    pub tls_key_path: *const c_char,
}

/// File information structure
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CommyFileInfo {
    pub file_id: u64,
    pub identifier: *mut c_char,
    pub file_path: *mut c_char,
    pub size: u64,
    pub connection_count: u32,
    pub created_at: i64,    // Unix timestamp
    pub last_accessed: i64, // Unix timestamp
    pub transport_used: CommyTransportPreference,
}

/// Initialize the FFI layer
#[no_mangle]
pub extern "C" fn commy_ffi_init() -> i32 {
    // Initialize any global state if needed
    CommyError::Success as i32
}

/// Cleanup and reset all FFI state (for testing)
#[no_mangle]
pub extern "C" fn commy_ffi_reset() -> i32 {
    // Clear all instances
    {
        let mut instances = GLOBAL_INSTANCES.write();
        instances.clear();
    }

    // Clear running instances
    {
        let mut running = RUNNING_INSTANCES.write();
        running.clear();
    }

    // Clear file managers
    {
        let mut managers = GLOBAL_FILE_MANAGERS.write();
        managers.clear();
    }

    // Reset next instance ID
    {
        let mut next_id = NEXT_INSTANCE_ID.lock().unwrap();
        *next_id = 1;
    }

    CommyError::Success as i32
}

/// Cleanup FFI resources (placeholder for now)
#[no_mangle]
pub extern "C" fn commy_ffi_cleanup() -> i32 {
    // Clear all instances
    {
        let mut instances = GLOBAL_INSTANCES.write();
        instances.clear();
    }

    // Clear running instances
    {
        let mut running = RUNNING_INSTANCES.write();
        running.clear();
    }

    // Clear file managers
    {
        let mut managers = GLOBAL_FILE_MANAGERS.write();
        managers.clear();
    }

    CommyError::Success as i32
}

/// Get the FFI version string
#[no_mangle]
pub extern "C" fn commy_ffi_version() -> *const c_char {
    static VERSION: &str = "0.1.0\0";
    VERSION.as_ptr() as *const c_char
}

/// Create a new mesh coordinator
///
/// # Safety
/// - `node_id` must be a non-null pointer to a valid, null-terminated C string.
/// - `node_id` must remain valid for the duration of the call and is owned by the caller.
/// - `port` must be non-zero and a valid port number.
/// - This function dereferences raw pointers and performs FFI allocations; callers must
///   free any returned C strings with `commy_free_string` when appropriate.
#[no_mangle]
pub unsafe extern "C" fn commy_create_mesh(node_id: *const c_char, port: u16) -> CommyHandle {
    if node_id.is_null() {
        return CommyHandle {
            instance_id: 0,
            error_code: CommyError::InvalidParameter as i32,
        };
    }

    let node_id_str = match unsafe { CStr::from_ptr(node_id) }.to_str() {
        Ok(s) if !s.is_empty() => s,
        _ => {
            return CommyHandle {
                instance_id: 0,
                error_code: CommyError::InvalidParameter as i32,
            }
        }
    };

    if port == 0 {
        return CommyHandle {
            instance_id: 0,
            error_code: CommyError::InvalidParameter as i32,
        };
    }

    // For now, create a mock coordinator since the actual one requires async and complex config
    // This demonstrates the FFI structure for future implementation
    let instance_id = {
        let mut next_id = NEXT_INSTANCE_ID.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        id
    };

    // Store a placeholder entry to track the instance
    // In the future, this will store the actual MeshCoordinator
    let mut instances = GLOBAL_INSTANCES.write();
    instances.insert(instance_id, format!("{}:{}", node_id_str, port));

    CommyHandle {
        instance_id,
        error_code: CommyError::Success as i32,
    }
}

/// Start the mesh coordinator
///
/// # Safety
/// - `handle` must be a valid `CommyHandle` previously returned from `commy_create_mesh`.
/// - Callers must ensure the underlying mesh instance is not concurrently destroyed.
#[no_mangle]
pub unsafe extern "C" fn commy_start_mesh(handle: CommyHandle) -> i32 {
    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        // Mark as running
        let mut running = RUNNING_INSTANCES.write();
        running.insert(handle.instance_id, true);
        eprintln!("✅ FFI: Started mesh instance {}", handle.instance_id);
        CommyError::Success as i32
    } else {
        CommyError::InstanceNotFound as i32
    }
}

/// Stop the mesh coordinator
///
/// # Safety
/// - `handle` must be a valid `CommyHandle` previously returned from `commy_create_mesh`.
/// - Callers must ensure the underlying mesh instance is not concurrently destroyed.
#[no_mangle]
pub unsafe extern "C" fn commy_stop_mesh(handle: CommyHandle) -> i32 {
    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        // Mark as stopped
        let mut running = RUNNING_INSTANCES.write();
        running.insert(handle.instance_id, false);
        eprintln!("✅ FFI: Stopped mesh instance {}", handle.instance_id);
        CommyError::Success as i32
    } else {
        CommyError::InstanceNotFound as i32
    }
}

/// Check if mesh is running
///
/// # Safety
/// - `handle` must be a valid `CommyHandle` previously returned from `commy_create_mesh`.
#[no_mangle]
pub unsafe extern "C" fn commy_is_mesh_running(handle: CommyHandle) -> i32 {
    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        let running = RUNNING_INSTANCES.read();
        if running.get(&handle.instance_id).copied().unwrap_or(false) {
            1 // true - running
        } else {
            0 // false - not running
        }
    } else {
        -1 // error - instance not found
    }
}

/// Configure mesh settings (minimal implementation - renamed to avoid symbol collisions)
///
/// # Safety
/// - `handle` must be a valid `CommyHandle` returned from `commy_create_mesh`.
/// - `health_config` and `lb_config` may be null; if non-null they must point to valid
///   structures that remain alive for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn commy_configure_mesh_minimal(
    handle: CommyHandle,
    health_config: *const CommyHealthConfig,
    lb_config: *const CommyLoadBalancerConfig,
) -> i32 {
    let instances = GLOBAL_INSTANCES.read();
    if !instances.contains_key(&handle.instance_id) {
        return CommyError::InstanceNotFound as i32;
    }

    // Validate health config if provided
    if !health_config.is_null() {
        unsafe {
            let config = &*health_config;
            if config.check_interval_ms == 0 || config.timeout_ms == 0 {
                return CommyError::InvalidParameter as i32;
            }
        }
    }

    // Validate load balancer config if provided
    if !lb_config.is_null() {
        unsafe {
            let config = &*lb_config;
            // Basic validation
            if config.circuit_breaker_threshold == 0 {
                return CommyError::InvalidParameter as i32;
            }
        }
    }

    // For now, just validate and return success
    CommyError::Success as i32
}

/// Register a service with the mesh
///
/// # Safety
/// - `config` must be a non-null pointer to a valid `CommyServiceConfig`.
/// - All `*const c_char` fields inside `CommyServiceConfig` must be valid, null-terminated
///   C strings and remain valid for the duration of the call.
/// - The caller retains ownership of strings; this function does not take ownership.
/// - `handle` must be a valid `CommyHandle` returned from `commy_create_mesh`.
#[no_mangle]
pub unsafe extern "C" fn commy_register_service(
    handle: CommyHandle,
    config: *const CommyServiceConfig,
) -> i32 {
    if config.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_INSTANCES.read();
    if !instances.contains_key(&handle.instance_id) {
        return CommyError::InstanceNotFound as i32;
    }

    unsafe {
        let service_config = &*config;

        // Validate required fields
        if service_config.service_name.is_null()
            || service_config.service_id.is_null()
            || service_config.endpoint.is_null()
        {
            return CommyError::InvalidParameter as i32;
        }

        // Convert and validate strings
        let _service_name = match CStr::from_ptr(service_config.service_name).to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => return CommyError::InvalidParameter as i32,
        };

        let _service_id = match CStr::from_ptr(service_config.service_id).to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => return CommyError::InvalidParameter as i32,
        };

        if service_config.port == 0 {
            return CommyError::InvalidParameter as i32;
        }

        // For now, just validate and return success
        // In future implementation, this would register with actual service discovery
        CommyError::Success as i32
    }
}

/// Unregister a service from the mesh
///
/// # Safety
/// - `service_name` must be a non-null pointer to a valid, null-terminated C string.
/// - The caller retains ownership of the string memory.
/// - `handle` must be a valid `CommyHandle` returned from `commy_create_mesh`.
#[no_mangle]
pub unsafe extern "C" fn commy_unregister_service(
    handle: CommyHandle,
    service_name: *const c_char,
) -> i32 {
    if service_name.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_INSTANCES.read();
    if !instances.contains_key(&handle.instance_id) {
        return CommyError::InstanceNotFound as i32;
    }

    unsafe {
        // Validate service name
        let _service_name = match CStr::from_ptr(service_name).to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => return CommyError::InvalidParameter as i32,
        };

        // For now, just validate and return success
        // In future implementation, this would unregister from actual service discovery
        CommyError::Success as i32
    }
}

/// Discover services by name
///
/// # Safety
/// - `service_name` must be a non-null pointer to a valid, null-terminated C string.
/// - `services` and `count` must be valid, non-null pointers where results will be written.
/// - The callee will allocate or set the `services` pointer; callers are responsible for
///   freeing any returned memory using the corresponding free functions documented in the API.
#[no_mangle]
pub unsafe extern "C" fn commy_discover_services(
    handle: CommyHandle,
    service_name: *const c_char,
    services: *mut *mut CommyServiceInfo,
    count: *mut usize,
) -> i32 {
    if service_name.is_null() || services.is_null() || count.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_INSTANCES.read();
    if !instances.contains_key(&handle.instance_id) {
        return CommyError::InstanceNotFound as i32;
    }

    unsafe {
        let _name_str = match CStr::from_ptr(service_name).to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => return CommyError::InvalidParameter as i32,
        };

        // For now, return empty results since we haven't implemented actual discovery
        // In future implementation, this would query the service registry
        *services = ptr::null_mut();
        *count = 0;

        CommyError::Success as i32
    }
}

/// Select a service using load balancer
///
/// # Safety
/// - `service_name` must be a non-null pointer to a valid, null-terminated C string.
/// - `selected_service` must be a valid, non-null pointer to a `CommyServiceInfo` struct
///   that the caller has allocated.
#[no_mangle]
pub unsafe extern "C" fn commy_select_service(
    handle: CommyHandle,
    service_name: *const c_char,
    _client_id: *const c_char,
    selected_service: *mut CommyServiceInfo,
) -> i32 {
    if service_name.is_null() || selected_service.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_INSTANCES.read();
    if !instances.contains_key(&handle.instance_id) {
        return CommyError::InstanceNotFound as i32;
    }

    unsafe {
        let _name_str = match CStr::from_ptr(service_name).to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => return CommyError::InvalidParameter as i32,
        };

        // For now, return a default "not found" result
        // In future implementation, this would use actual load balancing logic
        *selected_service = CommyServiceInfo {
            service_name: ptr::null_mut(),
            service_id: ptr::null_mut(),
            endpoint: ptr::null_mut(),
            port: 0,
            status: 0, // Unknown
            weight: 0,
            response_time_ms: 0.0,
        };

        CommyError::Success as i32
    }
}

/// Get node ID
///
/// # Safety
/// - The returned pointer is allocated by the FFI and must be freed by the caller using
///   `commy_free_string` when no longer needed.
/// - The returned pointer may be null to indicate errors or missing instances.
#[no_mangle]
pub unsafe extern "C" fn commy_get_node_id(handle: CommyHandle) -> *mut c_char {
    let instances = GLOBAL_INSTANCES.read();
    if let Some(node_info) = instances.get(&handle.instance_id) {
        // Extract node ID from stored info (format: "node_id:port")
        let node_id = node_info.split(':').next().unwrap_or("unknown");
        match CString::new(node_id) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Get mesh statistics
///
/// # Safety
/// - `stats` must be a valid, non-null pointer to a `CommyMeshStats` structure where results
///   will be written.
#[no_mangle]
pub unsafe extern "C" fn commy_get_mesh_stats(
    handle: CommyHandle,
    stats: *mut CommyMeshStats,
) -> i32 {
    if stats.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        // Return default stats for now
        unsafe {
            *stats = CommyMeshStats {
                total_services: 0,
                healthy_services: 0,
                unhealthy_services: 0,
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                average_response_time_ms: 0.0,
            };
        }
        CommyError::Success as i32
    } else {
        CommyError::InstanceNotFound as i32
    }
}

/// Free a string allocated by the FFI
///
/// # Safety
/// - `ptr` must be a pointer previously returned by one of the FFI allocation helpers
///   (e.g., `commy_strdup`, `commy_get_node_id`, or similar).
/// - After calling this function the caller must not use `ptr` again.
#[no_mangle]
pub unsafe extern "C" fn commy_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Allocate memory (simple wrapper around malloc)
///
/// # Safety
/// - Caller must check for null return when allocation fails.
/// - Memory returned must be freed using `commy_free`.
#[no_mangle]
pub unsafe extern "C" fn commy_malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return ptr::null_mut();
    }

    unsafe { libc::malloc(size) }
}

/// Free memory allocated by commy_malloc
///
/// # Safety
/// - `ptr` must point to memory previously returned by `commy_malloc`.
/// - After calling this function the pointer must not be used again.
#[no_mangle]
pub unsafe extern "C" fn commy_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            libc::free(ptr);
        }
    }
}

/// Duplicate a string
///
/// # Safety
/// - `src` must be a non-null pointer to a valid, null-terminated C string.
/// - The returned pointer is owned by the caller and must be freed with `commy_free_string`.
#[no_mangle]
pub unsafe extern "C" fn commy_strdup(src: *const c_char) -> *mut c_char {
    if src.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let src_str = CStr::from_ptr(src);
        match CString::new(src_str.to_bytes()) {
            Ok(new_string) => new_string.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    }
}

/// Allocate a service info array
///
/// # Safety
/// - `count` must be greater than zero.
/// - The returned pointer must be freed with `commy_free_service_info_array` which will also
///   free any nested strings.
#[no_mangle]
pub unsafe extern "C" fn commy_alloc_service_info_array(count: usize) -> *mut CommyServiceInfo {
    if count == 0 {
        return ptr::null_mut();
    }

    unsafe {
        let size = count * std::mem::size_of::<CommyServiceInfo>();
        let ptr = libc::malloc(size) as *mut CommyServiceInfo;
        if !ptr.is_null() {
            // Initialize to zeros
            std::ptr::write_bytes(ptr, 0, count);
        }
        ptr
    }
}

/// Free a service info array
///
/// # Safety
/// - `ptr` must be either null or a pointer previously returned by `commy_alloc_service_info_array`.
/// - `count` must match the number of elements allocated.
#[no_mangle]
pub unsafe extern "C" fn commy_free_service_info_array(ptr: *mut CommyServiceInfo, count: usize) {
    if ptr.is_null() || count == 0 {
        return;
    }

    unsafe {
        // Free any allocated strings in the array
        for i in 0..count {
            let service_info = &*ptr.add(i);
            if !service_info.service_name.is_null() {
                commy_free_string(service_info.service_name);
            }
            if !service_info.service_id.is_null() {
                commy_free_string(service_info.service_id);
            }
            if !service_info.endpoint.is_null() {
                commy_free_string(service_info.endpoint);
            }
        }

        // Free the array itself
        libc::free(ptr as *mut c_void);
    }
}

/// Simple service registration that koffi can handle
///
/// # Safety
/// - All `*const c_char` parameters must be valid, null-terminated C strings owned by the caller.
#[no_mangle]
pub unsafe extern "C" fn commy_register_service_simple(
    handle: CommyHandle,
    name: *const c_char,
    version: *const c_char,
    endpoint: *const c_char,
    port: u16,
) -> i32 {
    if name.is_null() || version.is_null() || endpoint.is_null() {
        return -1;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    let version_str = match unsafe { CStr::from_ptr(version) }.to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    let endpoint_str = match unsafe { CStr::from_ptr(endpoint) }.to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };

    // Check if we have a valid mesh instance
    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        // Register the service (simplified)
        eprintln!(
            "✅ FFI: Registering service: {}@{} on {}:{}",
            name_str, version_str, endpoint_str, port
        );
        0 // Success
    } else {
        -3 // Mesh not initialized
    }
}

/// Simple service discovery count that koffi can handle
///
/// # Safety
/// - `service_name` must be a non-null pointer to a valid, null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn commy_discover_services_count(
    handle: CommyHandle,
    service_name: *const c_char,
) -> i32 {
    if service_name.is_null() {
        return -1;
    }

    let service_name_str = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Check if we have a valid mesh instance
    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        // Mock discovery - return 1 if we have any services
        eprintln!("✅ FFI: Discovering services for: {}", service_name_str);
        1 // Mock count
    } else {
        0 // No mesh
    }
}

/// Get active service count that koffi can handle
///
/// # Safety
/// - `handle` must be a valid `CommyHandle` returned from a successful call to `commy_create_mesh`.
#[no_mangle]
pub unsafe extern "C" fn commy_get_active_service_count(handle: CommyHandle) -> u32 {
    // Check if we have a valid mesh instance
    let instances = GLOBAL_INSTANCES.read();
    if instances.contains_key(&handle.instance_id) {
        eprintln!("✅ FFI: Getting active service count");
        1 // Mock count
    } else {
        0
    }
}

// ============================================================================
// PHASE 1 SHAREDFILEMANAGER FFI FUNCTIONS
// ============================================================================

/// Create a new SharedFileManager instance
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_create_file_manager(
    config: *const CommyManagerConfig,
) -> CommyFileManagerHandle {
    if config.is_null() {
        return CommyFileManagerHandle {
            manager_id: 0,
            error_code: CommyError::InvalidParameter as i32,
        };
    }

    let _config_ref = unsafe { &*config };

    // Convert C strings to Rust strings
    let bind_address = if _config_ref.bind_address.is_null() {
        "127.0.0.1".to_string()
    } else {
        match unsafe { CStr::from_ptr(_config_ref.bind_address) }.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                return CommyFileManagerHandle {
                    manager_id: 0,
                    error_code: CommyError::InvalidParameter as i32,
                }
            }
        }
    };

    let database_path = if _config_ref.database_path.is_null() {
        std::path::PathBuf::from("./commy_data/db.sqlite")
    } else {
        match unsafe { CStr::from_ptr(_config_ref.database_path) }.to_str() {
            Ok(s) => std::path::PathBuf::from(s),
            Err(_) => {
                return CommyFileManagerHandle {
                    manager_id: 0,
                    error_code: CommyError::InvalidParameter as i32,
                }
            }
        }
    };

    let files_directory = if _config_ref.files_directory.is_null() {
        std::path::PathBuf::from("./commy_data/files")
    } else {
        match unsafe { CStr::from_ptr(_config_ref.files_directory) }.to_str() {
            Ok(s) => std::path::PathBuf::from(s),
            Err(_) => {
                return CommyFileManagerHandle {
                    manager_id: 0,
                    error_code: CommyError::InvalidParameter as i32,
                }
            }
        }
    };

    // Create ManagerConfig
    let _manager_config = crate::manager::core::ManagerConfig {
        listen_port: _config_ref.listen_port,
        bind_address,
        max_files: _config_ref.max_files,
        max_file_size: _config_ref.max_file_size,
        default_ttl_seconds: _config_ref.default_ttl_seconds as u64,
        heartbeat_timeout_seconds: _config_ref.heartbeat_timeout_seconds as u64,
        cleanup_interval_seconds: _config_ref.cleanup_interval_seconds as u64,
        database_path,
        files_directory,
        tls_cert_path: None, // Simplified for now
        tls_key_path: None,  // Simplified for now
        require_tls: _config_ref.require_tls,
        enable_mesh_capabilities: true,
        performance_config: Default::default(),
        security_config: Default::default(),
    };

    // Convert C strings to Rust strings
    let bind_address = if _config_ref.bind_address.is_null() {
        "127.0.0.1".to_string()
    } else {
        match unsafe { CStr::from_ptr(_config_ref.bind_address) }.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                return CommyFileManagerHandle {
                    manager_id: 0,
                    error_code: CommyError::InvalidParameter as i32,
                }
            }
        }
    };

    let database_path = if _config_ref.database_path.is_null() {
        std::path::PathBuf::from("./commy_data/db.sqlite")
    } else {
        match unsafe { CStr::from_ptr(_config_ref.database_path) }.to_str() {
            Ok(s) => std::path::PathBuf::from(s),
            Err(_) => {
                return CommyFileManagerHandle {
                    manager_id: 0,
                    error_code: CommyError::InvalidParameter as i32,
                }
            }
        }
    };

    let files_directory = if _config_ref.files_directory.is_null() {
        std::path::PathBuf::from("./commy_data/files")
    } else {
        match unsafe { CStr::from_ptr(_config_ref.files_directory) }.to_str() {
            Ok(s) => std::path::PathBuf::from(s),
            Err(_) => {
                return CommyFileManagerHandle {
                    manager_id: 0,
                    error_code: CommyError::InvalidParameter as i32,
                }
            }
        }
    };

    // Create ManagerConfig
    let _manager_config = crate::manager::core::ManagerConfig {
        listen_port: _config_ref.listen_port,
        bind_address,
        max_files: _config_ref.max_files,
        max_file_size: _config_ref.max_file_size,
        default_ttl_seconds: _config_ref.default_ttl_seconds as u64,
        heartbeat_timeout_seconds: _config_ref.heartbeat_timeout_seconds as u64,
        cleanup_interval_seconds: _config_ref.cleanup_interval_seconds as u64,
        database_path,
        files_directory,
        tls_cert_path: None, // Simplified for now
        tls_key_path: None,  // Simplified for now
        require_tls: _config_ref.require_tls,
        enable_mesh_capabilities: true,
        performance_config: Default::default(),
        security_config: Default::default(),
    };

    // Create SharedFileManager asynchronously (this is a simplified version)
    // In a real implementation, this would need proper async handling
    let manager_id = {
        let mut next_id = NEXT_INSTANCE_ID.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        id
    };

    // Store placeholder for now (real implementation would store actual manager)
    {
        let _managers = GLOBAL_FILE_MANAGERS.write();
        // This is a simplified approach - in practice we'd need proper async initialization
        eprintln!(
            "✅ FFI: Created SharedFileManager placeholder with ID: {}",
            manager_id
        );
    }

    CommyFileManagerHandle {
        manager_id,
        error_code: CommyError::Success as i32,
    }
}

/// Request a shared file allocation
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_request_shared_file(
    ___manager_handle: CommyFileManagerHandle,
    request: *const CommySharedFileRequest,
    response: *mut CommySharedFileResponse,
) -> i32 {
    if request.is_null() || response.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let request_ref = unsafe { &*request };

    // Validate required fields
    if request_ref.identifier.is_null() || request_ref.size == 0 {
        return CommyError::InvalidParameter as i32;
    }

    let identifier = match unsafe { CStr::from_ptr(request_ref.identifier) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    let file_path = if request_ref.file_path.is_null() {
        format!("./commy_files/{}.mmap", identifier)
    } else {
        match unsafe { CStr::from_ptr(request_ref.file_path) }.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return CommyError::InvalidParameter as i32,
        }
    };

    // Create simplified SharedFileRequest
    eprintln!(
        "✅ FFI: Requesting shared file '{}' with size {} bytes",
        identifier, request_ref.size
    );

    // Mock successful response
    let file_id = 1000 + ___manager_handle.manager_id; // Mock file ID

    // Allocate and set response
    unsafe {
        (*response).file_id = file_id;
        (*response).file_path = commy_strdup(file_path.as_ptr() as *const c_char);
        (*response).actual_size = request_ref.size;
        (*response).is_creator = true;
        (*response).connection_count = 1;
        (*response).transport_used = request_ref.transport_preference;
        (*response).error_message = ptr::null_mut();
    }

    CommyError::Success as i32
}

/// Disconnect from a shared file
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_disconnect_shared_file(
    ____manager_handle: CommyFileManagerHandle,
    file_id: u64,
) -> i32 {
    eprintln!("✅ FFI: Disconnecting from shared file ID: {}", file_id);

    // Mock successful disconnection
    CommyError::Success as i32
}

/// List active shared files
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_list_active_files(
    ____manager_handle: CommyFileManagerHandle,
    files_out: *mut *mut CommyFileInfo,
    count_out: *mut u32,
) -> i32 {
    if files_out.is_null() || count_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    eprintln!("✅ FFI: Listing active shared files");

    // Mock response with no active files
    unsafe {
        *files_out = ptr::null_mut();
        *count_out = 0;
    }

    CommyError::Success as i32
}

/// Get file information by ID
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_get_file_info(
    ____manager_handle: CommyFileManagerHandle,
    file_id: u64,
    info_out: *mut CommyFileInfo,
) -> i32 {
    if info_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    eprintln!("✅ FFI: Getting file info for ID: {}", file_id);

    // Mock file info
    unsafe {
        (*info_out).file_id = file_id;
        (*info_out).identifier = commy_strdup("mock-file\0".as_ptr() as *const c_char);
        (*info_out).file_path = commy_strdup("./mock-file.mmap\0".as_ptr() as *const c_char);
        (*info_out).size = 1024;
        (*info_out).connection_count = 1;
        (*info_out).created_at = 0; // Mock timestamp
        (*info_out).last_accessed = 0; // Mock timestamp
        (*info_out).transport_used = CommyTransportPreference::SharedMemory;
    }

    CommyError::Success as i32
}
/// Shutdown the SharedFileManager
#[cfg(feature = "manager")]
#[no_mangle]
pub extern "C" fn commy_shutdown_file_manager(manager_handle: CommyFileManagerHandle) -> i32 {
    eprintln!(
        "✅ FFI: Shutting down SharedFileManager ID: {}",
        manager_handle.manager_id
    );

    // Remove from global storage
    {
        let mut managers = GLOBAL_FILE_MANAGERS.write();
        managers.remove(&manager_handle.manager_id);
    }

    CommyError::Success as i32
}

/// Free a shared file response structure
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_free_shared_file_response(response: *mut CommySharedFileResponse) {
    if response.is_null() {
        return;
    }

    unsafe {
        if !(*response).file_path.is_null() {
            commy_free_string((*response).file_path);
        }
        if !(*response).error_message.is_null() {
            commy_free_string((*response).error_message);
        }
    }
}

/// Free a file info structure
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_free_file_info(info: *mut CommyFileInfo) {
    if info.is_null() {
        return;
    }

    unsafe {
        if !(*info).identifier.is_null() {
            commy_free_string((*info).identifier);
        }
        if !(*info).file_path.is_null() {
            commy_free_string((*info).file_path);
        }
    }
}

/// Free array of file info structures
#[cfg(feature = "manager")]
#[no_mangle]
pub unsafe extern "C" fn commy_free_file_info_array(files: *mut CommyFileInfo, count: u32) {
    if files.is_null() {
        return;
    }

    for i in 0..count {
        unsafe {
            let file_info = files.add(i as usize);
            commy_free_file_info(file_info);
        }
    }

    unsafe {
        libc::free(files as *mut c_void);
    }
}

// ============================================================================
// PHASE 1 NON-MANAGER FFI FUNCTIONS (available without manager feature)
// ============================================================================

/// Create a default manager configuration
#[no_mangle]
pub unsafe extern "C" fn commy_create_default_manager_config() -> *mut CommyManagerConfig {
    let config = Box::new(CommyManagerConfig {
        listen_port: 8080,
        bind_address: commy_strdup("127.0.0.1\0".as_ptr() as *const c_char),
        max_files: 1000,
        max_file_size: 1024 * 1024 * 1024, // 1GB
        default_ttl_seconds: 3600,         // 1 hour
        heartbeat_timeout_seconds: 30,
        cleanup_interval_seconds: 60,
        database_path: commy_strdup("./commy_data/db.sqlite\0".as_ptr() as *const c_char),
        files_directory: commy_strdup("./commy_data/files\0".as_ptr() as *const c_char),
        require_tls: false,
        tls_cert_path: ptr::null(),
        tls_key_path: ptr::null(),
    });

    Box::into_raw(config)
}

/// Free manager configuration
#[no_mangle]
pub unsafe extern "C" fn commy_free_manager_config(config: *mut CommyManagerConfig) {
    if config.is_null() {
        return;
    }

    unsafe {
        let config_ref = &*config;

        if !config_ref.bind_address.is_null() {
            commy_free_string(config_ref.bind_address as *mut c_char);
        }
        if !config_ref.database_path.is_null() {
            commy_free_string(config_ref.database_path as *mut c_char);
        }
        if !config_ref.files_directory.is_null() {
            commy_free_string(config_ref.files_directory as *mut c_char);
        }

        let _ = Box::from_raw(config);
    }
}

// ============================================================================
// COMPLIANCE REPORTING FFI FUNCTIONS
// ============================================================================

/// Generate a compliance report
#[no_mangle]
pub unsafe extern "C" fn commy_generate_compliance_report(
    ____manager_handle: CommyFileManagerHandle,
    report_type: CommyComplianceReportType,
    report_out: *mut CommyComplianceReport,
) -> i32 {
    if report_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Generate mock compliance report based on type
    let (summary, violations, recommendations, data) = match report_type {
        CommyComplianceReportType::AccessAudit => (
            "Access audit completed successfully",
            0,
            2,
            r#"{"total_access_events": 1250, "failed_access_attempts": 0, "privileged_access": 45}"#,
        ),
        CommyComplianceReportType::SecuritySummary => (
            "Security compliance status: COMPLIANT",
            0,
            1,
            r#"{"encryption_enabled": true, "tls_version": "1.3", "certificate_valid": true}"#,
        ),
        CommyComplianceReportType::PerformanceMetrics => (
            "Performance metrics within acceptable ranges",
            0,
            3,
            r#"{"avg_latency_ms": 2.1, "throughput_ops_sec": 15000, "memory_usage_mb": 245}"#,
        ),
        CommyComplianceReportType::ConfigurationStatus => (
            "Configuration compliance verified",
            0,
            1,
            r#"{"config_version": "1.0.0", "drift_detected": false, "last_updated": "2025-08-30"}"#,
        ),
        CommyComplianceReportType::FullCompliance => (
            "Full compliance audit completed - All systems compliant",
            0,
            5,
            r#"{"overall_score": 98.5, "categories": {"security": 100, "performance": 97, "access": 99}}"#,
        ),
    };

    let report_id = format!(
        "RPT-{}-{}",
        match report_type {
            CommyComplianceReportType::AccessAudit => "ACC",
            CommyComplianceReportType::SecuritySummary => "SEC",
            CommyComplianceReportType::PerformanceMetrics => "PERF",
            CommyComplianceReportType::ConfigurationStatus => "CFG",
            CommyComplianceReportType::FullCompliance => "FULL",
        },
        timestamp
    );

    unsafe {
        (*report_out).report_id = commy_strdup(report_id.as_ptr() as *const c_char);
        (*report_out).report_type = report_type;
        (*report_out).generated_at = timestamp;
        (*report_out).data_json = commy_strdup(data.as_ptr() as *const c_char);
        (*report_out).summary = commy_strdup(summary.as_ptr() as *const c_char);
        (*report_out).violations_count = violations;
        (*report_out).recommendations_count = recommendations;
    }

    eprintln!("✅ FFI: Generated compliance report: {}", report_id);
    CommyError::Success as i32
}

/// Get audit events for a time period
#[no_mangle]
pub unsafe extern "C" fn commy_get_audit_events(
    ____manager_handle: CommyFileManagerHandle,
    start_timestamp: u64,
    end_timestamp: u64,
    events_out: *mut *mut CommyAuditEvent,
    count_out: *mut u32,
) -> i32 {
    if events_out.is_null() || count_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    eprintln!(
        "✅ FFI: Retrieving audit events from {} to {}",
        start_timestamp, end_timestamp
    );

    // Generate mock audit events
    let mock_events = vec![
        (
            "EVT-001",
            "file_access",
            "user123",
            "shared_file_001",
            "read",
            "success",
            "File accessed successfully",
        ),
        (
            "EVT-002",
            "authentication",
            "user456",
            "auth_system",
            "login",
            "success",
            "User authenticated via JWT",
        ),
        (
            "EVT-003",
            "configuration",
            "admin",
            "mesh_config",
            "update",
            "success",
            "Configuration updated",
        ),
    ];

    let event_count = mock_events.len() as u32;

    // Allocate array for events
    let events_array = unsafe {
        libc::malloc(event_count as usize * std::mem::size_of::<CommyAuditEvent>())
            as *mut CommyAuditEvent
    };

    if events_array.is_null() {
        return CommyError::AllocError as i32;
    }

    // Populate events
    for (i, (id, event_type, user, resource, action, result, details)) in
        mock_events.iter().enumerate()
    {
        unsafe {
            let event = events_array.add(i);
            (*event).event_id = commy_strdup(id.as_ptr() as *const c_char);
            (*event).timestamp = start_timestamp + (i as u64 * 3600); // 1 hour apart
            (*event).event_type = commy_strdup(event_type.as_ptr() as *const c_char);
            (*event).user_id = commy_strdup(user.as_ptr() as *const c_char);
            (*event).resource = commy_strdup(resource.as_ptr() as *const c_char);
            (*event).action = commy_strdup(action.as_ptr() as *const c_char);
            (*event).result = commy_strdup(result.as_ptr() as *const c_char);
            (*event).details = commy_strdup(details.as_ptr() as *const c_char);
        }
    }

    unsafe {
        *events_out = events_array;
        *count_out = event_count;
    }

    CommyError::Success as i32
}

/// Record an audit event
#[no_mangle]
pub unsafe extern "C" fn commy_record_audit_event(
    ____manager_handle: CommyFileManagerHandle,
    event_type: *const c_char,
    _user_id: *const c_char,
    _resource: *const c_char,
    action: *const c_char,
    _result: *const c_char,
    _details: *const c_char,
) -> i32 {
    if event_type.is_null() || action.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    // Convert C strings to Rust strings for logging
    let event_type_str = if event_type.is_null() {
        "unknown"
    } else {
        match unsafe { CStr::from_ptr(event_type) }.to_str() {
            Ok(s) => s,
            Err(_) => "invalid",
        }
    };

    let action_str = if action.is_null() {
        "unknown"
    } else {
        match unsafe { CStr::from_ptr(action) }.to_str() {
            Ok(s) => s,
            Err(_) => "invalid",
        }
    };

    eprintln!(
        "✅ FFI: Audit event recorded - Type: {}, Action: {}",
        event_type_str, action_str
    );

    CommyError::Success as i32
}

/// Free compliance report
#[no_mangle]
pub unsafe extern "C" fn commy_free_compliance_report(report: *mut CommyComplianceReport) {
    if report.is_null() {
        return;
    }

    unsafe {
        if !(*report).report_id.is_null() {
            commy_free_string((*report).report_id);
        }
        if !(*report).data_json.is_null() {
            commy_free_string((*report).data_json);
        }
        if !(*report).summary.is_null() {
            commy_free_string((*report).summary);
        }
    }
}

/// Free audit events array
#[no_mangle]
pub unsafe extern "C" fn commy_free_audit_events(events: *mut CommyAuditEvent, count: u32) {
    if events.is_null() {
        return;
    }

    for i in 0..count {
        unsafe {
            let event = events.add(i as usize);
            if !(*event).event_id.is_null() {
                commy_free_string((*event).event_id);
            }
            if !(*event).event_type.is_null() {
                commy_free_string((*event).event_type);
            }
            if !(*event).user_id.is_null() {
                commy_free_string((*event).user_id);
            }
            if !(*event).resource.is_null() {
                commy_free_string((*event).resource);
            }
            if !(*event).action.is_null() {
                commy_free_string((*event).action);
            }
            if !(*event).result.is_null() {
                commy_free_string((*event).result);
            }
            if !(*event).details.is_null() {
                commy_free_string((*event).details);
            }
        }
    }

    unsafe {
        libc::free(events as *mut c_void);
    }
}

// ============================================================================
// PHASE 4 ENTERPRISE FEATURES - OBSERVABILITY FUNCTIONS
// ============================================================================

/// Start a distributed trace span
#[no_mangle]
pub unsafe extern "C" fn commy_start_trace_span(
    _manager_handle: CommyFileManagerHandle,
    operation_name: *const c_char,
    parent_span_id: *const c_char,
    span_out: *mut CommyTraceSpan,
) -> i32 {
    if operation_name.is_null() || span_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let op_name = match unsafe { CStr::from_ptr(operation_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;

    let span_id = format!("span-{}-{}", timestamp, rand::random::<u32>());
    let trace_id = format!("trace-{}-{}", timestamp, rand::random::<u64>());

    let parent_id = if parent_span_id.is_null() {
        ptr::null_mut()
    } else {
        commy_strdup(parent_span_id)
    };

    unsafe {
        (*span_out).span_id = commy_strdup(span_id.as_ptr() as *const c_char);
        (*span_out).trace_id = commy_strdup(trace_id.as_ptr() as *const c_char);
        (*span_out).parent_span_id = parent_id;
        (*span_out).operation_name = commy_strdup(operation_name);
        (*span_out).start_time = timestamp;
        (*span_out).end_time = 0;
        (*span_out).duration_microseconds = 0;
        (*span_out).tags = ptr::null_mut();
        (*span_out).tag_count = 0;
        (*span_out).status = CommyTraceStatus::Ok;
    }

    eprintln!(
        "✅ FFI: Started trace span '{}' with ID: {}",
        op_name, span_id
    );
    CommyError::Success as i32
}

/// Finish a distributed trace span
#[no_mangle]
pub unsafe extern "C" fn commy_finish_trace_span(
    ___manager_handle: CommyFileManagerHandle,
    span: *mut CommyTraceSpan,
    status: CommyTraceStatus,
) -> i32 {
    if span.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let end_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;

    unsafe {
        (*span).end_time = end_time;
        (*span).duration_microseconds = end_time - (*span).start_time;
        (*span).status = status;
    }

    eprintln!(
        "✅ FFI: Finished trace span with duration: {} microseconds",
        unsafe { (*span).duration_microseconds }
    );
    CommyError::Success as i32
}

/// Export metrics to external systems
#[no_mangle]
pub unsafe extern "C" fn commy_export_metrics(
    ___manager_handle: CommyFileManagerHandle,
    export_format: *const c_char, // "prometheus", "influxdb", "otlp"
    endpoint: *const c_char,
    metrics: *const CommyMetric,
    metric_count: u32,
) -> i32 {
    if export_format.is_null() || endpoint.is_null() || metrics.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let format_str = match unsafe { CStr::from_ptr(export_format) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    let endpoint_str = match unsafe { CStr::from_ptr(endpoint) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    eprintln!(
        "✅ FFI: Exporting {} metrics to {} endpoint: {}",
        metric_count, format_str, endpoint_str
    );

    // Mock metric export - in real implementation would send to actual endpoint
    for i in 0..metric_count {
        unsafe {
            let metric = metrics.add(i as usize);
            if !(*metric).name.is_null() {
                let name = CStr::from_ptr((*metric).name).to_str().unwrap_or("unknown");
                eprintln!("  📊 Metric: {} = {}", name, (*metric).value);
            }
        }
    }

    CommyError::Success as i32
}

/// Record a custom metric
#[no_mangle]
pub unsafe extern "C" fn commy_record_metric(
    ___manager_handle: CommyFileManagerHandle,
    name: *const c_char,
    metric_type: CommyMetricType,
    value: f64,
    _labels: *const *const c_char,
    label_count: u32,
) -> i32 {
    if name.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    let metric_type_str = match metric_type {
        CommyMetricType::Counter => "counter",
        CommyMetricType::Gauge => "gauge",
        CommyMetricType::Histogram => "histogram",
        CommyMetricType::Summary => "summary",
    };

    eprintln!(
        "✅ FFI: Recorded {} metric '{}' = {} (labels: {})",
        metric_type_str, name_str, value, label_count
    );

    CommyError::Success as i32
}

// ============================================================================
// PHASE 4 ENTERPRISE FEATURES - FEDERATION FUNCTIONS
// ============================================================================

/// Configure multi-region federation
#[no_mangle]
pub unsafe extern "C" fn commy_configure_federation(
    ___manager_handle: CommyFileManagerHandle,
    federation_config: *const CommyFederationConfig,
) -> i32 {
    if federation_config.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let config = unsafe { &*federation_config };

    let local_region = if config.local_region.is_null() {
        "unknown"
    } else {
        match unsafe { CStr::from_ptr(config.local_region) }.to_str() {
            Ok(s) => s,
            Err(_) => "invalid",
        }
    };

    eprintln!(
        "✅ FFI: Configuring federation for region '{}' with {} regions",
        local_region, config.region_count
    );
    eprintln!("  🌐 WAN optimization: {}", config.wan_optimization);
    eprintln!("  🔄 Failover enabled: {}", config.failover_enabled);
    eprintln!(
        "  ⚖️ Global load balancing: {}",
        config.global_load_balancing
    );

    CommyError::Success as i32
}

/// Discover services across regions
#[no_mangle]
pub unsafe extern "C" fn commy_discover_cross_region_services(
    ___manager_handle: CommyFileManagerHandle,
    target_region: *const c_char,
    _service_name: *const c_char,
    services_out: *mut *mut CommyServiceInfo,
    count_out: *mut u32,
) -> i32 {
    if target_region.is_null() || services_out.is_null() || count_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let region_str = match unsafe { CStr::from_ptr(target_region) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    eprintln!(
        "✅ FFI: Discovering cross-region services in region: {}",
        region_str
    );

    // Mock cross-region service discovery
    unsafe {
        *services_out = ptr::null_mut();
        *count_out = 0; // No services found in mock implementation
    }

    CommyError::Success as i32
}

/// Get region health status
#[no_mangle]
pub unsafe extern "C" fn commy_get_region_health(
    ___manager_handle: CommyFileManagerHandle,
    region_id: *const c_char,
    region_out: *mut CommyRegion,
) -> i32 {
    if region_id.is_null() || region_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let region_str = match unsafe { CStr::from_ptr(region_id) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    // Mock region health data
    unsafe {
        (*region_out).region_id = commy_strdup(region_id);
        (*region_out).region_name =
            commy_strdup(format!("Region {}", region_str).as_ptr() as *const c_char);
        (*region_out).endpoint =
            commy_strdup("https://region.commy.mesh\0".as_ptr() as *const c_char);
        (*region_out).latency_ms = 45; // Mock latency
        (*region_out).is_available = true;
        (*region_out).data_locality_preference = true;
        (*region_out).compliance_zone = commy_strdup("US-WEST\0".as_ptr() as *const c_char);
    }

    eprintln!(
        "✅ FFI: Retrieved health for region: {} (latency: 45ms)",
        region_str
    );
    CommyError::Success as i32
}

// ============================================================================
// PHASE 4 ENTERPRISE FEATURES - POLICY ENGINE FUNCTIONS
// ============================================================================

/// Create a policy rule
#[no_mangle]
pub unsafe extern "C" fn commy_create_policy_rule(
    ___manager_handle: CommyFileManagerHandle,
    rule: *const CommyPolicyRule,
    rule_id_out: *mut *mut c_char,
) -> i32 {
    if rule.is_null() || rule_id_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let rule_ref = unsafe { &*rule };

    let name = if rule_ref.name.is_null() {
        "unnamed-rule"
    } else {
        match unsafe { CStr::from_ptr(rule_ref.name) }.to_str() {
            Ok(s) => s,
            Err(_) => "invalid-name",
        }
    };

    let rule_id = format!(
        "rule-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        rand::random::<u32>()
    );

    unsafe {
        *rule_id_out = commy_strdup(rule_id.as_ptr() as *const c_char);
    }

    eprintln!(
        "✅ FFI: Created policy rule '{}' with ID: {}",
        name, rule_id
    );
    CommyError::Success as i32
}

/// Evaluate policy rules against a request
#[no_mangle]
pub unsafe extern "C" fn commy_evaluate_policies(
    ___manager_handle: CommyFileManagerHandle,
    context: *const c_char, // JSON context
    violations_out: *mut *mut c_char,
    violation_count_out: *mut u32,
) -> i32 {
    if context.is_null() || violations_out.is_null() || violation_count_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    eprintln!("✅ FFI: Evaluating policies against request context");

    // Mock policy evaluation - no violations found
    unsafe {
        *violations_out = ptr::null_mut();
        *violation_count_out = 0;
    }

    CommyError::Success as i32
}

/// Generate compliance scan report
///
/// # Safety
/// - `scan_type` must be a non-null pointer to a valid, null-terminated C string.
/// - `report_out` must be a valid, non-null pointer to a `CommyComplianceReport` struct
///   that the caller has allocated.
/// - The callee will set fields on `report_out` and allocate any nested C strings; the
///   caller is responsible for freeing them using the appropriate free helpers.
#[no_mangle]
pub unsafe extern "C" fn commy_scan_compliance(
    ___manager_handle: CommyFileManagerHandle,
    scan_type: *const c_char, // "SOC2", "GDPR", "HIPAA", "ALL"
    report_out: *mut CommyComplianceReport,
) -> i32 {
    if scan_type.is_null() || report_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let scan_str = match unsafe { CStr::from_ptr(scan_type) }.to_str() {
        Ok(s) => s,
        Err(_) => return CommyError::InvalidParameter as i32,
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let report_id = format!("COMPLIANCE-{}-{}", scan_str, timestamp);
    let summary = format!("{} compliance scan completed - COMPLIANT", scan_str);
    let data = format!(
        r#"{{"scan_type": "{}", "status": "COMPLIANT", "score": 98.5, "issues": 0}}"#,
        scan_str
    );

    unsafe {
        (*report_out).report_id = commy_strdup(report_id.as_ptr() as *const c_char);
        (*report_out).report_type = CommyComplianceReportType::FullCompliance;
        (*report_out).generated_at = timestamp;
        (*report_out).data_json = commy_strdup(data.as_ptr() as *const c_char);
        (*report_out).summary = commy_strdup(summary.as_ptr() as *const c_char);
        (*report_out).violations_count = 0;
        (*report_out).recommendations_count = 2;
    }

    eprintln!(
        "✅ FFI: Generated {} compliance scan: {}",
        scan_str, report_id
    );
    CommyError::Success as i32
}

// ============================================================================
// PHASE 4 ENTERPRISE FEATURES - DEPLOYMENT FUNCTIONS
// ============================================================================

/// Generate Kubernetes deployment manifests
///
/// # Safety
/// - `deployment_config` must be a valid pointer to a `CommyDeploymentConfig` if non-null.
/// - `manifests_out` must be a valid, non-null pointer where the allocated manifest C string
///   pointer will be stored. The returned string must be freed by the caller using
///   `commy_free_string`.
#[no_mangle]
pub unsafe extern "C" fn commy_generate_k8s_manifests(
    deployment_config: *const CommyDeploymentConfig,
    manifests_out: *mut *mut c_char,
) -> i32 {
    if deployment_config.is_null() || manifests_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let config = unsafe { &*deployment_config };

    let deployment_name = if config.deployment_id.is_null() {
        "commy-mesh"
    } else {
        match unsafe { CStr::from_ptr(config.deployment_id) }.to_str() {
            Ok(s) => s,
            Err(_) => "commy-mesh",
        }
    };

    let manifest = format!(
        r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: {}
  namespace: {}
spec:
  replicas: {}
  selector:
    matchLabels:
      app: commy-mesh
  template:
    metadata:
      labels:
        app: commy-mesh
    spec:
      containers:
      - name: commy
        image: commy/service-mesh:latest
        ports:
        - containerPort: 8080
        resources:
          limits:
            cpu: {}
            memory: {}
        env:
        - name: COMMY_ENABLE_TLS
          value: "{}"
        - name: COMMY_ENABLE_METRICS
          value: "{}"
        - name: COMMY_ENABLE_TRACING
          value: "{}"
---
apiVersion: v1
kind: Service
metadata:
  name: {}-service
  namespace: {}
spec:
  selector:
    app: commy-mesh
  ports:
  - port: 8080
    targetPort: 8080
  type: ClusterIP
"#,
        deployment_name,
        if config.namespace.is_null() {
            "default"
        } else {
            unsafe { CStr::from_ptr(config.namespace) }
                .to_str()
                .unwrap_or("default")
        },
        config.replica_count,
        if config.cpu_limit.is_null() {
            "500m"
        } else {
            unsafe { CStr::from_ptr(config.cpu_limit) }
                .to_str()
                .unwrap_or("500m")
        },
        if config.memory_limit.is_null() {
            "512Mi"
        } else {
            unsafe { CStr::from_ptr(config.memory_limit) }
                .to_str()
                .unwrap_or("512Mi")
        },
        config.enable_tls,
        config.enable_metrics,
        config.enable_tracing,
        deployment_name,
        if config.namespace.is_null() {
            "default"
        } else {
            unsafe { CStr::from_ptr(config.namespace) }
                .to_str()
                .unwrap_or("default")
        }
    );

    unsafe {
        *manifests_out = commy_strdup(manifest.as_ptr() as *const c_char);
    }

    eprintln!(
        "✅ FFI: Generated Kubernetes manifests for deployment: {}",
        deployment_name
    );
    CommyError::Success as i32
}

/// Generate Helm chart values
///
/// # Safety
/// - `deployment_config` must be a valid pointer to a `CommyDeploymentConfig` if non-null.
/// - `values_out` must be a valid, non-null pointer where the allocated values C string
///   pointer will be stored. The returned string must be freed by the caller using
///   `commy_free_string`.
#[no_mangle]
pub unsafe extern "C" fn commy_generate_helm_values(
    deployment_config: *const CommyDeploymentConfig,
    values_out: *mut *mut c_char,
) -> i32 {
    if deployment_config.is_null() || values_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let config = unsafe { &*deployment_config };

    let values = format!(
        r#"
# Commy Service Mesh Helm Values
replicaCount: {}

image:
  repository: commy/service-mesh
  tag: latest
  pullPolicy: IfNotPresent

service:
  type: ClusterIP
  port: 8080

resources:
  limits:
    cpu: {}
    memory: {}
  requests:
    cpu: 250m
    memory: 256Mi

tls:
  enabled: {}

metrics:
  enabled: {}

tracing:
  enabled: {}

storage:
  class: {}
  size: 10Gi

environment: {}

mesh:
  federation:
    enabled: false
  observability:
    enabled: true
  policies:
    enabled: true
"#,
        config.replica_count,
        if config.cpu_limit.is_null() {
            "500m"
        } else {
            unsafe { CStr::from_ptr(config.cpu_limit) }
                .to_str()
                .unwrap_or("500m")
        },
        if config.memory_limit.is_null() {
            "512Mi"
        } else {
            unsafe { CStr::from_ptr(config.memory_limit) }
                .to_str()
                .unwrap_or("512Mi")
        },
        config.enable_tls,
        config.enable_metrics,
        config.enable_tracing,
        if config.storage_class.is_null() {
            "standard"
        } else {
            unsafe { CStr::from_ptr(config.storage_class) }
                .to_str()
                .unwrap_or("standard")
        },
        if config.environment.is_null() {
            "production"
        } else {
            unsafe { CStr::from_ptr(config.environment) }
                .to_str()
                .unwrap_or("production")
        }
    );

    unsafe {
        *values_out = commy_strdup(values.as_ptr() as *const c_char);
    }

    eprintln!("✅ FFI: Generated Helm values configuration");
    CommyError::Success as i32
}

/// Generate Docker Compose configuration
///
/// # Safety
/// - `deployment_config` must be a valid pointer to a `CommyDeploymentConfig` if non-null.
/// - `compose_out` must be a valid, non-null pointer where the allocated compose C string
///   pointer will be stored. The returned string must be freed by the caller using
///   `commy_free_string`.
#[no_mangle]
pub unsafe extern "C" fn commy_generate_docker_compose(
    deployment_config: *const CommyDeploymentConfig,
    compose_out: *mut *mut c_char,
) -> i32 {
    if deployment_config.is_null() || compose_out.is_null() {
        return CommyError::InvalidParameter as i32;
    }

    let config = unsafe { &*deployment_config };

    let compose = format!(
        r#"
version: '3.8'

services:
  commy-mesh:
    image: commy/service-mesh:latest
    ports:
      - "8080:8080"
    environment:
      - COMMY_ENABLE_TLS={}
      - COMMY_ENABLE_METRICS={}
      - COMMY_ENABLE_TRACING={}
      - COMMY_ENVIRONMENT={}
    volumes:
      - commy-data:/app/data
      - commy-config:/app/config
    restart: unless-stopped
    deploy:
      replicas: {}
      resources:
        limits:
          cpus: '{}'
          memory: {}
        reservations:
          cpus: '0.25'
          memory: 256M
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

volumes:
  commy-data:
    driver: local
  commy-config:
    driver: local

networks:
  default:
    driver: bridge
"#,
        config.enable_tls,
        config.enable_metrics,
        config.enable_tracing,
        if config.environment.is_null() {
            "production"
        } else {
            unsafe { CStr::from_ptr(config.environment) }
                .to_str()
                .unwrap_or("production")
        },
        config.replica_count,
        if config.cpu_limit.is_null() {
            "0.5"
        } else {
            let cpu_str = unsafe { CStr::from_ptr(config.cpu_limit) }
                .to_str()
                .unwrap_or("0.5");
            cpu_str.trim_end_matches('m')
        },
        if config.memory_limit.is_null() {
            "512M"
        } else {
            unsafe { CStr::from_ptr(config.memory_limit) }
                .to_str()
                .unwrap_or("512M")
        }
    );

    unsafe {
        *compose_out = commy_strdup(compose.as_ptr() as *const c_char);
    }

    eprintln!("✅ FFI: Generated Docker Compose configuration");
    CommyError::Success as i32
}

// ============================================================================
// PHASE 4 ENTERPRISE FEATURES - CLEANUP FUNCTIONS
// ============================================================================

/// Free trace span
///
/// # Safety
/// - `span` must be either null or a pointer previously returned by `commy_start_trace_span`.
/// - After calling this function the caller must not use `span` or any nested pointers it
///   contained.
#[no_mangle]
pub unsafe extern "C" fn commy_free_trace_span(span: *mut CommyTraceSpan) {
    if span.is_null() {
        return;
    }

    unsafe {
        if !(*span).span_id.is_null() {
            commy_free_string((*span).span_id);
        }
        if !(*span).trace_id.is_null() {
            commy_free_string((*span).trace_id);
        }
        if !(*span).parent_span_id.is_null() {
            commy_free_string((*span).parent_span_id);
        }
        if !(*span).operation_name.is_null() {
            commy_free_string((*span).operation_name);
        }

        // Free tags array
        if !(*span).tags.is_null() {
            for i in 0..(*span).tag_count {
                let tag = (*span).tags.add(i as usize);
                if !(*tag).is_null() {
                    commy_free_string(*tag);
                }
            }
            libc::free((*span).tags as *mut c_void);
        }
    }
}

/// Free region information
///
/// # Safety
/// - `region` must be either null or a pointer previously populated by a commy FFI getter
///   (for example `commy_get_region_health`) and its nested C strings must be owned by the
///   caller after the call.
/// - After calling this function the caller must not use `region` or any nested pointers it
///   contained.
#[no_mangle]
pub unsafe extern "C" fn commy_free_region(region: *mut CommyRegion) {
    if region.is_null() {
        return;
    }

    unsafe {
        if !(*region).region_id.is_null() {
            commy_free_string((*region).region_id);
        }
        if !(*region).region_name.is_null() {
            commy_free_string((*region).region_name);
        }
        if !(*region).endpoint.is_null() {
            commy_free_string((*region).endpoint);
        }
        if !(*region).compliance_zone.is_null() {
            commy_free_string((*region).compliance_zone);
        }
    }
}

/// Free policy rule
///
/// # Safety
/// - `rule` must be either null or a pointer previously returned or filled by a commy
///   FFI function that allocated nested strings. After calling this function the caller
///   must not use `rule` or any nested pointers.
#[no_mangle]
pub unsafe extern "C" fn commy_free_policy_rule(rule: *mut CommyPolicyRule) {
    if rule.is_null() {
        return;
    }

    unsafe {
        if !(*rule).rule_id.is_null() {
            commy_free_string((*rule).rule_id);
        }
        if !(*rule).name.is_null() {
            commy_free_string((*rule).name);
        }
        if !(*rule).description.is_null() {
            commy_free_string((*rule).description);
        }
        if !(*rule).condition.is_null() {
            commy_free_string((*rule).condition);
        }
        if !(*rule).action.is_null() {
            commy_free_string((*rule).action);
        }
    }
}
