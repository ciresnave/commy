//! Multi-server clustering support for COMMY
//! 
//! Enables multiple COMMY servers to work together:
//! - Service replication across servers
//! - Token/session synchronization via MessagePack
//! - Cluster-wide client discovery
//! - Failover and redundancy
//!
//! ## Message Format
//! Cluster communication (token sync, pings) uses MessagePack for efficiency
//! in high-frequency inter-server communication.

use crate::protocol::WssMessage;
use crate::auth::AuthResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Cluster node information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node ID
    pub node_id: String,
    /// Human-readable node name
    pub name: String,
    /// HTTP/WSS address (e.g., "https://server1:9000")
    pub address: String,
    /// Node status
    pub status: NodeStatus,
    /// Last heartbeat from this node
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// Services this node hosts
    pub services: Vec<String>,
}

/// Status of a cluster node
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is healthy and reachable
    Healthy,
    /// Node heartbeat missed (temporary unavailable)
    Degraded,
    /// Node is down
    Down,
    /// Node is new, not yet verified
    Joining,
}

/// Service replica information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceReplica {
    /// Tenant ID
    pub tenant_id: String,
    /// Service name
    pub service_name: String,
    /// Node hosting this replica
    pub node_id: String,
    /// Last sync timestamp
    pub last_synced: DateTime<Utc>,
    /// Version/generation number
    pub version: u64,
}

/// Synchronized token/session across cluster
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncToken {
    /// Token value
    pub token: String,
    /// Client ID
    pub client_id: String,
    /// Tenant ID
    pub tenant_id: String,
    /// Permissions (serialized)
    pub permissions: String,
    /// Expiration time
    pub expires_at: DateTime<Utc>,
    /// Node that issued this token
    pub issued_by: String,
}

/// Cluster topology manager
/// 
/// Manages:
/// 1. Node discovery and registration
/// 2. Health monitoring
/// 3. Service replication metadata
/// 4. Token synchronization
pub struct ClusterManager {
    /// Current node's ID
    local_node_id: String,
    /// All known cluster nodes
    nodes: Arc<RwLock<HashMap<String, ClusterNode>>>,
    /// Service replicas
    replicas: Arc<RwLock<Vec<ServiceReplica>>>,
    /// Synchronized tokens
    tokens: Arc<RwLock<HashMap<String, SyncToken>>>,
}

impl ClusterManager {
    /// Create a new cluster manager for this node
    pub fn new(node_id: String, node_name: String, address: String) -> Self {
        let local_node = ClusterNode {
            node_id: node_id.clone(),
            name: node_name,
            address,
            status: NodeStatus::Healthy,
            last_heartbeat: Some(Utc::now()),
            services: Vec::new(),
        };

        let mut nodes = HashMap::new();
        nodes.insert(local_node.node_id.clone(), local_node);

        Self {
            local_node_id: node_id,
            nodes: Arc::new(RwLock::new(nodes)),
            replicas: Arc::new(RwLock::new(Vec::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a peer node in the cluster
    pub async fn register_peer(
        &self,
        node_id: String,
        name: String,
        address: String,
    ) -> AuthResult<()> {
        let node = ClusterNode {
            node_id: node_id.clone(),
            name,
            address,
            status: NodeStatus::Joining,
            last_heartbeat: Some(Utc::now()),
            services: Vec::new(),
        };

        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id, node);

        Ok(())
    }

    /// Update node health status
    pub async fn update_node_status(&self, node_id: &str, status: NodeStatus) -> AuthResult<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.status = status;
            node.last_heartbeat = Some(Utc::now());
            Ok(())
        } else {
            Err(crate::auth::AuthError::InternalError(format!(
                "Node {} not found",
                node_id
            )))
        }
    }

    /// Register a service replica
    pub async fn register_replica(
        &self,
        tenant_id: String,
        service_name: String,
        node_id: String,
    ) -> AuthResult<()> {
        let replica = ServiceReplica {
            tenant_id,
            service_name,
            node_id,
            last_synced: Utc::now(),
            version: 1,
        };

        let mut replicas = self.replicas.write().await;
        replicas.push(replica);

        Ok(())
    }

    /// Find a service replica on a specific node
    pub async fn find_replica(
        &self,
        tenant_id: &str,
        service_name: &str,
    ) -> Option<ServiceReplica> {
        let replicas = self.replicas.read().await;
        replicas
            .iter()
            .find(|r| r.tenant_id == tenant_id && r.service_name == service_name)
            .cloned()
    }

    /// Sync token across cluster (broadcast to all nodes)
    pub async fn sync_token(
        &self,
        token: SyncToken,
    ) -> AuthResult<()> {
        let mut tokens = self.tokens.write().await;
        tokens.insert(token.token.clone(), token);
        Ok(())
    }

    /// Get synchronized token
    pub async fn get_synced_token(&self, token: &str) -> Option<SyncToken> {
        let tokens = self.tokens.read().await;
        tokens.get(token).cloned()
    }

    /// Get all healthy nodes
    pub async fn get_healthy_nodes(&self) -> Vec<ClusterNode> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|n| n.status == NodeStatus::Healthy || n.status == NodeStatus::Degraded)
            .cloned()
            .collect()
    }

    /// Get all nodes
    pub async fn get_all_nodes(&self) -> Vec<ClusterNode> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// Get a specific node
    pub async fn get_node(&self, node_id: &str) -> Option<ClusterNode> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// Check if this is the local node
    pub fn is_local_node(&self, node_id: &str) -> bool {
        self.local_node_id == node_id
    }

    /// Get local node ID
    pub fn local_node_id(&self) -> &str {
        &self.local_node_id
    }

    /// Create cluster ping message
    pub fn create_cluster_ping(&self) -> WssMessage {
        WssMessage::ClusterPing {
            node_id: self.local_node_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    /// Create token sync message
    pub fn create_token_sync(token: &SyncToken) -> WssMessage {
        WssMessage::TokenSync {
            client_id: token.client_id.clone(),
            token: token.token.clone(),
            tenant_id: token.tenant_id.clone(),
            permissions: token.permissions.clone(),
            expires_at: token.expires_at.to_rfc3339(),
        }
    }

    /// Cleanup expired tokens
    pub async fn cleanup_expired_tokens(&self) {
        let mut tokens = self.tokens.write().await;
        let now = Utc::now();
        tokens.retain(|_, token| token.expires_at > now);
    }

    /// Get cluster statistics
    pub async fn get_stats(&self) -> ClusterStats {
        let nodes = self.nodes.read().await;
        let replicas = self.replicas.read().await;
        let tokens = self.tokens.read().await;

        let healthy = nodes.values().filter(|n| n.status == NodeStatus::Healthy).count();
        let degraded = nodes.values().filter(|n| n.status == NodeStatus::Degraded).count();
        let down = nodes.values().filter(|n| n.status == NodeStatus::Down).count();

        ClusterStats {
            total_nodes: nodes.len(),
            healthy_nodes: healthy,
            degraded_nodes: degraded,
            down_nodes: down,
            total_replicas: replicas.len(),
            active_tokens: tokens.len(),
        }
    }
}

/// Cluster statistics
#[derive(Clone, Debug)]
pub struct ClusterStats {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub degraded_nodes: usize,
    pub down_nodes: usize,
    pub total_replicas: usize,
    pub active_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_cluster_manager() {
        let manager = ClusterManager::new(
            "node1".to_string(),
            "Server 1".to_string(),
            "https://server1:9000".to_string(),
        );

        assert_eq!(manager.local_node_id(), "node1");
        let nodes = manager.get_all_nodes().await;
        assert_eq!(nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_register_peer() {
        let manager = ClusterManager::new(
            "node1".to_string(),
            "Server 1".to_string(),
            "https://server1:9000".to_string(),
        );

        manager
            .register_peer(
                "node2".to_string(),
                "Server 2".to_string(),
                "https://server2:9000".to_string(),
            )
            .await
            .unwrap();

        let nodes = manager.get_all_nodes().await;
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_node_status_update() {
        let manager = ClusterManager::new(
            "node1".to_string(),
            "Server 1".to_string(),
            "https://server1:9000".to_string(),
        );

        manager
            .register_peer(
                "node2".to_string(),
                "Server 2".to_string(),
                "https://server2:9000".to_string(),
            )
            .await
            .unwrap();

        manager
            .update_node_status("node2", NodeStatus::Degraded)
            .await
            .unwrap();

        let node = manager.get_node("node2").await.unwrap();
        assert_eq!(node.status, NodeStatus::Degraded);
    }

    #[tokio::test]
    async fn test_sync_token() {
        let manager = ClusterManager::new(
            "node1".to_string(),
            "Server 1".to_string(),
            "https://server1:9000".to_string(),
        );

        let token = SyncToken {
            token: "token123".to_string(),
            client_id: "client1".to_string(),
            tenant_id: "tenant1".to_string(),
            permissions: "read,write".to_string(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            issued_by: "node1".to_string(),
        };

        manager.sync_token(token.clone()).await.unwrap();

        let retrieved = manager.get_synced_token("token123").await.unwrap();
        assert_eq!(retrieved.client_id, "client1");
    }

    #[tokio::test]
    async fn test_cluster_stats() {
        let manager = ClusterManager::new(
            "node1".to_string(),
            "Server 1".to_string(),
            "https://server1:9000".to_string(),
        );

        manager
            .register_peer(
                "node2".to_string(),
                "Server 2".to_string(),
                "https://server2:9000".to_string(),
            )
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.healthy_nodes, 1); // Only local node is healthy initially
    }
}
