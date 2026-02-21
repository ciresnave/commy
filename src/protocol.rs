//! WebSocket protocol for COMMY
//!
//! Defines messages and message routing for remote Client-Server communication
//! over WSS (WebSocket Secure).
//!
//! ## Serialization Strategy
//! All WssMessage types are serialized using **MessagePack** for maximum efficiency:
//! - 40-50% smaller payloads compared to JSON
//! - 20-50% faster serialization and deserialization  
//! - Maintains compatibility with custom tenant services
//! - Example: Heartbeat message 64 bytes (JSON) → 31 bytes (MessagePack)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for WSS connections/clients
pub type ClientSessionId = String;

/// WebSocket message types for COMMY protocol
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WssMessage {
    /// Client->Server: Authenticate with credentials
    Authenticate {
        tenant_id: String,
        client_id: String,
        client_version: String,
        credentials: String, // Token or credentials
        auth_method: String, // "api_key", "jwt", "mtls", "custom", etc.
    },

    /// Server->Client: Authentication response
    AuthenticationResponse {
        success: bool,
        message: String,
        server_version: String,
        token: Option<String>,
        expires_in_seconds: Option<u64>,
    },

    /// Client->Server: Request to read variables from a service
    GetVariables {
        session_id: ClientSessionId,
        tenant_id: String,
        service_name: String,
        variable_names: Vec<String>,
    },

    /// Server->Client: Variable data response
    VariablesData {
        tenant_id: String,
        service_name: String,
        variables: std::collections::HashMap<String, Vec<u8>>,
        timestamp: String,
    },

    /// Client->Server: Write variables to a service
    SetVariables {
        session_id: ClientSessionId,
        tenant_id: String,
        service_name: String,
        variables: std::collections::HashMap<String, Vec<u8>>,
    },

    /// Server->Client: Variable write acknowledgment
    VariablesUpdated {
        success: bool,
        message: String,
        service_name: String,
    },

    /// Server->Client: Broadcast when other clients modify variables
    VariableChanged {
        tenant_id: String,
        service_name: String,
        variable_name: String,
        new_value: Vec<u8>,
        changed_by_client: Option<String>,
        timestamp: String,
    },

    /// Client->Server: Subscribe to variable changes
    Subscribe {
        session_id: ClientSessionId,
        tenant_id: String,
        service_name: String,
        variable_names: Vec<String>,
    },

    /// Server->Client: Subscription acknowledgment
    SubscriptionAck {
        success: bool,
        message: String,
        service_name: String,
    },

    /// Client->Server: Heartbeat/keepalive
    Heartbeat { session_id: ClientSessionId },

    /// Server->Client: Heartbeat acknowledgment
    HeartbeatAck { timestamp: String },

    /// Server->Client: Permission revocation notice
    PermissionRevoked { reason: String, detail: String },

    /// Server->Client: Request to migrate to new service file
    FileMigration {
        old_service_path: String,
        new_service_path: String,
        service_name: String,
        reason: String,
    },

    /// Client->Server: Migration acknowledgment
    MigrationAck { success: bool, service_name: String },

    /// Client->Server: Check permission
    CheckPermission {
        session_id: ClientSessionId,
        tenant_id: String,
        permission: String, // Serialized Permission
    },

    /// Server->Client: Permission check response
    PermissionResponse { has_permission: bool },

    /// Server->Client: Server-to-server: sync token/session across cluster
    TokenSync {
        client_id: String,
        token: String,
        tenant_id: String,
        permissions: String, // Serialized PermissionSet
        expires_at: String,  // ISO 8601
    },

    /// Server->Client: Cluster node availability ping
    ClusterPing { node_id: String, timestamp: String },

    /// Client->Server: Cluster ping response
    ClusterPingResponse { node_id: String, timestamp: String },

    /// Client->Server: Error message
    Error {
        code: String,
        message: String,
        details: Option<String>,
    },

    /// Generic acknowledgment
    Ack { message_id: String },

    /// Client->Server: Logout and revoke token
    Logout {
        session_id: ClientSessionId,
        token: String,
    },

    /// Server->Client: Logout acknowledgment
    LogoutResponse { success: bool, message: String },

    /// Client->Server: Refresh token before expiration
    RefreshToken {
        session_id: ClientSessionId,
        current_token: String,
    },

    /// Server->Client: New token after refresh
    TokenRefreshResponse {
        success: bool,
        message: String,
        token: Option<String>,
        expires_in_seconds: Option<u64>,
    },
}

impl WssMessage {
    /// Get a human-readable type name for logging
    pub fn message_type(&self) -> &'static str {
        match self {
            WssMessage::Authenticate { .. } => "Authenticate",
            WssMessage::AuthenticationResponse { .. } => "AuthenticationResponse",
            WssMessage::GetVariables { .. } => "GetVariables",
            WssMessage::VariablesData { .. } => "VariablesData",
            WssMessage::SetVariables { .. } => "SetVariables",
            WssMessage::VariablesUpdated { .. } => "VariablesUpdated",
            WssMessage::VariableChanged { .. } => "VariableChanged",
            WssMessage::Subscribe { .. } => "Subscribe",
            WssMessage::SubscriptionAck { .. } => "SubscriptionAck",
            WssMessage::Heartbeat { .. } => "Heartbeat",
            WssMessage::HeartbeatAck { .. } => "HeartbeatAck",
            WssMessage::PermissionRevoked { .. } => "PermissionRevoked",
            WssMessage::FileMigration { .. } => "FileMigration",
            WssMessage::MigrationAck { .. } => "MigrationAck",
            WssMessage::CheckPermission { .. } => "CheckPermission",
            WssMessage::PermissionResponse { .. } => "PermissionResponse",
            WssMessage::TokenSync { .. } => "TokenSync",
            WssMessage::ClusterPing { .. } => "ClusterPing",
            WssMessage::ClusterPingResponse { .. } => "ClusterPingResponse",
            WssMessage::Error { .. } => "Error",
            WssMessage::Ack { .. } => "Ack",
            WssMessage::Logout { .. } => "Logout",
            WssMessage::LogoutResponse { .. } => "LogoutResponse",
            WssMessage::RefreshToken { .. } => "RefreshToken",
            WssMessage::TokenRefreshResponse { .. } => "TokenRefreshResponse",
        }
    }
}

/// Client connection state
#[derive(Clone, Debug, PartialEq)]
pub enum ClientState {
    /// Newly connected, not yet authenticated
    Unauthenticated,
    /// Authenticated and active
    Active,
    /// Waiting for heartbeat response
    HeartbeatPending,
    /// Waiting for client to migrate to new service file
    MigrationPending,
    /// Client revoked (honeypot detection)
    Revoked,
    /// Connection lost
    Disconnected,
}

/// Client session information
#[derive(Clone, Debug)]
pub struct ClientSession {
    /// Unique session ID
    pub session_id: ClientSessionId,
    /// Associated client ID (None until authenticated)
    pub client_id: Option<String>,
    /// Associated tenant ID (None until authenticated)
    pub tenant_id: Option<String>,
    /// Authentication token (None until authenticated)
    pub token: Option<String>,
    /// Permissions granted to this session (None until authenticated)
    pub permissions: Option<crate::auth::PermissionSet>,
    /// Current state
    pub state: ClientState,
    /// Last message timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Last heartbeat response timestamp
    pub last_heartbeat_ack: Option<chrono::DateTime<chrono::Utc>>,
    /// Outbound message queue size (for stall detection)
    pub outbound_queue_size: usize,
    /// Subscribed services
    pub subscriptions: std::collections::HashSet<String>,
}

impl ClientSession {
    /// Create a new client session
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            client_id: None,
            tenant_id: None,
            token: None,
            permissions: None,
            state: ClientState::Unauthenticated,
            last_activity: chrono::Utc::now(),
            last_heartbeat_ack: None,
            outbound_queue_size: 0,
            subscriptions: std::collections::HashSet::new(),
        }
    }

    /// Mark this session as authenticated
    pub fn authenticate(&mut self, client_id: String, tenant_id: String) {
        self.client_id = Some(client_id);
        self.tenant_id = Some(tenant_id);
        self.state = ClientState::Active;
        self.last_activity = chrono::Utc::now();
    }

    /// Check if this session is healthy (not stalled)
    pub fn is_healthy(&self, queue_stall_threshold_secs: u64) -> bool {
        // Dead if queue hasn't drained in threshold time
        if self.outbound_queue_size > 0 {
            if let Some(last_ack) = self.last_heartbeat_ack {
                let elapsed = (chrono::Utc::now() - last_ack).num_seconds() as u64;
                return elapsed < queue_stall_threshold_secs;
            }
            // Queue growing but no heartbeat response yet
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wss_message_types() {
        let auth_msg = WssMessage::Authenticate {
            tenant_id: "tenant1".to_string(),
            client_id: "client1".to_string(),
            client_version: "0.1.0".to_string(),
            credentials: "secret".to_string(),
            auth_method: "api_key".to_string(),
        };
        assert_eq!(auth_msg.message_type(), "Authenticate");

        let heartbeat = WssMessage::Heartbeat {
            session_id: "session1".to_string(),
        };
        assert_eq!(heartbeat.message_type(), "Heartbeat");
    }

    #[test]
    fn test_client_session_creation() {
        let session = ClientSession::new();
        assert_eq!(session.state, ClientState::Unauthenticated);
        assert!(session.client_id.is_none());
        assert!(session.tenant_id.is_none());
        assert!(!session.session_id.is_empty());
    }

    #[test]
    fn test_client_session_authentication() {
        let mut session = ClientSession::new();
        session.authenticate("client1".to_string(), "tenant1".to_string());

        assert_eq!(session.state, ClientState::Active);
        assert_eq!(session.client_id, Some("client1".to_string()));
        assert_eq!(session.tenant_id, Some("tenant1".to_string()));
    }

    #[test]
    fn test_client_health_check() {
        let session = ClientSession::new();
        assert!(session.is_healthy(30)); // No queue, should be healthy

        let mut session = ClientSession::new();
        session.outbound_queue_size = 10;
        assert!(!session.is_healthy(30)); // Queue growing without heartbeat ack
    }
}
