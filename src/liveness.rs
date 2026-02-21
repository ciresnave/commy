//! Client liveness detection system for COMMY
//! 
//! Monitors client connections for:
//! - Connection drops (WSS disconnect)
//! - Queue stalls (outbound message queue not draining)
//! - Heartbeat timeouts

use crate::protocol::{ClientSession, ClientState, ClientSessionId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use chrono::{DateTime, Utc, Duration};

/// Errors that can occur in liveness detection
#[derive(Error, Debug)]
pub enum LivenessError {
    #[error("Client not found: {0}")]
    ClientNotFound(String),

    #[error("Client is dead: {0}")]
    ClientDead(String),

    #[error("Queue stalled for client: {0}")]
    QueueStalled(String),
}

pub type LivenessResult<T> = Result<T, LivenessError>;

/// Liveness detection configuration
#[derive(Clone, Debug)]
pub struct LivenessConfig {
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
    /// Heartbeat timeout in seconds
    pub heartbeat_timeout_secs: u64,
    /// Queue stall threshold in seconds (30s recommended)
    pub queue_stall_threshold_secs: u64,
    /// Check interval in seconds (background task)
    pub check_interval_secs: u64,
}

impl Default for LivenessConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 30,
            heartbeat_timeout_secs: 60,
            queue_stall_threshold_secs: 30,
            check_interval_secs: 5,
        }
    }
}

/// Client liveness monitor
/// 
/// Tracks client connection health and detects:
/// 1. Connection drops (WSS disconnect)
/// 2. Queue stalls (outbound queue not draining)
/// 3. Heartbeat timeouts (no response in time)
pub struct LivenessMonitor {
    /// Active client sessions
    sessions: Arc<RwLock<HashMap<ClientSessionId, ClientSession>>>,
    /// Dead clients (for honeypot detection)
    dead_clients: Arc<RwLock<HashMap<ClientSessionId, DeadClientInfo>>>,
    /// Configuration
    config: LivenessConfig,
}

/// Information about a dead/revoked client
#[derive(Clone, Debug)]
pub struct DeadClientInfo {
    /// Session ID
    pub session_id: ClientSessionId,
    /// When the client was marked dead
    pub death_time: DateTime<Utc>,
    /// Reason for death
    pub reason: String,
}

impl LivenessMonitor {
    /// Create a new liveness monitor
    pub fn new(config: LivenessConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            dead_clients: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a new client session
    pub async fn register_client(&self, session: ClientSession) -> LivenessResult<String> {
        let session_id = session.session_id.clone();
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Update client activity (heartbeat, message, etc.)
    pub async fn update_activity(&self, session_id: &str) -> LivenessResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_activity = Utc::now();
            Ok(())
        } else {
            Err(LivenessError::ClientNotFound(session_id.to_string()))
        }
    }

    /// Record heartbeat acknowledgment
    pub async fn record_heartbeat_ack(&self, session_id: &str) -> LivenessResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_heartbeat_ack = Some(Utc::now());
            session.outbound_queue_size = 0; // Queue drained
            Ok(())
        } else {
            Err(LivenessError::ClientNotFound(session_id.to_string()))
        }
    }

    /// Update outbound queue size for stall detection
    pub async fn update_queue_size(&self, session_id: &str, size: usize) -> LivenessResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.outbound_queue_size = size;
            Ok(())
        } else {
            Err(LivenessError::ClientNotFound(session_id.to_string()))
        }
    }

    /// Check if a client is alive
    pub async fn is_client_alive(&self, session_id: &str) -> LivenessResult<bool> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            Ok(session.state != ClientState::Disconnected && 
               session.is_healthy(self.config.queue_stall_threshold_secs))
        } else {
            Err(LivenessError::ClientNotFound(session_id.to_string()))
        }
    }

    /// Mark a client as dead and move to dead clients map
    pub async fn mark_client_dead(&self, session_id: &str, reason: String) -> LivenessResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(mut session) = sessions.remove(session_id) {
            session.state = ClientState::Disconnected;

            let dead_info = DeadClientInfo {
                session_id: session_id.to_string(),
                death_time: Utc::now(),
                reason,
            };

            let mut dead = self.dead_clients.write().await;
            dead.insert(session_id.to_string(), dead_info);
            Ok(())
        } else {
            Err(LivenessError::ClientNotFound(session_id.to_string()))
        }
    }

    /// Detect dead clients (connection drop or queue stall)
    /// 
    /// Returns list of dead session IDs
    pub async fn detect_dead_clients(&self) -> Vec<(String, String)> {
        let mut dead_list = Vec::new();
        let mut sessions = self.sessions.write().await;

        let now = Utc::now();
        let heartbeat_timeout = Duration::seconds(self.config.heartbeat_timeout_secs as i64);

        // Check each client
        let dead_sessions: Vec<(String, String)> = sessions
            .iter()
            .filter_map(|(id, session)| {
                // Check 1: Queue stall - queue growing without heartbeat ack in threshold time
                if session.outbound_queue_size > 0 {
                    if let Some(last_ack) = session.last_heartbeat_ack {
                        let elapsed = now - last_ack;
                        if elapsed > Duration::seconds(self.config.queue_stall_threshold_secs as i64) {
                            return Some((
                                id.clone(),
                                format!("Queue stalled for {} seconds", elapsed.num_seconds()),
                            ));
                        }
                    }
                }

                // Check 2: Heartbeat timeout - no ack within timeout
                if session.state == ClientState::HeartbeatPending {
                    if let Some(last_ack) = session.last_heartbeat_ack {
                        if (now - last_ack) > heartbeat_timeout {
                            return Some((
                                id.clone(),
                                "Heartbeat timeout".to_string(),
                            ));
                        }
                    }
                }

                // Check 3: Inactivity timeout - no activity in 2x heartbeat interval
                let inactivity_timeout = Duration::seconds(
                    (self.config.heartbeat_interval_secs * 2) as i64
                );
                if (now - session.last_activity) > inactivity_timeout {
                    return Some((
                        id.clone(),
                        "Inactivity timeout".to_string(),
                    ));
                }

                None
            })
            .collect();

        // Remove dead clients
        for (session_id, reason) in dead_sessions {
            sessions.remove(&session_id);
            let dead_info = DeadClientInfo {
                session_id: session_id.clone(),
                death_time: Utc::now(),
                reason: reason.clone(),
            };
            dead_list.push((session_id, reason));

            let mut dead = self.dead_clients.write().await;
            dead.insert(dead_info.session_id.clone(), dead_info);
        }

        dead_list
    }

    /// Get all active client sessions
    pub async fn get_active_clients(&self) -> Vec<ClientSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// Get client session by ID
    pub async fn get_client(&self, session_id: &str) -> LivenessResult<ClientSession> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| LivenessError::ClientNotFound(session_id.to_string()))
    }

    /// Remove a client (cleanup after revocation)
    pub async fn remove_client(&self, session_id: &str) -> LivenessResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }

    /// Clean up honeypot dead clients older than threshold
    pub async fn cleanup_dead_clients(&self, max_age_secs: u64) {
        let mut dead = self.dead_clients.write().await;
        let now = Utc::now();
        let max_age = Duration::seconds(max_age_secs as i64);

        dead.retain(|_, info| (now - info.death_time) < max_age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_get_client() {
        let monitor = LivenessMonitor::new(LivenessConfig::default());
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        monitor.register_client(session).await.unwrap();
        let retrieved = monitor.get_client(&session_id).await.unwrap();

        assert_eq!(retrieved.session_id, session_id);
        assert_eq!(retrieved.state, ClientState::Unauthenticated);
    }

    #[tokio::test]
    async fn test_update_activity() {
        let monitor = LivenessMonitor::new(LivenessConfig::default());
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        monitor.register_client(session).await.unwrap();
        let old_time = monitor.get_client(&session_id).await.unwrap().last_activity;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        monitor.update_activity(&session_id).await.unwrap();

        let new_session = monitor.get_client(&session_id).await.unwrap();
        assert!(new_session.last_activity > old_time);
    }

    #[tokio::test]
    async fn test_detect_dead_clients() {
        let mut config = LivenessConfig::default();
        config.heartbeat_timeout_secs = 1;
        config.queue_stall_threshold_secs = 1;

        let monitor = LivenessMonitor::new(config);
        let mut session = ClientSession::new();
        let session_id = session.session_id.clone();

        // Simulate old last activity
        session.last_activity = Utc::now() - Duration::seconds(100);
        monitor.register_client(session).await.unwrap();

        let dead = monitor.detect_dead_clients().await;
        assert!(!dead.is_empty());
        assert_eq!(dead[0].0, session_id);
    }

    #[tokio::test]
    async fn test_heartbeat_ack() {
        let monitor = LivenessMonitor::new(LivenessConfig::default());
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        monitor.register_client(session).await.unwrap();
        monitor.record_heartbeat_ack(&session_id).await.unwrap();

        let updated = monitor.get_client(&session_id).await.unwrap();
        assert!(updated.last_heartbeat_ack.is_some());
        assert_eq!(updated.outbound_queue_size, 0);
    }

    #[tokio::test]
    async fn test_queue_stall_detection() {
        let monitor = LivenessMonitor::new(LivenessConfig::default());
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        monitor.register_client(session).await.unwrap();
        monitor.update_queue_size(&session_id, 10).await.unwrap();

        // Without heartbeat ack, should be considered unhealthy
        let client = monitor.get_client(&session_id).await.unwrap();
        assert!(!client.is_healthy(30));
    }
}
