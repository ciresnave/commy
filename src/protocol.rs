// SPDX-License-Identifier: MIT
//! Client session state for COMMY
//!
//! The wire protocol itself is JSON text frames plus the binary range frames of `commy-wire`; the
//! older MessagePack `WssMessage` protocol was removed in 0.21.0 (R12).

use uuid::Uuid;

/// Unique identifier for WSS connections/clients
pub type ClientSessionId = String;

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
    /// Local-mapping nonces handed out and not yet confirmed, by `file_id`.
    pub pending_nonces: std::collections::HashMap<u32, crate::server::local_mapping::PendingNonce>,
    /// Files this session proved it can map locally (`file_id`s).
    pub local_files: std::collections::HashSet<u32>,
    /// Binary frames the connection loop sends before the next text reply (resync snapshots).
    pub outbound_frames: Vec<Vec<u8>>,
    /// Queue other connections use to forward ranges to this one (set by the connection loop).
    pub outbound: Option<crate::server::forwarding::OutTx>,
    /// Ranges a remote sender has written into the file and not yet ended with `FlushEnd`.
    pub pending_flush: std::collections::HashMap<u32, Vec<commy_wire::ByteRange>>,
    /// Whether this session's authentication permissions were imported into the grant store.
    pub grants_imported: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
            pending_nonces: std::collections::HashMap::new(),
            local_files: std::collections::HashSet::new(),
            outbound_frames: Vec::new(),
            outbound: None,
            pending_flush: std::collections::HashMap::new(),
            grants_imported: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Delete the side files of every unconfirmed nonce (on disconnect).
    pub fn drop_pending_nonces(&mut self) {
        for (_, pending) in self.pending_nonces.drain() {
            let _ = std::fs::remove_file(&pending.path);
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


    #[test]
    fn test_is_healthy_queue_with_recent_heartbeat_ack_true() {
        let mut session = ClientSession::new();
        session.outbound_queue_size = 10;
        // Recent ack (now) — well within the 30-second threshold
        session.last_heartbeat_ack = Some(chrono::Utc::now());
        assert!(
            session.is_healthy(30),
            "is_healthy must return true when queue > 0 and heartbeat ack is recent"
        );
    }

    #[test]
    fn test_is_healthy_queue_with_stale_heartbeat_ack_false() {
        let mut session = ClientSession::new();
        session.outbound_queue_size = 5;
        // Stale ack: 60 seconds ago, threshold 30 s → stale
        session.last_heartbeat_ack = Some(chrono::Utc::now() - chrono::Duration::seconds(60));
        assert!(
            !session.is_healthy(30),
            "is_healthy must return false when queue > 0 and heartbeat ack is stale"
        );
    }

    #[test]
    fn test_authenticate_updates_last_activity() {
        let mut session = ClientSession::new();
        // Rewind last_activity to a known past moment
        let before = chrono::Utc::now() - chrono::Duration::seconds(5);
        session.last_activity = before;

        session.authenticate("cli_42".to_string(), "tenant_x".to_string());

        assert!(
            session.last_activity > before,
            "authenticate() must update last_activity: got {:?}, expected > {:?}",
            session.last_activity,
            before
        );
        assert_eq!(session.client_id, Some("cli_42".to_string()));
        assert_eq!(session.tenant_id, Some("tenant_x".to_string()));
        assert_eq!(session.state, ClientState::Active);
    }
}
