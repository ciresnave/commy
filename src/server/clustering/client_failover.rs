/// Client Failover Detection - Detects connection loss and triggers reconnection
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a server that a client is connected to
#[derive(Clone, Debug)]
pub struct ServerConnection {
    pub server_id: String,
    pub address: String,
    pub port: u16,
    pub is_connected: bool,
    pub last_heartbeat: u64,
    pub consecutive_failures: u32,
}

/// Failover detection state for a client session
#[derive(Clone, Debug, PartialEq)]
pub enum ClientFailoverState {
    Connected,
    Reconnecting,
    Failed,
    Recovered,
}

/// Represents a candidate server for reconnection
#[derive(Clone, Debug)]
pub struct ServerCandidate {
    pub server_id: String,
    pub address: String,
    pub port: u16,
    pub latency_ms: u64,
    pub priority: u32,
}

/// Configuration for client-side failover behavior
#[derive(Clone, Debug)]
pub struct ClientFailoverConfig {
    /// Maximum consecutive connection failures before triggering failover
    pub max_consecutive_failures: u32,
    /// Time to wait between reconnection attempts (ms)
    pub reconnection_delay_ms: u64,
    /// Maximum time to attempt reconnection (ms)
    pub reconnection_timeout_ms: u64,
    /// Enable automatic reconnection to healthy peer
    pub auto_reconnect: bool,
    /// List of backup servers to use for failover
    pub backup_servers: Vec<ServerConnection>,
}

impl Default for ClientFailoverConfig {
    fn default() -> Self {
        Self {
            max_consecutive_failures: 3,
            reconnection_delay_ms: 1000,
            reconnection_timeout_ms: 30000,
            auto_reconnect: true,
            backup_servers: Vec::new(),
        }
    }
}

/// Manages client-side failure detection and reconnection
pub struct ClientFailoverDetector {
    current_server: Arc<RwLock<Option<ServerConnection>>>,
    state: Arc<RwLock<ClientFailoverState>>,
    config: ClientFailoverConfig,
    available_servers: Arc<RwLock<Vec<ServerCandidate>>>,
    reconnection_attempts: Arc<RwLock<u32>>,
}

impl ClientFailoverDetector {
    /// Create new client failover detector
    pub fn new(config: ClientFailoverConfig) -> Self {
        Self {
            current_server: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ClientFailoverState::Failed)),
            config,
            available_servers: Arc::new(RwLock::new(Vec::new())),
            reconnection_attempts: Arc::new(RwLock::new(0)),
        }
    }

    /// Set the current server connection
    pub async fn set_current_server(&self, server: ServerConnection) {
        *self.current_server.write().await = Some(server);
        *self.state.write().await = ClientFailoverState::Connected;
        *self.reconnection_attempts.write().await = 0;
    }

    /// Record a connection failure
    pub async fn record_failure(&self) -> bool {
        let mut current = self.current_server.write().await;

        if let Some(ref mut server) = *current {
            server.consecutive_failures += 1;

            // Check if we should trigger failover
            if server.consecutive_failures >= self.config.max_consecutive_failures {
                *self.state.write().await = ClientFailoverState::Reconnecting;
                return true; // Failover triggered
            }
        }

        false
    }

    /// Record successful connection
    pub async fn record_success(&self) {
        if let Some(ref mut server) = *self.current_server.write().await {
            server.consecutive_failures = 0;
            server.is_connected = true;
        }
        *self.state.write().await = ClientFailoverState::Connected;
        *self.reconnection_attempts.write().await = 0;
    }

    /// Get current server
    pub async fn get_current_server(&self) -> Option<ServerConnection> {
        self.current_server.read().await.clone()
    }

    /// Get current failover state
    pub async fn get_state(&self) -> ClientFailoverState {
        self.state.read().await.clone()
    }

    /// Update available servers (from server discovery)
    pub async fn update_available_servers(&self, servers: Vec<ServerCandidate>) {
        *self.available_servers.write().await = servers;
    }

    /// Get list of available servers for failover
    pub async fn get_available_servers(&self) -> Vec<ServerCandidate> {
        self.available_servers.read().await.clone()
    }

    /// Find best server for reconnection
    pub async fn find_reconnection_target(&self) -> Option<ServerCandidate> {
        let available = self.available_servers.read().await;

        if available.is_empty() {
            return None;
        }

        // Sort by priority (higher first), then by latency (lower first)
        let mut candidates = available.clone();
        candidates.sort_by(|a, b| {
            match b.priority.cmp(&a.priority) {
                std::cmp::Ordering::Equal => a.latency_ms.cmp(&b.latency_ms),
                other => other,
            }
        });

        candidates.first().cloned()
    }

    /// Attempt reconnection to a new server
    pub async fn attempt_reconnection(&self, new_server: ServerConnection) -> bool {
        let mut attempts = self.reconnection_attempts.write().await;
        *attempts += 1;

        if *attempts > 10 {
            // Too many attempts
            *self.state.write().await = ClientFailoverState::Failed;
            return false;
        }

        // In real implementation, would establish connection here
        // For now, just update state
        *self.current_server.write().await = Some(new_server);
        *self.state.write().await = ClientFailoverState::Reconnecting;

        true
    }

    /// Mark reconnection as successful
    pub async fn reconnection_successful(&self) {
        *self.state.write().await = ClientFailoverState::Recovered;
        *self.reconnection_attempts.write().await = 0;
    }

    /// Mark reconnection as failed
    pub async fn reconnection_failed(&self) {
        let attempts = self.reconnection_attempts.read().await;
        if *attempts >= 10 {
            *self.state.write().await = ClientFailoverState::Failed;
        }
    }

    /// Get reconnection attempt count
    pub async fn get_reconnection_attempts(&self) -> u32 {
        *self.reconnection_attempts.read().await
    }

    /// Reset failover state (e.g., after successful recovery)
    pub async fn reset(&self) {
        *self.state.write().await = ClientFailoverState::Failed;
        *self.reconnection_attempts.write().await = 0;
        
        if let Some(ref mut server) = *self.current_server.write().await {
            server.consecutive_failures = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_failover_detector_creation() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        assert_eq!(detector.get_state().await, ClientFailoverState::Failed);
    }

    #[tokio::test]
    async fn test_set_current_server() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        let server = ServerConnection {
            server_id: "server1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5000,
            is_connected: true,
            last_heartbeat: 1000,
            consecutive_failures: 0,
        };

        detector.set_current_server(server.clone()).await;

        assert_eq!(detector.get_state().await, ClientFailoverState::Connected);
        assert_eq!(
            detector.get_current_server().await.unwrap().server_id,
            "server1"
        );
    }

    #[tokio::test]
    async fn test_record_failure_no_trigger() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        let server = ServerConnection {
            server_id: "server1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5000,
            is_connected: true,
            last_heartbeat: 1000,
            consecutive_failures: 0,
        };

        detector.set_current_server(server).await;
        let triggered = detector.record_failure().await;

        assert!(!triggered);
        let current = detector.get_current_server().await.unwrap();
        assert_eq!(current.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_record_failure_trigger_failover() {
        let mut config = ClientFailoverConfig::default();
        config.max_consecutive_failures = 2;

        let detector = ClientFailoverDetector::new(config);
        let server = ServerConnection {
            server_id: "server1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5000,
            is_connected: true,
            last_heartbeat: 1000,
            consecutive_failures: 0,
        };

        detector.set_current_server(server).await;
        detector.record_failure().await;
        let triggered = detector.record_failure().await;

        assert!(triggered);
        assert_eq!(detector.get_state().await, ClientFailoverState::Reconnecting);
    }

    #[tokio::test]
    async fn test_record_success() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        let server = ServerConnection {
            server_id: "server1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5000,
            is_connected: true,
            last_heartbeat: 1000,
            consecutive_failures: 2,
        };

        detector.set_current_server(server).await;
        detector.record_success().await;

        let current = detector.get_current_server().await.unwrap();
        assert_eq!(current.consecutive_failures, 0);
        assert_eq!(detector.get_state().await, ClientFailoverState::Connected);
    }

    #[tokio::test]
    async fn test_find_reconnection_target() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());

        let candidates = vec![
            ServerCandidate {
                server_id: "server1".to_string(),
                address: "127.0.0.1".to_string(),
                port: 5001,
                latency_ms: 10,
                priority: 1,
            },
            ServerCandidate {
                server_id: "server2".to_string(),
                address: "127.0.0.1".to_string(),
                port: 5002,
                latency_ms: 5,
                priority: 2,
            },
        ];

        detector.update_available_servers(candidates).await;
        let target = detector.find_reconnection_target().await;

        assert!(target.is_some());
        assert_eq!(target.unwrap().server_id, "server2"); // Higher priority
    }

    #[tokio::test]
    async fn test_find_reconnection_target_empty() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        detector.update_available_servers(vec![]).await;

        let target = detector.find_reconnection_target().await;
        assert!(target.is_none());
    }

    #[tokio::test]
    async fn test_attempt_reconnection() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        let new_server = ServerConnection {
            server_id: "server2".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5001,
            is_connected: true,
            last_heartbeat: 2000,
            consecutive_failures: 0,
        };

        let result = detector.attempt_reconnection(new_server.clone()).await;

        assert!(result);
        assert_eq!(detector.get_current_server().await.unwrap().server_id, "server2");
        assert_eq!(detector.get_state().await, ClientFailoverState::Reconnecting);
    }

    #[tokio::test]
    async fn test_reconnection_successful() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        let new_server = ServerConnection {
            server_id: "server2".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5001,
            is_connected: true,
            last_heartbeat: 2000,
            consecutive_failures: 0,
        };

        detector.attempt_reconnection(new_server).await;
        detector.reconnection_successful().await;

        assert_eq!(detector.get_state().await, ClientFailoverState::Recovered);
        assert_eq!(detector.get_reconnection_attempts().await, 0);
    }

    #[tokio::test]
    async fn test_reset() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());
        let server = ServerConnection {
            server_id: "server1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 5000,
            is_connected: true,
            last_heartbeat: 1000,
            consecutive_failures: 5,
        };

        detector.set_current_server(server).await;
        detector.reset().await;

        let current = detector.get_current_server().await.unwrap();
        assert_eq!(current.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_latency_based_selection() {
        let detector = ClientFailoverDetector::new(ClientFailoverConfig::default());

        let candidates = vec![
            ServerCandidate {
                server_id: "server1".to_string(),
                address: "127.0.0.1".to_string(),
                port: 5001,
                latency_ms: 50,
                priority: 1,
            },
            ServerCandidate {
                server_id: "server2".to_string(),
                address: "127.0.0.1".to_string(),
                port: 5002,
                latency_ms: 10,
                priority: 1,
            },
        ];

        detector.update_available_servers(candidates).await;
        let target = detector.find_reconnection_target().await;

        assert!(target.is_some());
        assert_eq!(target.unwrap().server_id, "server2"); // Lower latency
    }

    #[tokio::test]
    async fn test_reconnection_failed_sets_failed_state() {
        let config = ClientFailoverConfig::default();
        let detector = ClientFailoverDetector::new(config);

        let server = ServerConnection {
            server_id: "s1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 9000,
            is_connected: true,
            last_heartbeat: 0,
            consecutive_failures: 0,
        };

        // Exhaust reconnection attempts to trigger Failed state
        for _ in 0..10 {
            detector.attempt_reconnection(server.clone()).await;
        }

        detector.reconnection_failed().await;
        assert_eq!(detector.get_state().await, ClientFailoverState::Failed);
    }

    #[tokio::test]
    async fn test_get_reconnection_attempts() {
        let config = ClientFailoverConfig::default();
        let detector = ClientFailoverDetector::new(config);

        assert_eq!(detector.get_reconnection_attempts().await, 0);

        let server = ServerConnection {
            server_id: "s1".to_string(),
            address: "127.0.0.1".to_string(),
            port: 9000,
            is_connected: true,
            last_heartbeat: 0,
            consecutive_failures: 0,
        };

        detector.attempt_reconnection(server).await;
        assert_eq!(detector.get_reconnection_attempts().await, 1);
    }
}
