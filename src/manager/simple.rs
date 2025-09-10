//! Simplified Manager Module for Phase 1
//! Focus on essential functionality

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod core;
pub mod protocol;

/// Simple file request for Phase 1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFileRequest {
    /// Unique identifier for the file
    pub identifier: String,

    /// Optional custom file path
    pub file_path: Option<PathBuf>,

    /// Maximum size in bytes
    pub max_size_bytes: Option<u64>,

    /// How to handle file existence
    pub existence_policy: ExistencePolicy,

    /// Required permissions
    pub required_permissions: Vec<Permission>,
}

/// Simple existence policies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExistencePolicy {
    /// Create file if needed, connect if exists
    CreateOrConnect,
    /// Only create new files
    CreateOnly,
    /// Only connect to existing files
    ConnectOnly,
}

/// Basic permission types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

/// Simple file status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileStatus {
    Active,
    Inactive,
    Error(String),
}

/// Response when a shared file is allocated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFileResponse {
    /// Unique identifier for this file allocation
    pub file_id: u64,
    /// Local path to the memory-mapped file
    pub file_path: PathBuf,
    /// Metadata about the allocated file
    pub metadata: FileMetadata,
    /// Information about the active transport
    pub transport: ActiveTransport,
    /// Performance characteristics of this connection
    pub performance_profile: PerformanceProfile,
    /// Security context for this connection
    pub security_context: SecurityContext,
}

/// Metadata about an allocated shared file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Original request that created this file
    pub original_request: SharedFileRequest,
    /// When the file was created
    pub created_at: DateTime<Utc>,
    /// Current number of connected clients
    pub connection_count: u32,
    /// Total size of the file in bytes
    pub size_bytes: u64,
    /// Current status of the file
    pub status: FileStatus,
    /// Performance statistics
    pub stats: FileStatistics,
}

/// File statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileStatistics {
    /// Total number of read operations
    pub read_count: u64,
    /// Total number of write operations
    pub write_count: u64,
    /// Total bytes read
    pub bytes_read: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Average latency in microseconds
    pub avg_latency_us: f64,
    /// Peak concurrent connections
    pub peak_connections: u32,
    /// Last access time
    pub last_access: DateTime<Utc>,
}

/// Information about the active transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActiveTransport {
    /// Local shared memory
    SharedMemory {
        file_path: PathBuf,
        local_peers: Vec<u32>,
    },
    /// Network transport
    Network { endpoints: Vec<String> },
}

/// Performance profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    /// Expected latency in microseconds
    pub expected_latency_us: u32,
    /// Expected throughput in MB/s
    pub expected_throughput_mbps: u32,
    /// High performance mode
    pub high_performance: bool,
    /// Performance tier
    pub tier: PerformanceTier,
}

/// Performance tiers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PerformanceTier {
    UltraLow,
    Low,
    Standard,
    High,
}

/// Security context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Auth token
    pub auth_token: String,
    /// Permissions granted
    pub permissions: Vec<Permission>,
    /// User identity
    pub identity: String,
    /// Session ID
    pub session_id: String,
    /// Security level
    pub security_level: SecurityLevel,
}

/// Security levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityLevel {
    None,
    Standard,
    High,
    Maximum,
}

/// Shared file information for tracking
#[derive(Debug, Clone)]
pub struct SharedFileInfo {
    /// File metadata
    pub metadata: FileMetadata,
    /// Transport info
    pub transport: ActiveTransport,
    /// Performance profile
    pub performance_profile: PerformanceProfile,
}

// Re-export the main manager
pub use core::SharedFileManager;
