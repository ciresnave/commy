//! Service state replication coordinator
//!
//! Manages state transfer between servers, including synchronization,
//! chunked transfers, and consistency verification.

use super::messages::ServerMessage;
use super::protocol::ProtocolHandler;
use super::snapshots::{ServiceSnapshot, SnapshotTransfer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Replication coordinator for managing state transfers
pub struct ReplicationCoordinator {
    /// This server's ID
    server_id: String,

    /// Protocol handler for sending/receiving
    protocol: Arc<ProtocolHandler>,

    /// Ongoing transfers (request_id -> transfer state)
    transfers: Arc<RwLock<HashMap<String, SnapshotTransfer>>>,

    /// Replication configuration
    config: Arc<RwLock<ReplicationConfig>>,
}

/// Replication configuration
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// Chunk size for file transfers (default: 1MB)
    pub chunk_size: usize,

    /// Maximum number of concurrent transfers
    pub max_concurrent_transfers: u32,

    /// Transfer timeout in milliseconds (default: 5 minutes)
    pub transfer_timeout_ms: u64,

    /// Stall detection threshold in milliseconds (default: 30 seconds)
    pub stall_threshold_ms: u64,

    /// Enable compression for transfers
    pub enable_compression: bool,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1024 * 1024, // 1MB
            max_concurrent_transfers: 10,
            transfer_timeout_ms: 5 * 60 * 1000, // 5 minutes
            stall_threshold_ms: 30 * 1000, // 30 seconds
            enable_compression: false,
        }
    }
}

impl ReplicationCoordinator {
    /// Create a new replication coordinator
    pub fn new(server_id: String, protocol: Arc<ProtocolHandler>) -> Self {
        Self {
            server_id,
            protocol,
            transfers: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(ReplicationConfig::default())),
        }
    }

    /// Request service state from a peer
    pub async fn request_sync(
        &self,
        peer_id: &str,
        peer_address: &str,
        tenant_name: &str,
        service_name: &str,
        from_version: Option<u64>,
    ) -> Result<String, String> {
        let msg = ServerMessage::SyncServiceRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            tenant_name: tenant_name.to_string(),
            service_name: service_name.to_string(),
            from_version,
            from_checksum: None,
        };

        self.protocol
            .send_message(peer_id, peer_address, msg)
            .await
    }

    /// Handle incoming sync request
    pub async fn handle_sync_request(
        &self,
        _request_id: &str,
        _tenant_name: &str,
        _service_name: &str,
        _from_version: Option<u64>,
    ) -> Result<ServiceSnapshot, String> {
        // In real implementation, this would:
        // 1. Load service data from local registry
        // 2. Create snapshot
        // 3. Calculate checksum
        // 4. Return snapshot for response

        // For now, return error (implemented in integration)
        Err("Service not found locally".to_string())
    }

    /// Start a service state transfer
    pub async fn start_transfer(
        &self,
        transfer_id: String,
        snapshot: ServiceSnapshot,
    ) -> Result<(), String> {
        let mut transfers = self.transfers.write().await;

        if transfers.len() as u32 >= self.get_config().await.max_concurrent_transfers {
            return Err("Too many concurrent transfers".to_string());
        }

        let transfer = SnapshotTransfer::new(transfer_id, snapshot);
        transfers.insert(transfer.transfer_id.clone(), transfer);

        Ok(())
    }

    /// Get transfer for chunk data
    pub async fn get_transfer_for_chunk(
        &self,
        transfer_id: &str,
    ) -> Result<SnapshotTransfer, String> {
        let transfers = self.transfers.read().await;
        transfers
            .get(transfer_id)
            .cloned()
            .ok_or_else(|| format!("Transfer {} not found", transfer_id))
    }

    /// Mark chunk as received
    pub async fn mark_chunk_received(
        &self,
        transfer_id: &str,
        offset: u64,
        size: u64,
    ) -> Result<(), String> {
        let mut transfers = self.transfers.write().await;

        if let Some(transfer) = transfers.get_mut(transfer_id) {
            transfer.mark_chunk_transferred(offset, size);

            if transfer.is_complete {
                // Optionally clean up completed transfer
                // (keep for now to allow verification)
            }

            Ok(())
        } else {
            Err(format!("Transfer {} not found", transfer_id))
        }
    }

    /// Complete a transfer and return snapshot
    pub async fn complete_transfer(
        &self,
        transfer_id: &str,
    ) -> Result<ServiceSnapshot, String> {
        let mut transfers = self.transfers.write().await;

        if let Some(transfer) = transfers.remove(transfer_id) {
            if !transfer.is_complete {
                return Err("Transfer not complete".to_string());
            }

            // Verify checksum
            if !transfer.snapshot.verify_checksum() {
                return Err("Checksum verification failed".to_string());
            }

            Ok(transfer.snapshot)
        } else {
            Err(format!("Transfer {} not found", transfer_id))
        }
    }

    /// Get configuration
    pub async fn get_config(&self) -> ReplicationConfig {
        self.config.read().await.clone()
    }

    /// Update configuration
    pub async fn update_config(&self, config: ReplicationConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    /// Get active transfers
    pub async fn get_active_transfers(&self) -> Vec<String> {
        let transfers = self.transfers.read().await;
        transfers.keys().cloned().collect()
    }

    /// Get transfer progress
    pub async fn get_transfer_progress(&self, transfer_id: &str) -> Result<u32, String> {
        let transfers = self.transfers.read().await;
        transfers
            .get(transfer_id)
            .map(|t| t.progress_percent())
            .ok_or_else(|| format!("Transfer {} not found", transfer_id))
    }

    /// Clean up stalled transfers
    pub async fn cleanup_stalled_transfers(&self) -> Vec<String> {
        let config = self.get_config().await;
        let mut transfers = self.transfers.write().await;

        let stalled: Vec<String> = transfers
            .iter()
            .filter(|(_, t)| t.is_stalled(config.stall_threshold_ms))
            .map(|(id, _)| id.clone())
            .collect();

        for id in &stalled {
            transfers.remove(id);
        }

        stalled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::clustering::ConnectionPool;

    #[test]
    fn test_replication_config_default() {
        let config = ReplicationConfig::default();
        assert_eq!(config.chunk_size, 1024 * 1024);
        assert_eq!(config.max_concurrent_transfers, 10);
        assert_eq!(config.transfer_timeout_ms, 5 * 60 * 1000);
    }

    #[tokio::test]
    async fn test_replication_coordinator_creation() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        assert_eq!(coordinator.server_id, "server_1");

        let transfers = coordinator.get_active_transfers().await;
        assert!(transfers.is_empty());
    }

    #[tokio::test]
    async fn test_start_transfer() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            b"test data".to_vec(),
            ServiceSnapshot::calculate_checksum(b"test data"),
        );

        let result = coordinator
            .start_transfer("transfer_1".to_string(), snapshot)
            .await;

        assert!(result.is_ok());
        let transfers = coordinator.get_active_transfers().await;
        assert_eq!(transfers.len(), 1);
    }

    #[tokio::test]
    async fn test_mark_chunk_received() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let data = b"test data with more content".to_vec();
        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data.clone(),
            ServiceSnapshot::calculate_checksum(&data),
        );

        coordinator
            .start_transfer("transfer_1".to_string(), snapshot)
            .await
            .unwrap();

        coordinator
            .mark_chunk_received("transfer_1", 0, data.len() as u64)
            .await
            .unwrap();

        let progress = coordinator
            .get_transfer_progress("transfer_1")
            .await
            .unwrap();
        assert_eq!(progress, 100);
    }

    #[tokio::test]
    async fn test_complete_transfer() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let data = b"test data".to_vec();
        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data.clone(),
            ServiceSnapshot::calculate_checksum(&data),
        );

        coordinator
            .start_transfer("transfer_1".to_string(), snapshot.clone())
            .await
            .unwrap();

        coordinator
            .mark_chunk_received("transfer_1", 0, data.len() as u64)
            .await
            .unwrap();

        let completed = coordinator.complete_transfer("transfer_1").await.unwrap();
        assert_eq!(completed.service_name, "service1");
    }

    #[tokio::test]
    async fn test_config_update() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let mut config = coordinator.get_config().await;
        config.chunk_size = 2 * 1024 * 1024;

        coordinator.update_config(config.clone()).await;

        let updated = coordinator.get_config().await;
        assert_eq!(updated.chunk_size, 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_get_transfer_for_chunk_not_found() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let result = coordinator.get_transfer_for_chunk("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_active_transfers_multiple() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let data = b"data".to_vec();
        let snap1 = ServiceSnapshot::new(
            "t".to_string(),
            "s1".to_string(),
            1,
            data.clone(),
            ServiceSnapshot::calculate_checksum(&data),
        );
        let snap2 = ServiceSnapshot::new(
            "t".to_string(),
            "s2".to_string(),
            1,
            data.clone(),
            ServiceSnapshot::calculate_checksum(&data),
        );
        coordinator.start_transfer("t1".to_string(), snap1).await.unwrap();
        coordinator.start_transfer("t2".to_string(), snap2).await.unwrap();

        let transfers = coordinator.get_active_transfers().await;
        assert_eq!(transfers.len(), 2);
    }

    #[tokio::test]
    async fn test_get_transfer_progress_not_found() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let result = coordinator.get_transfer_progress("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_stalled_transfers() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let data = b"data".to_vec();
        let snap = ServiceSnapshot::new(
            "t".to_string(),
            "s".to_string(),
            1,
            data.clone(),
            ServiceSnapshot::calculate_checksum(&data),
        );
        coordinator.start_transfer("stale".to_string(), snap).await.unwrap();

        // Sleep so time passes, then use a short threshold so transfer is considered stalled
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let mut config = coordinator.get_config().await;
        config.stall_threshold_ms = 5; // Transfer has been idle > 5ms (we slept 20ms)
        coordinator.update_config(config).await;

        let removed = coordinator.cleanup_stalled_transfers().await;
        assert!(!removed.is_empty());
        assert_eq!(coordinator.get_active_transfers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_handle_sync_request_returns_error() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator = ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let result = coordinator
            .handle_sync_request("req_1", "tenant1", "service1", None)
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_replication_config_stall_threshold() {
        let config = ReplicationConfig::default();
        assert_eq!(config.stall_threshold_ms, 30 * 1000);
        assert!(!config.enable_compression);
    }

    #[tokio::test]
    async fn test_complete_transfer_not_complete_returns_error() {
        // Start a transfer but never mark any chunks received → is_complete stays false.
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator =
            ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let data = b"some payload".to_vec();
        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data.clone(),
            ServiceSnapshot::calculate_checksum(&data),
        );

        coordinator
            .start_transfer("t_incomplete".to_string(), snapshot)
            .await
            .unwrap();

        // Intentionally skip mark_chunk_received so transfer stays incomplete.
        let result = coordinator.complete_transfer("t_incomplete").await;
        assert!(result.is_err(), "Expected error for incomplete transfer");
        assert!(
            result.unwrap_err().contains("Transfer not complete"),
            "Expected 'Transfer not complete' error"
        );
    }

    #[tokio::test]
    async fn test_complete_transfer_bad_checksum_returns_error() {
        // Build a snapshot with a deliberately wrong checksum.
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);
        let coordinator =
            ReplicationCoordinator::new("server_1".to_string(), Arc::new(handler));

        let data = b"real data payload".to_vec();
        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data.clone(),
            "intentionally_wrong_checksum".to_string(),
        );

        coordinator
            .start_transfer("t_badchk".to_string(), snapshot)
            .await
            .unwrap();

        // Mark as complete so we get past the is_complete guard.
        coordinator
            .mark_chunk_received("t_badchk", 0, data.len() as u64)
            .await
            .unwrap();

        let result = coordinator.complete_transfer("t_badchk").await;
        assert!(result.is_err(), "Expected error for bad checksum");
        assert!(
            result.unwrap_err().contains("Checksum verification failed"),
            "Expected 'Checksum verification failed' error"
        );
    }
}
