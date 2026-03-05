//! Inter-server connection management and pooling
//!
//! This module handles TCP connections between servers in the cluster,
//! including connection pooling, health monitoring, and graceful shutdown.

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Connection to a peer server
#[derive(Debug, Clone)]
pub struct PeerConnection {
    /// Target server ID
    pub server_id: String,

    /// Remote address
    pub remote_address: String,

    /// Whether connection is active
    pub is_active: bool,

    /// Connection attempt count
    pub attempt_count: u32,
}

impl PeerConnection {
    /// Create a new peer connection
    pub fn new(server_id: String, remote_address: String) -> Self {
        Self {
            server_id,
            remote_address,
            is_active: false,
            attempt_count: 0,
        }
    }
}

/// Connection pool for managing multiple peer connections
pub struct ConnectionPool {
    /// Map of server_id -> PeerConnection
    connections: Arc<RwLock<HashMap<String, PeerConnection>>>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a connection to a peer
    pub async fn get_connection(&self, server_id: &str, address: &str) -> Result<PeerConnection, String> {
        let mut conns = self.connections.write().await;

        if let Some(conn) = conns.get(server_id) {
            Ok(conn.clone())
        } else {
            let conn = PeerConnection::new(server_id.to_string(), address.to_string());
            conns.insert(server_id.to_string(), conn.clone());
            Ok(conn)
        }
    }

    /// Mark connection as active
    pub async fn mark_active(&self, server_id: &str) -> Result<(), String> {
        let mut conns = self.connections.write().await;
        if let Some(conn) = conns.get_mut(server_id) {
            conn.is_active = true;
            conn.attempt_count = 0;
            Ok(())
        } else {
            Err(format!("Connection {} not found", server_id))
        }
    }

    /// Mark connection as inactive
    pub async fn mark_inactive(&self, server_id: &str) -> Result<(), String> {
        let mut conns = self.connections.write().await;
        if let Some(conn) = conns.get_mut(server_id) {
            conn.is_active = false;
            conn.attempt_count += 1;
            Ok(())
        } else {
            Err(format!("Connection {} not found", server_id))
        }
    }

    /// Get all active connections
    pub async fn get_active_connections(&self) -> Vec<PeerConnection> {
        let conns = self.connections.read().await;
        conns
            .values()
            .filter(|c| c.is_active)
            .cloned()
            .collect()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let pool = ConnectionPool::new();
        let conn = pool
            .get_connection("peer_1", "127.0.0.1:9001")
            .await
            .unwrap();

        assert_eq!(conn.server_id, "peer_1");
        assert_eq!(conn.remote_address, "127.0.0.1:9001");
        assert!(!conn.is_active);
    }

    #[tokio::test]
    async fn test_mark_connection_active() {
        let pool = ConnectionPool::new();
        pool.get_connection("peer_1", "127.0.0.1:9001")
            .await
            .unwrap();
        pool.mark_active("peer_1").await.unwrap();

        let conn = pool
            .get_connection("peer_1", "127.0.0.1:9001")
            .await
            .unwrap();
        assert!(conn.is_active);
        assert_eq!(conn.attempt_count, 0);
    }

    #[tokio::test]
    async fn test_mark_connection_inactive() {
        let pool = ConnectionPool::new();
        pool.get_connection("peer_1", "127.0.0.1:9001")
            .await
            .unwrap();
        pool.mark_active("peer_1").await.unwrap();
        pool.mark_inactive("peer_1").await.unwrap();

        let conn = pool
            .get_connection("peer_1", "127.0.0.1:9001")
            .await
            .unwrap();
        assert!(!conn.is_active);
        assert_eq!(conn.attempt_count, 1);
    }

    #[tokio::test]
    async fn test_get_active_connections() {
        let pool = ConnectionPool::new();
        pool.get_connection("peer_1", "127.0.0.1:9001")
            .await
            .unwrap();
        pool.get_connection("peer_2", "127.0.0.1:9002")
            .await
            .unwrap();

        pool.mark_active("peer_1").await.unwrap();

        let active = pool.get_active_connections().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].server_id, "peer_1");
    }

    #[test]
    fn test_peer_connection_new() {
        let conn = PeerConnection::new("peer_x".to_string(), "10.0.0.1:8080".to_string());
        assert_eq!(conn.server_id, "peer_x");
        assert_eq!(conn.remote_address, "10.0.0.1:8080");
        assert!(!conn.is_active);
        assert_eq!(conn.attempt_count, 0);
    }

    #[tokio::test]
    async fn test_mark_active_not_found_returns_error() {
        let pool = ConnectionPool::new();
        // No connections registered; mark_active must return an error.
        let result = pool.mark_active("ghost_server").await;
        assert!(result.is_err(), "mark_active on unknown server_id should fail");
        assert!(
            result.unwrap_err().contains("not found"),
            "Error message should mention 'not found'"
        );
    }

    #[tokio::test]
    async fn test_mark_inactive_not_found_returns_error() {
        let pool = ConnectionPool::new();
        // No connections registered; mark_inactive must return an error.
        let result = pool.mark_inactive("ghost_server").await;
        assert!(result.is_err(), "mark_inactive on unknown server_id should fail");
        assert!(
            result.unwrap_err().contains("not found"),
            "Error message should mention 'not found'"
        );
    }
}
