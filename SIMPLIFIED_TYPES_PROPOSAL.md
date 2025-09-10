// Simplified Phase 1 Types for Commy
// Focus on essential functionality, add complexity incrementally

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// Simple file request for Phase 1
# [derive(Debug, Clone, Serialize, Deserialize)]
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
# [derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExistencePolicy {
    /// Create file if needed, connect if exists
    CreateOrConnect,
    /// Only create new files
    CreateOnly,
    /// Only connect to existing files
    ConnectOnly,
}

/// Basic permission types
# [derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

/// Simple file status
# [derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileStatus {
    Active,
    Inactive,
    Error(String),
}

/// Manager events for monitoring
# [derive(Debug, Clone, Serialize, Deserialize)]
pub enum ManagerEvent {
    FileCreated {
        file_id: u64,
        identifier: String,
        file_path: PathBuf,
        size_bytes: u64,
    },
    FileConnected {
        file_id: u64,
        identifier: String,
        connection_count: usize,
    },
    FileDisconnected {
        file_id: u64,
        identifier: String,
        remaining_connections: usize,
    },
    FileRemoved {
        file_id: u64,
        identifier: String,
        reason: String,
    },
    ClientDisconnected {
        client_id: String,
        session_id: Option<String>,
        disconnect_reason: String,
        disconnect_time: DateTime<Utc>,
    },
}

// Advanced features can be added later:
// - Topology specification
// - Serialization format selection
// - Performance requirements
// - Directionality constraints
// - Complex auth policies
