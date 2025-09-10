//! Unified Error Handling System
//!
//! This module provides a comprehensive, type-safe error handling system
//! using thiserror for all Commy components.

use std::io;
use std::num::ParseIntError;
use std::path::PathBuf;
use thiserror::Error;

/// Main error type for all Commy operations
#[derive(Error, Debug)]
pub enum CommyError {
    // I/O and File System Errors
    #[error("I/O error: {source} (path: {path:?})")]
    Io {
        source: io::Error,
        path: Option<PathBuf>,
    },

    #[error("Failed to create file at path: {path}")]
    FileCreation { path: PathBuf },

    #[error("Failed to access file: {path}")]
    FileAccess { path: PathBuf },

    #[error("File size {size} exceeds maximum allowed size {max_size}")]
    FileSizeExceeded { size: u64, max_size: u64 },

    #[error("Disk space insufficient: requested {requested} bytes, available {available} bytes")]
    InsufficientDiskSpace { requested: u64, available: u64 },

    // Serialization Errors
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("Binary serialization error: {0}")]
    BinarySerialization(String),

    #[error("MessagePack serialization error: {0}")]
    MessagePackSerialization(String),

    #[error("CBOR serialization error: {0}")]
    CborSerialization(String),

    #[error("Unsupported serialization format: {format}")]
    UnsupportedFormat { format: String },

    // Compatibility (legacy) variants used by some internal modules. These
    // mirror the smaller `crate::errors::CommyError` shape and are kept during
    // the conservative migration to avoid large, risky diffs. Plan is to
    // remove these once all call sites are migrated to the richer variants
    // above.
    // legacy Serialize(String) removed after migration to format-specific variants
    #[error("buffer too small")]
    BufferTooSmall,

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("plugin load error: {0}")]
    PluginLoad(String),

    #[error("other: {0}")]
    Other(String),

    // Network and Transport Errors
    #[error("Network connection failed to {host}:{port}")]
    NetworkConnection { host: String, port: u16 },

    #[error("Transport error: {message}")]
    Transport { message: String },

    #[error("Transport selection failed: {reason}")]
    TransportSelection { reason: String },

    #[error("Transport timeout after {timeout_ms}ms")]
    TransportTimeout { timeout_ms: u64 },

    #[error("Transport unavailable: {transport_type}")]
    TransportUnavailable { transport_type: String },

    // Security and Authentication Errors
    #[error("Authentication failed: {reason}")]
    Authentication { reason: String },

    #[error("Authorization denied for operation: {operation}")]
    Authorization { operation: String },

    #[error("Invalid JWT token: {reason}")]
    InvalidToken { reason: String },

    #[error("TLS/SSL error: {0}")]
    Tls(String),

    #[error("Security policy violation: {policy}")]
    SecurityPolicyViolation { policy: String },

    // Configuration and Validation Errors
    #[error("Configuration error: {field} - {message}")]
    Configuration { field: String, message: String },

    #[error("Validation failed for {field}: {reason}")]
    Validation { field: String, reason: String },

    #[error("Missing required configuration: {field}")]
    MissingConfiguration { field: String },

    #[error("Invalid configuration value: {field} = {value}")]
    InvalidConfiguration { field: String, value: String },

    // Parsing and Conversion Errors
    #[error("Parse error: {0}")]
    Parse(#[from] ParseIntError),

    #[error("Failed to parse {field}: {reason}")]
    ParseField { field: String, reason: String },

    #[error("Type conversion failed from {from_type} to {to_type}")]
    TypeConversion { from_type: String, to_type: String },

    // Manager and Resource Errors
    #[error("Manager not initialized")]
    ManagerNotInitialized,

    #[error("Resource allocation failed: {resource}")]
    ResourceAllocation { resource: String },

    #[error("Resource limit exceeded: {resource} (limit: {limit})")]
    ResourceLimitExceeded { resource: String, limit: u64 },

    #[error("Resource not found: {resource}")]
    ResourceNotFound { resource: String },

    #[error("Resource already exists: {resource}")]
    ResourceAlreadyExists { resource: String },

    // Concurrency and Threading Errors
    #[error("Lock acquisition timeout for: {resource}")]
    LockTimeout { resource: String },

    #[error("Deadlock detected involving: {resources:?}")]
    Deadlock { resources: Vec<String> },

    #[error("Channel communication error: {0}")]
    Channel(String),

    #[error("Task join error: {0}")]
    TaskJoin(String),

    // FFI and Interoperability Errors
    #[error("FFI error: {operation} - {message}")]
    Ffi { operation: String, message: String },

    #[error("Null pointer error in {function}")]
    NullPointer { function: String },

    #[error("Invalid pointer provided to {function}")]
    InvalidPointer { function: String },

    #[error("String conversion error: {0}")]
    StringConversion(String),

    // Service Mesh and Discovery Errors
    #[error("Service discovery failed: {service}")]
    ServiceDiscovery { service: String },

    #[error("Load balancer error: {reason}")]
    LoadBalancer { reason: String },

    #[error("Health check failed for service: {service}")]
    HealthCheck { service: String },

    #[error("Node registration failed: {node_id}")]
    NodeRegistration { node_id: String },

    // Performance and Monitoring Errors
    #[error("Performance threshold exceeded: {metric} = {value} (max: {threshold})")]
    PerformanceThreshold {
        metric: String,
        value: f64,
        threshold: f64,
    },

    #[error("Monitoring system error: {0}")]
    Monitoring(String),

    #[error("Metrics collection failed: {metric}")]
    MetricsCollection { metric: String },

    // Protocol and Message Errors
    #[error("Protocol error: unsupported version {version}")]
    UnsupportedProtocolVersion { version: String },

    #[error("Invalid message format: {reason}")]
    InvalidMessage { reason: String },

    #[error("Message size {size} exceeds maximum {max_size}")]
    MessageSizeExceeded { size: usize, max_size: usize },

    #[error("Corrupted message: {checksum_expected} != {checksum_actual}")]
    MessageCorrupted {
        checksum_expected: String,
        checksum_actual: String,
    },

    // Internal and Unknown Errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("Unexpected error: {0}")]
    Unexpected(String),
}

/// Result type alias for Commy operations
pub type CommyResult<T> = Result<T, CommyError>;

/// Error context trait for adding additional context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> CommyResult<T>
    where
        F: FnOnce() -> String;

    /// Add simple string context
    fn context(self, msg: &str) -> CommyResult<T>;
}

impl<T> ErrorContext<T> for CommyResult<T> {
    fn with_context<F>(self, f: F) -> CommyResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| CommyError::Internal(format!("{}: {}", f(), e)))
    }

    fn context(self, msg: &str) -> CommyResult<T> {
        self.map_err(|e| CommyError::Internal(format!("{}: {}", msg, e)))
    }
}

impl<T> ErrorContext<T> for Result<T, std::io::Error> {
    fn with_context<F>(self, f: F) -> CommyResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| CommyError::Internal(format!("{}: {}", f(), e)))
    }

    fn context(self, msg: &str) -> CommyResult<T> {
        self.map_err(|e| CommyError::Internal(format!("{}: {}", msg, e)))
    }
}

/// Convenience macros for error creation
#[macro_export]
macro_rules! commy_error {
    ($variant:ident { $($field:ident: $value:expr),* }) => {
        $crate::error::CommyError::$variant { $($field: $value),* }
    };
    ($variant:ident($value:expr)) => {
        $crate::error::CommyError::$variant($value)
    };
}

#[macro_export]
macro_rules! bail {
    ($($args:tt)*) => {
        return Err(commy_error!($($args)*))
    };
}

#[macro_export]
macro_rules! ensure {
    ($condition:expr, $($args:tt)*) => {
        if !$condition {
            bail!($($args)*);
        }
    };
}

/// Error category for grouping related errors
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    FileSystem,
    Network,
    Security,
    Configuration,
    Serialization,
    Concurrency,
    Ffi,
    ServiceMesh,
    Performance,
    Protocol,
    Internal,
}

impl CommyError {
    /// Get the category of this error
    pub fn category(&self) -> ErrorCategory {
        match self {
            CommyError::Io { .. }
            | CommyError::FileCreation { .. }
            | CommyError::FileAccess { .. }
            | CommyError::FileSizeExceeded { .. }
            | CommyError::InsufficientDiskSpace { .. } => ErrorCategory::FileSystem,

            CommyError::NetworkConnection { .. }
            | CommyError::Transport { .. }
            | CommyError::TransportSelection { .. }
            | CommyError::TransportTimeout { .. }
            | CommyError::TransportUnavailable { .. } => ErrorCategory::Network,

            CommyError::Authentication { .. }
            | CommyError::Authorization { .. }
            | CommyError::InvalidToken { .. }
            | CommyError::Tls(_)
            | CommyError::SecurityPolicyViolation { .. } => ErrorCategory::Security,

            CommyError::Configuration { .. }
            | CommyError::Validation { .. }
            | CommyError::MissingConfiguration { .. }
            | CommyError::InvalidConfiguration { .. } => ErrorCategory::Configuration,

            CommyError::JsonSerialization(_)
            | CommyError::BinarySerialization(_)
            | CommyError::MessagePackSerialization(_)
            | CommyError::CborSerialization(_)
            | CommyError::UnsupportedFormat { .. } => ErrorCategory::Serialization,

            CommyError::LockTimeout { .. }
            | CommyError::Deadlock { .. }
            | CommyError::Channel(_)
            | CommyError::TaskJoin(_) => ErrorCategory::Concurrency,

            CommyError::Ffi { .. }
            | CommyError::NullPointer { .. }
            | CommyError::InvalidPointer { .. }
            | CommyError::StringConversion(_) => ErrorCategory::Ffi,

            CommyError::ServiceDiscovery { .. }
            | CommyError::LoadBalancer { .. }
            | CommyError::HealthCheck { .. }
            | CommyError::NodeRegistration { .. } => ErrorCategory::ServiceMesh,

            CommyError::PerformanceThreshold { .. }
            | CommyError::Monitoring(_)
            | CommyError::MetricsCollection { .. } => ErrorCategory::Performance,

            CommyError::UnsupportedProtocolVersion { .. }
            | CommyError::InvalidMessage { .. }
            | CommyError::MessageSizeExceeded { .. }
            | CommyError::MessageCorrupted { .. } => ErrorCategory::Protocol,

            _ => ErrorCategory::Internal,
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Retryable network errors
            CommyError::NetworkConnection { .. }
            | CommyError::TransportTimeout { .. }
            | CommyError::TransportUnavailable { .. } => true,

            // Retryable resource errors
            CommyError::LockTimeout { .. } | CommyError::ResourceAllocation { .. } => true,

            // Retryable service mesh errors
            CommyError::ServiceDiscovery { .. }
            | CommyError::LoadBalancer { .. }
            | CommyError::HealthCheck { .. } => true,

            // Non-retryable errors
            CommyError::Authentication { .. }
            | CommyError::Authorization { .. }
            | CommyError::Validation { .. }
            | CommyError::InvalidConfiguration { .. }
            | CommyError::FileSizeExceeded { .. } => false,

            // IO errors might be retryable depending on kind
            CommyError::Io {
                source: io_error, ..
            } => matches!(
                io_error.kind(),
                io::ErrorKind::Interrupted
                    | io::ErrorKind::WouldBlock
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::ConnectionAborted
            ),

            _ => false,
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            CommyError::FileCreation { path } => {
                format!("Could not create file at '{}'", path.display())
            }
            CommyError::NetworkConnection { host, port } => {
                format!("Could not connect to {}:{}", host, port)
            }
            CommyError::Authentication { .. } => {
                "Authentication failed. Please check your credentials.".to_string()
            }
            CommyError::Authorization { operation } => {
                format!("You don't have permission to perform: {}", operation)
            }
            CommyError::FileSizeExceeded { size, max_size } => {
                format!(
                    "File size ({} bytes) exceeds the maximum allowed ({} bytes)",
                    size, max_size
                )
            }
            _ => self.to_string(),
        }
    }
}

// Compatibility From impls removed as part of the migration to explicit
// `CommyError` construction at call-sites. This ensures error contexts such as
// file paths and serialization formats are preserved. Re-add only if a
// deliberate compatibility shim is required.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let io_error = CommyError::Io {
            source: io::Error::new(io::ErrorKind::NotFound, "test"),
            path: None,
        };
        assert_eq!(io_error.category(), ErrorCategory::FileSystem);

        let network_error = CommyError::NetworkConnection {
            host: "localhost".to_string(),
            port: 8080,
        };
        assert_eq!(network_error.category(), ErrorCategory::Network);
    }

    #[test]
    fn test_retryable_errors() {
        let timeout_error = CommyError::TransportTimeout { timeout_ms: 5000 };
        assert!(timeout_error.is_retryable());

        let auth_error = CommyError::Authentication {
            reason: "Invalid credentials".to_string(),
        };
        assert!(!auth_error.is_retryable());
    }

    #[test]
    fn test_error_context() {
        let result: Result<(), std::io::Error> =
            Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));

        let with_context = result.context("Failed to read configuration file");
        assert!(with_context.is_err());
        assert!(with_context
            .unwrap_err()
            .to_string()
            .contains("Failed to read configuration file"));
    }

    #[test]
    fn test_user_messages() {
        let error = CommyError::FileSizeExceeded {
            size: 2000,
            max_size: 1000,
        };
        let message = error.user_message();
        assert!(message.contains("2000 bytes"));
        assert!(message.contains("1000 bytes"));
    }
}
