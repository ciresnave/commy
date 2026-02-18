//! Peer registry for cluster management

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;

use super::peer::{PeerInfo, PeerStatus};

/// Configuration for peer discovery
#[derive(Debug, Clone)]
pub struct PeerConfig {
    /// Static list of known peers (host:port)
    pub static_peers: Vec<String>,

    /// This server's ID
    pub server_id: String,

    /// This server's advertised address
    pub listen_address: String,

    /// Heartbeat interval
    pub heartbeat_interval: Duration,

    /// Heartbeat timeout (how long to wait before marking as suspected)
    pub heartbeat_timeout: Duration,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            static_peers: vec![],
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(60),
        }
    }
}

/// Manages peer discovery and health monitoring for the cluster
pub struct PeerRegistry {
    /// This server's configuration
    config: PeerConfig,

    /// Map of peer_id -> PeerInfo
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,

    /// Total peers in cluster (including self)
    cluster_size: usize,
}

impl PeerRegistry {
    /// Create a new peer registry
    pub fn new(config: PeerConfig) -> Self {
        Self {
            config,
            peers: Arc::new(RwLock::new(HashMap::new())),
            cluster_size: 0,
        }
    }

    /// Initialize peer registry with static peers
    pub async fn initialize(&mut self) -> Result<(), String> {
        let mut peers = self.peers.write().await;

        // Generate server IDs for static peers
        for (idx, address) in self.config.static_peers.iter().enumerate() {
            let server_id = format!("peer_{}", idx + 1);
            let peer_info = PeerInfo::new(server_id.clone(), address.clone());
            peers.insert(server_id, peer_info);
        }

        self.cluster_size = self.config.static_peers.len() + 1; // +1 for self

        Ok(())
    }

    /// Get all healthy peers
    pub async fn get_healthy_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.is_healthy())
            .cloned()
            .collect()
    }

    /// Get all peers regardless of status
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get a specific peer by ID
    pub async fn get_peer(&self, server_id: &str) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.get(server_id).cloned()
    }

    /// Mark a peer as having received a heartbeat
    pub async fn mark_heartbeat_received(&self, server_id: &str) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(server_id) {
            peer.heartbeat_received();
            Ok(())
        } else {
            Err(format!("Peer {} not found", server_id))
        }
    }

    /// Mark a peer as having missed a heartbeat
    pub async fn mark_heartbeat_missed(&self, server_id: &str) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(server_id) {
            peer.heartbeat_missed();
            Ok(())
        } else {
            Err(format!("Peer {} not found", server_id))
        }
    }

    /// Get cluster health status
    pub async fn get_cluster_status(&self) -> ClusterStatus {
        let peers = self.peers.read().await;

        let healthy_count = peers.values().filter(|p| p.is_healthy()).count();
        let suspected_count = peers.values().filter(|p| p.status == PeerStatus::Suspected).count();
        let down_count = peers.values().filter(|p| p.status == PeerStatus::Down).count();

        ClusterStatus {
            total_peers: self.cluster_size,
            healthy_peers: healthy_count + 1, // +1 for self (always healthy)
            suspected_peers: suspected_count,
            down_peers: down_count,
            is_degraded: suspected_count > 0 || down_count > 0,
        }
    }

    /// Record bytes received from a peer
    pub async fn add_bytes_received(&self, server_id: &str, bytes: u64) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(server_id) {
            peer.add_bytes_received(bytes);
            Ok(())
        } else {
            Err(format!("Peer {} not found", server_id))
        }
    }

    /// Record bytes sent to a peer
    pub async fn add_bytes_sent(&self, server_id: &str, bytes: u64) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(server_id) {
            peer.add_bytes_sent(bytes);
            Ok(())
        } else {
            Err(format!("Peer {} not found", server_id))
        }
    }

    /// Get configuration
    pub fn config(&self) -> &PeerConfig {
        &self.config
    }

    /// Get cluster size (including self)
    pub fn cluster_size(&self) -> usize {
        self.cluster_size
    }

    /// Add a peer (primarily for testing)
    pub async fn add_peer(&mut self, peer: PeerInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(peer.server_id.clone(), peer);
    }
}

/// Cluster health status
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    /// Total number of peers (including this server)
    pub total_peers: usize,
    /// Number of healthy peers
    pub healthy_peers: usize,
    /// Number of suspected peers
    pub suspected_peers: usize,
    /// Number of down peers
    pub down_peers: usize,
    /// Whether cluster is operating in degraded mode
    pub is_degraded: bool,
}

impl ClusterStatus {
    /// Check if cluster is quorum available
    pub fn is_quorum_available(&self) -> bool {
        self.healthy_peers > self.total_peers / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_registry_creation() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string(), "127.0.0.1:9002".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        assert_eq!(registry.cluster_size(), 3); // 1 local + 2 static peers
    }

    #[tokio::test]
    async fn test_get_all_peers() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string(), "127.0.0.1:9002".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        let peers = registry.get_all_peers().await;
        assert_eq!(peers.len(), 2);
    }

    #[tokio::test]
    async fn test_get_healthy_peers() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string(), "127.0.0.1:9002".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        let healthy = registry.get_healthy_peers().await;
        assert_eq!(healthy.len(), 2); // All should be healthy initially
    }

    #[tokio::test]
    async fn test_mark_heartbeat_received() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        let peer = registry.get_peer("peer_1").await.unwrap();
        assert!(peer.is_healthy());

        registry.mark_heartbeat_received("peer_1").await.unwrap();
        let peer = registry.get_peer("peer_1").await.unwrap();
        assert!(peer.is_healthy());
    }

    #[tokio::test]
    async fn test_mark_heartbeat_missed() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        // Miss some heartbeats
        registry.mark_heartbeat_missed("peer_1").await.unwrap();
        registry.mark_heartbeat_missed("peer_1").await.unwrap();

        let peer = registry.get_peer("peer_1").await.unwrap();
        assert_eq!(peer.status, PeerStatus::Suspected);
    }

    #[tokio::test]
    async fn test_cluster_status() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string(), "127.0.0.1:9002".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        let status = registry.get_cluster_status().await;
        assert_eq!(status.total_peers, 3);
        assert_eq!(status.healthy_peers, 3); // All healthy initially
        assert!(!status.is_degraded);
        assert!(status.is_quorum_available());
    }

    #[tokio::test]
    async fn test_bytes_tracking() {
        let config = PeerConfig {
            server_id: "server_1".to_string(),
            listen_address: "127.0.0.1:9000".to_string(),
            static_peers: vec!["127.0.0.1:9001".to_string()],
            ..Default::default()
        };

        let mut registry = PeerRegistry::new(config);
        registry.initialize().await.unwrap();

        registry.add_bytes_received("peer_1", 1024).await.unwrap();
        registry.add_bytes_sent("peer_1", 512).await.unwrap();

        let peer = registry.get_peer("peer_1").await.unwrap();
        assert_eq!(peer.bytes_received, 1024);
        assert_eq!(peer.bytes_sent, 512);
    }
}
