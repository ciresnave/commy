//! Inter-server communication protocol
//!
//! Defines message types for server-to-server communication in the cluster.
//! All messages are JSON-serializable for transmission over TCP/WebSocket.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Inter-server message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Heartbeat ping from peer
    HeartbeatPing {
        /// Sending server's ID
        server_id: String,
        /// Sending server's current timestamp (ms since epoch)
        timestamp: u64,
        /// Server's version/sequence number
        sequence: u64,
    },

    /// Heartbeat pong response
    HeartbeatPong {
        /// Responding server's ID
        server_id: String,
        /// Responding server's timestamp
        timestamp: u64,
        /// Health status of responding server
        status: ServerStatus,
    },

    /// Request to sync service state
    SyncServiceRequest {
        /// Request ID for tracking
        request_id: String,
        /// Tenant name
        tenant_name: String,
        /// Service name
        service_name: String,
        /// Version of local state (for incremental sync)
        from_version: Option<u64>,
        /// Checksum of local state (for validation)
        from_checksum: Option<String>,
    },

    /// Response with service state
    SyncServiceResponse {
        /// Matches the request_id
        request_id: String,
        /// Tenant name
        tenant_name: String,
        /// Service name
        service_name: String,
        /// Current version of this service
        version: u64,
        /// Overall checksum of service state
        checksum: String,
        /// Serialized service data (incremental or full)
        data: Vec<u8>,
        /// Whether this is incremental (true) or full state (false)
        is_incremental: bool,
        /// If incremental, the range of versions included
        version_range: Option<(u64, u64)>,
    },

    /// Notify peer about client migration
    ClientMigration {
        /// Client session ID being migrated
        client_id: String,
        /// Tenant name
        tenant_name: String,
        /// Reason for migration
        reason: MigrationReason,
        /// Client's current permissions
        permissions: HashMap<String, Vec<String>>,
        /// Client's subscriptions
        subscriptions: Vec<(String, String)>, // (tenant, service)
    },

    /// Acknowledge client migration completion
    ClientMigrationAck {
        /// Matches client_id
        client_id: String,
        /// Whether migration was accepted
        accepted: bool,
        /// Message if rejected
        reason: Option<String>,
    },

    /// Request file transfer for service state
    FileTransferRequest {
        /// Request ID
        request_id: String,
        /// Tenant name
        tenant_name: String,
        /// Service name
        service_name: String,
        /// File name/path on source server
        source_filename: String,
        /// Starting byte offset (for resume)
        offset: u64,
        /// Maximum bytes to send in response
        chunk_size: u64,
    },

    /// Response with file chunk
    FileTransferChunk {
        /// Matches request_id
        request_id: String,
        /// Byte offset of this chunk
        offset: u64,
        /// Chunk data
        data: Vec<u8>,
        /// Total file size
        total_size: u64,
        /// Whether more chunks follow
        has_more: bool,
        /// SHA256 checksum of this chunk
        checksum: String,
    },

    /// Exchange server metadata
    MetadataExchange {
        /// This server's ID
        server_id: String,
        /// List of tenants this server knows about
        tenants: Vec<TenantMetadata>,
        /// Total services hosted
        total_services: usize,
        /// Cluster size this server knows about
        cluster_size: usize,
    },

    /// Response to metadata exchange
    MetadataExchangeAck {
        /// Responding server's ID
        server_id: String,
        /// Acknowledgment that metadata was received
        acknowledged: bool,
        /// Any conflicts or inconsistencies found
        conflicts: Vec<String>,
    },

    /// Error response to any request
    Error {
        /// Original request ID if applicable
        request_id: Option<String>,
        /// Error code (e.g., "PEER_NOT_FOUND", "SERVICE_NOT_FOUND")
        error_code: String,
        /// Human-readable error message
        message: String,
    },
}

/// Server health status for heartbeat responses
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerStatus {
    /// Server is healthy and operational
    Healthy,
    /// Server is running but degraded (some services down)
    Degraded,
    /// Server is shutting down
    ShuttingDown,
}

impl ServerStatus {
    /// Check if server can accept new operations
    pub fn is_operational(self) -> bool {
        matches!(self, ServerStatus::Healthy | ServerStatus::Degraded)
    }
}

/// Reason for client migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationReason {
    /// This server is shutting down
    ServerShutdown,
    /// This server lost quorum
    QuorumLoss,
    /// Load balancing/rebalancing
    LoadBalancing,
    /// Client disconnected and reconnected to different server
    ClientReconnected,
    /// Explicit migration by operator
    Manual,
}

/// Metadata about a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantMetadata {
    /// Tenant name
    pub name: String,
    /// Number of services in this tenant
    pub service_count: usize,
    /// Last update timestamp (ms since epoch)
    pub last_updated: u64,
    /// Whether this server has quorum for this tenant
    pub has_quorum: bool,
}

/// Request/response wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMessageEnvelope {
    /// Message ID for correlation
    pub message_id: String,
    /// Sending server ID
    pub from_server: String,
    /// Receiving server ID
    pub to_server: String,
    /// Unix timestamp (ms since epoch)
    pub timestamp: u64,
    /// Actual message payload
    pub message: ServerMessage,
}

impl PeerMessageEnvelope {
    /// Create a new envelope
    pub fn new(
        from_server: String,
        to_server: String,
        message: ServerMessage,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            message_id: uuid::Uuid::new_v4().to_string(),
            from_server,
            to_server,
            timestamp,
            message,
        }
    }

    /// Get message ID for tracking
    pub fn message_id(&self) -> &str {
        &self.message_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_ping_serialization() {
        let msg = ServerMessage::HeartbeatPing {
            server_id: "server_1".to_string(),
            timestamp: 1000000,
            sequence: 42,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            ServerMessage::HeartbeatPing {
                server_id,
                timestamp,
                sequence,
            } => {
                assert_eq!(server_id, "server_1");
                assert_eq!(timestamp, 1000000);
                assert_eq!(sequence, 42);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_server_status_operational() {
        assert!(ServerStatus::Healthy.is_operational());
        assert!(ServerStatus::Degraded.is_operational());
        assert!(!ServerStatus::ShuttingDown.is_operational());
    }

    #[test]
    fn test_envelope_creation() {
        let msg = ServerMessage::HeartbeatPing {
            server_id: "server_1".to_string(),
            timestamp: 1000000,
            sequence: 1,
        };

        let envelope = PeerMessageEnvelope::new(
            "server_1".to_string(),
            "server_2".to_string(),
            msg,
        );

        assert_eq!(envelope.from_server, "server_1");
        assert_eq!(envelope.to_server, "server_2");
        assert!(!envelope.message_id.is_empty());
    }

    #[test]
    fn test_envelope_serialization() {
        let msg = ServerMessage::HeartbeatPing {
            server_id: "server_1".to_string(),
            timestamp: 1000000,
            sequence: 1,
        };

        let envelope = PeerMessageEnvelope::new(
            "server_1".to_string(),
            "server_2".to_string(),
            msg,
        );

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: PeerMessageEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.from_server, "server_1");
        assert_eq!(deserialized.to_server, "server_2");
    }

    #[test]
    fn test_error_message() {
        let msg = ServerMessage::Error {
            request_id: Some("req_123".to_string()),
            error_code: "SERVICE_NOT_FOUND".to_string(),
            message: "Service does not exist".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();

        match deserialized {
            ServerMessage::Error {
                request_id,
                error_code,
                message: _,
            } => {
                assert_eq!(request_id, Some("req_123".to_string()));
                assert_eq!(error_code, "SERVICE_NOT_FOUND");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_migration_reason_serialization() {
        let reasons = vec![
            MigrationReason::ServerShutdown,
            MigrationReason::QuorumLoss,
            MigrationReason::LoadBalancing,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            let _: MigrationReason = serde_json::from_str(&json).unwrap();
        }
    }
}
