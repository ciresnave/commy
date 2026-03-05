//! Heartbeat and liveness detection for peer servers
//!
//! Maintains periodic heartbeat with all peers and detects server failures
//! through timeout and response monitoring.

use super::messages::ServerMessage;
use super::protocol::ProtocolHandler;
use super::registry::PeerRegistry;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::interval;

/// Heartbeat service for monitoring peer health
pub struct HeartbeatService {
    /// Peer registry to update
    registry: Arc<PeerRegistry>,

    /// Protocol handler for sending heartbeats
    protocol: Arc<ProtocolHandler>,

    /// This server's ID
    server_id: String,

    /// Heartbeat configuration
    config: HeartbeatConfig,

    /// Current sequence number
    sequence: u64,
}

/// Heartbeat service configuration
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeats
    pub heartbeat_interval: Duration,

    /// How long to wait for heartbeat response before timeout
    pub heartbeat_timeout: Duration,

    /// Maximum retries before marking peer as down
    pub max_retries: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(60),
            max_retries: 5,
        }
    }
}

impl HeartbeatService {
    /// Create a new heartbeat service
    pub fn new(
        server_id: String,
        registry: Arc<PeerRegistry>,
        protocol: Arc<ProtocolHandler>,
    ) -> Self {
        Self {
            server_id,
            registry,
            protocol,
            config: HeartbeatConfig::default(),
            sequence: 0,
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        server_id: String,
        registry: Arc<PeerRegistry>,
        protocol: Arc<ProtocolHandler>,
        config: HeartbeatConfig,
    ) -> Self {
        Self {
            server_id,
            registry,
            protocol,
            config,
            sequence: 0,
        }
    }

    /// Start the heartbeat service (returns task handle)
    pub fn start(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = interval(self.config.heartbeat_interval);

            loop {
                ticker.tick().await;

                // Get all peers from registry
                let peers = self.registry.get_all_peers().await;

                for peer in peers {
                    // Send heartbeat
                    self.sequence = self.sequence.wrapping_add(1);

                    let msg = ServerMessage::HeartbeatPing {
                        server_id: self.server_id.clone(),
                        timestamp: Self::current_timestamp(),
                        sequence: self.sequence,
                    };

                    let send_result = self
                        .protocol
                        .send_message(&peer.server_id, &peer.address, msg)
                        .await;

                    match send_result {
                        Ok(_) => {
                            // Successfully sent, mark heartbeat received
                            let _ = self
                                .registry
                                .mark_heartbeat_received(&peer.server_id)
                                .await;
                        }
                        Err(_) => {
                            // Send failed, mark heartbeat missed
                            let _ = self
                                .registry
                                .mark_heartbeat_missed(&peer.server_id)
                                .await;
                        }
                    }
                }
            }
        })
    }

    /// Get current Unix timestamp in milliseconds
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Get configuration
    pub fn config(&self) -> &HeartbeatConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: HeartbeatConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::clustering::ConnectionPool;

    #[test]
    fn test_heartbeat_config_default() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(config.heartbeat_timeout, Duration::from_secs(60));
        assert_eq!(config.max_retries, 5);
    }

    #[tokio::test]
    async fn test_heartbeat_service_creation() {
        let config = crate::server::clustering::PeerConfig::default();
        let registry = Arc::new(PeerRegistry::new(config));
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);

        let service = HeartbeatService::new(
            "server_1".to_string(),
            registry,
            Arc::new(handler),
        );

        assert_eq!(service.server_id, "server_1");
        assert_eq!(service.sequence, 0);
    }

    #[test]
    fn test_heartbeat_timestamp() {
        let ts1 = HeartbeatService::current_timestamp();
        let ts2 = HeartbeatService::current_timestamp();

        // Timestamps should be close (within 1000ms)
        assert!(ts2 >= ts1);
        assert!(ts2 - ts1 < 1000);
    }

    #[tokio::test]
    async fn test_heartbeat_service_with_custom_config() {
        let config = crate::server::clustering::PeerConfig::default();
        let registry = Arc::new(PeerRegistry::new(config));
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);

        let hb_config = HeartbeatConfig {
            heartbeat_interval: Duration::from_secs(10),
            heartbeat_timeout: Duration::from_secs(20),
            max_retries: 3,
        };

        let service = HeartbeatService::with_config(
            "server_1".to_string(),
            registry,
            Arc::new(handler),
            hb_config.clone(),
        );

        assert_eq!(service.config.heartbeat_interval, Duration::from_secs(10));
        assert_eq!(service.config.max_retries, 3);
    }

    #[test]
    fn test_update_config() {
        let config = crate::server::clustering::PeerConfig::default();
        let registry = Arc::new(PeerRegistry::new(config));
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);

        let mut service = HeartbeatService::new(
            "server_1".to_string(),
            registry,
            Arc::new(handler),
        );

        let new_config = HeartbeatConfig {
            heartbeat_interval: Duration::from_secs(5),
            heartbeat_timeout: Duration::from_secs(10),
            max_retries: 2,
        };

        service.update_config(new_config);

        assert_eq!(service.config.heartbeat_interval, Duration::from_secs(5));
        assert_eq!(service.config.heartbeat_timeout, Duration::from_secs(10));
        assert_eq!(service.config.max_retries, 2);
    }

    #[tokio::test]
    async fn test_heartbeat_service_config_getter() {
        let peer_config = crate::server::clustering::PeerConfig::default();
        let registry = Arc::new(PeerRegistry::new(peer_config));
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);

        let custom_config = HeartbeatConfig {
            heartbeat_interval: Duration::from_secs(15),
            heartbeat_timeout: Duration::from_secs(45),
            max_retries: 7,
        };

        let service = HeartbeatService::with_config(
            "server_1".to_string(),
            registry,
            Arc::new(handler),
            custom_config,
        );

        // Access configuration via the public getter
        let cfg = service.config();
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(15));
        assert_eq!(cfg.heartbeat_timeout, Duration::from_secs(45));
        assert_eq!(cfg.max_retries, 7);
    }
}
