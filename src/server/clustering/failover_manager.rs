use crate::protocol::ClientSession;
/// Failover Manager - Detects peer failures and orchestrates service migration
use crate::server::clustering::{
    peer::{PeerInfo, PeerStatus},
    registry::{PeerConfig, PeerRegistry},
    replication::{ReplicationConfig, ReplicationCoordinator},
    session_persistence::{SessionData, SessionStore},
};
use crate::server::session_manager::SessionManager;
use chrono::TimeZone;
use chrono::Utc;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a failed peer and services that need migration
#[derive(Clone, Debug)]
pub struct FailedPeer {
    pub peer_id: String,
    pub server_addr: String,
    pub services: Vec<String>,
    pub failure_time: u64,
    pub reason: FailureReason,
}

/// Reason a peer failed
#[derive(Clone, Debug, PartialEq)]
pub enum FailureReason {
    HeartbeatTimeout,
    ConnectionLost,
    OutboundQueueStall,
    ManualShutdown,
}

/// Migration status for a service
#[derive(Clone, Debug, PartialEq)]
pub enum MigrationStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

/// Tracks a service migration operation
#[derive(Clone, Debug)]
pub struct ServiceMigration {
    pub service_name: String,
    pub source_peer: String,
    pub target_peer: String,
    pub status: MigrationStatus,
    pub transfer_id: Option<String>,
    pub start_time: u64,
    pub completion_time: Option<u64>,
}

/// Configuration for failover behavior
#[derive(Clone, Debug)]
pub struct FailoverConfig {
    /// Maximum time to wait for migration to complete (ms)
    pub migration_timeout_ms: u64,
    /// Time before marking a peer as failed after heartbeat detection (ms)
    pub failure_confirmation_delay_ms: u64,
    /// Enable automatic migration of services from failed peers
    pub auto_migrate_services: bool,
    /// Preferred peer for migrations (load balancing)
    pub preferred_target_selection: TargetSelection,
}

/// Strategy for selecting target peer for migration
#[derive(Clone, Debug, PartialEq)]
pub enum TargetSelection {
    /// Select peer with fewest services
    LeastLoaded,
    /// Select first healthy peer
    FirstAvailable,
    /// Round-robin selection
    RoundRobin,
    /// Custom peer ID
    Specific(String),
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            migration_timeout_ms: 30000,
            failure_confirmation_delay_ms: 5000,
            auto_migrate_services: true,
            preferred_target_selection: TargetSelection::LeastLoaded,
        }
    }
}

/// Manages failover detection and service migration
pub struct FailoverManager {
    config: FailoverConfig,
    peer_registry: Arc<RwLock<PeerRegistry>>,
    replication_coordinator: Arc<RwLock<ReplicationCoordinator>>,
    /// Optional session store used to persist/restore client sessions during migration
    session_store: Option<Arc<SessionStore>>,
    /// Optional session manager used to re-associate restored sessions on target
    session_manager: Option<Arc<SessionManager>>,
    /// (source_peer, service_name) -> migration info
    migrations: Arc<RwLock<HashMap<(String, String), ServiceMigration>>>,
    /// Failed peers that need cleanup
    failed_peers: Arc<RwLock<Vec<FailedPeer>>>,
}

impl FailoverManager {
    /// Create new failover manager
    pub fn new(
        config: FailoverConfig,
        peer_registry: Arc<RwLock<PeerRegistry>>,
        replication_coordinator: Arc<RwLock<ReplicationCoordinator>>,
    ) -> Self {
        Self {
            config,
            peer_registry,
            replication_coordinator,
            session_store: None,
            session_manager: None,
            migrations: Arc::new(RwLock::new(HashMap::new())),
            failed_peers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Set an optional `SessionStore` to enable persisting sessions during migrations
    pub fn set_session_store(&mut self, store: Arc<SessionStore>) {
        self.session_store = Some(store);
    }

    /// Set an optional `SessionManager` so restored sessions can be re-registered
    pub fn set_session_manager(&mut self, manager: Arc<SessionManager>) {
        self.session_manager = Some(manager);
    }

    /// Restore sessions for a given service from the configured session store.
    /// This is intended to be called on the target server after a migration completes.
    pub async fn restore_sessions_for_service(&self, _service_name: &str) -> Result<(), String> {
        if let Some(store) = &self.session_store {
            // Load persisted sessions from disk into the store's in-memory cache
            store.load_all().await?;

            // Find sessions referencing the service and re-register them if a SessionManager is configured
            if let Some(sm) = &self.session_manager {
                let sessions = store.list_sessions_by_service(_service_name).await;
                for sess in sessions {
                    let mut cs = ClientSession::new();
                    cs.session_id = sess.session_id.clone();
                    cs.client_id = Some(sess.client_id.clone());
                    cs.tenant_id = Some(sess.tenant.clone());
                    cs.state = crate::protocol::ClientState::Active;
                    cs.last_activity = Utc
                        .timestamp_millis_opt(sess.last_activity as i64)
                        .single()
                        .unwrap_or_else(|| Utc::now());
                    cs.last_heartbeat_ack = None;
                    cs.outbound_queue_size = 0;
                    cs.subscriptions = sess.services.into_iter().collect::<HashSet<String>>();

                    sm.register_session(cs).await;
                }
            }

            Ok(())
        } else {
            Err("No SessionStore configured for this FailoverManager".to_string())
        }
    }

    /// Check for failed peers and trigger migrations
    pub async fn check_peer_health(&self) -> Vec<FailedPeer> {
        let registry = self.peer_registry.read().await;
        let mut failed = Vec::new();

        let peers = registry.get_all_peers().await;
        for peer in peers {
            if peer.status == PeerStatus::Down {
                let failed_peer = FailedPeer {
                    peer_id: peer.server_id.clone(),
                    server_addr: peer.address.clone(),
                    services: vec![], // Would be populated from server's service registry
                    failure_time: Self::current_timestamp(),
                    reason: FailureReason::HeartbeatTimeout,
                };
                failed.push(failed_peer);
            }
        }

        // Store failed peers
        if !failed.is_empty() && self.config.auto_migrate_services {
            let mut peers_list = self.failed_peers.write().await;
            peers_list.extend(failed.clone());
        }

        failed
    }

    /// Request migration of a service to a healthy peer
    pub async fn migrate_service(
        &self,
        service_name: String,
        source_peer: String,
        target_peer: Option<String>,
    ) -> Result<String, String> {
        let registry = self.peer_registry.read().await;

        // Validate source peer is down
        if let Some(peer) = registry.get_peer(&source_peer).await {
            if peer.status != PeerStatus::Down {
                return Err(format!("Source peer {} is still healthy", source_peer));
            }
        } else {
            return Err(format!("Source peer {} not found", source_peer));
        }

        // Get all peers for selection
        let peers = registry.get_all_peers().await;

        // Select target peer
        let target =
            target_peer.or_else(|| self.select_migration_target(&registry, &source_peer, &peers));

        let target_peer_id = target.ok_or("No healthy peers available for migration")?;

        if registry.get_peer(&target_peer_id).await.is_none() {
            return Err(format!("Target peer {} not found", target_peer_id));
        }

        // Persist any sessions that reference this service to durable store before migration
        if let Some(store) = &self.session_store {
            let sessions: Vec<SessionData> = store.list_sessions_by_service(&service_name).await;
            for sess in sessions {
                if let Err(e) = store.persist_session(&sess).await {
                    return Err(format!(
                        "Failed to persist session {}: {}",
                        sess.session_id, e
                    ));
                }
            }
        }

        // Create migration record
        let migration = ServiceMigration {
            service_name: service_name.clone(),
            source_peer: source_peer.clone(),
            target_peer: target_peer_id.clone(),
            status: MigrationStatus::Pending,
            transfer_id: None,
            start_time: Self::current_timestamp(),
            completion_time: None,
        };

        let key = (source_peer, service_name);
        let mut migrations = self.migrations.write().await;
        migrations.insert(key, migration);

        Ok(target_peer_id)
    }

    /// Complete a migration
    pub async fn complete_migration(
        &self,
        source_peer: String,
        service_name: String,
        transfer_id: String,
    ) -> Result<(), String> {
        let key = (source_peer, service_name);
        let mut migrations = self.migrations.write().await;

        if let Some(migration) = migrations.get_mut(&key) {
            migration.status = MigrationStatus::Completed;
            migration.transfer_id = Some(transfer_id);
            migration.completion_time = Some(Self::current_timestamp());
            // Try to restore sessions for this service on this server (target)
            if let Err(e) = self
                .restore_sessions_for_service(&migration.service_name)
                .await
            {
                eprintln!(
                    "Warning: failed to restore sessions for {}: {}",
                    migration.service_name, e
                );
            }

            Ok(())
        } else {
            Err(format!("Migration not found: {:?}", key))
        }
    }

    /// Mark migration as failed
    pub async fn fail_migration(
        &self,
        source_peer: String,
        service_name: String,
        reason: String,
    ) -> Result<(), String> {
        let key = (source_peer, service_name);
        let mut migrations = self.migrations.write().await;

        if let Some(migration) = migrations.get_mut(&key) {
            migration.status = MigrationStatus::Failed(reason);
            migration.completion_time = Some(Self::current_timestamp());
            Ok(())
        } else {
            Err(format!("Migration not found: {:?}", key))
        }
    }

    /// Get migration status
    pub async fn get_migration_status(
        &self,
        source_peer: &str,
        service_name: &str,
    ) -> Option<MigrationStatus> {
        let migrations = self.migrations.read().await;
        migrations
            .get(&(source_peer.to_string(), service_name.to_string()))
            .map(|m| m.status.clone())
    }

    /// Get all active migrations
    pub async fn get_active_migrations(&self) -> Vec<ServiceMigration> {
        let migrations = self.migrations.read().await;
        migrations
            .values()
            .filter(|m| m.status == MigrationStatus::InProgress)
            .cloned()
            .collect()
    }

    /// Get failed peers awaiting migration
    pub async fn get_failed_peers(&self) -> Vec<FailedPeer> {
        self.failed_peers.read().await.clone()
    }

    /// Clear failed peer record after migration complete
    pub async fn clear_failed_peer(&self, peer_id: &str) -> bool {
        let mut peers = self.failed_peers.write().await;
        let initial_len = peers.len();
        peers.retain(|p| p.peer_id != peer_id);
        peers.len() < initial_len
    }

    /// Select best target peer for migration
    fn select_migration_target(
        &self,
        _registry: &PeerRegistry,
        exclude_peer: &str,
        peers: &[PeerInfo],
    ) -> Option<String> {
        let healthy_peers: Vec<_> = peers
            .iter()
            .filter(|p| p.status == PeerStatus::Healthy && p.server_id != exclude_peer)
            .collect();

        if healthy_peers.is_empty() {
            return None;
        }

        match self.config.preferred_target_selection {
            TargetSelection::LeastLoaded => {
                // Select peer with fewest services (in real impl, check actual service count)
                healthy_peers.first().map(|p| p.server_id.clone())
            }
            TargetSelection::FirstAvailable => healthy_peers.first().map(|p| p.server_id.clone()),
            TargetSelection::RoundRobin => healthy_peers.first().map(|p| p.server_id.clone()),
            TargetSelection::Specific(ref peer_id) => {
                if healthy_peers.iter().any(|p| p.server_id == *peer_id) {
                    Some(peer_id.clone())
                } else {
                    healthy_peers.first().map(|p| p.server_id.clone())
                }
            }
        }
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::clustering::peer::PeerInfo;
    use crate::server::clustering::registry::{PeerConfig, PeerRegistry};

    // Helper function to create a mock replication coordinator
    fn create_mock_replication() -> Arc<RwLock<ReplicationCoordinator>> {
        use crate::server::clustering::connection::ConnectionPool;
        use crate::server::clustering::protocol::ProtocolHandler;

        let pool = Arc::new(ConnectionPool::new());
        let protocol = Arc::new(ProtocolHandler::new("server1".to_string(), pool));
        Arc::new(RwLock::new(ReplicationCoordinator::new(
            "server1".to_string(),
            protocol,
        )))
    }

    #[tokio::test]
    async fn test_failover_manager_creation() {
        let config = PeerConfig::default();
        let registry = Arc::new(RwLock::new(PeerRegistry::new(config)));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        assert_eq!(manager.config.auto_migrate_services, true);
    }

    #[tokio::test]
    async fn test_detect_failed_peer() {
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);
        let mut peer = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer.status = PeerStatus::Down;
        registry.add_peer(peer).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        let failed = manager.check_peer_health().await;
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].peer_id, "peer1");
        assert_eq!(failed[0].reason, FailureReason::HeartbeatTimeout);
    }

    #[tokio::test]
    async fn test_migrate_service_no_healthy_peers() {
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);
        let mut peer = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer.status = PeerStatus::Down;
        registry.add_peer(peer).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        let result = manager
            .migrate_service("service1".to_string(), "peer1".to_string(), None)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No healthy peers available"));
    }

    #[tokio::test]
    async fn test_migrate_service_success() {
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);

        let mut peer1 = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer1.status = PeerStatus::Down;
        registry.add_peer(peer1).await;

        let peer2 = PeerInfo::new("peer2", "127.0.0.1:5002");
        registry.add_peer(peer2).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        let result = manager
            .migrate_service("service1".to_string(), "peer1".to_string(), None)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "peer2");
    }

    #[tokio::test]
    async fn test_migration_completion() {
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);

        // Add peer1 as down
        let mut peer1 = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer1.status = PeerStatus::Down;
        registry.add_peer(peer1).await;

        // Add peer2 as healthy
        let peer2 = PeerInfo::new("peer2", "127.0.0.1:5002");
        registry.add_peer(peer2).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        // Create migration
        let migrate_result = manager
            .migrate_service("service1".to_string(), "peer1".to_string(), None)
            .await;

        assert!(migrate_result.is_ok());

        // Complete it
        let result = manager
            .complete_migration(
                "peer1".to_string(),
                "service1".to_string(),
                "transfer_1".to_string(),
            )
            .await;

        assert!(result.is_ok());

        let status = manager.get_migration_status("peer1", "service1").await;

        assert_eq!(status, Some(MigrationStatus::Completed));
    }

    #[tokio::test]
    async fn test_migration_failure() {
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);

        // Add peer1 as down
        let mut peer1 = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer1.status = PeerStatus::Down;
        registry.add_peer(peer1).await;

        // Add peer2 as healthy
        let peer2 = PeerInfo::new("peer2", "127.0.0.1:5002");
        registry.add_peer(peer2).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        // Create migration
        let migrate_result = manager
            .migrate_service("service1".to_string(), "peer1".to_string(), None)
            .await;

        assert!(migrate_result.is_ok());

        // Fail it
        let result = manager
            .fail_migration(
                "peer1".to_string(),
                "service1".to_string(),
                "Connection timeout".to_string(),
            )
            .await;

        assert!(result.is_ok());

        let status = manager.get_migration_status("peer1", "service1").await;

        assert!(matches!(status, Some(MigrationStatus::Failed(_))));
    }

    #[tokio::test]
    async fn test_clear_failed_peer() {
        let config = PeerConfig::default();
        let registry = Arc::new(RwLock::new(PeerRegistry::new(config)));
        let replication = create_mock_replication();

        let manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        // Manually add a failed peer for testing
        let mut peers = manager.failed_peers.write().await;
        peers.push(FailedPeer {
            peer_id: "peer1".to_string(),
            server_addr: "127.0.0.1:5001".to_string(),
            services: vec![],
            failure_time: 0,
            reason: FailureReason::HeartbeatTimeout,
        });
        drop(peers);

        let before = manager.get_failed_peers().await;
        assert_eq!(before.len(), 1);

        manager.clear_failed_peer("peer1").await;

        let after = manager.get_failed_peers().await;
        assert_eq!(after.len(), 0);
    }

    #[tokio::test]
    async fn test_target_selection_least_loaded() {
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);

        // Add peer1 as down
        let mut peer1 = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer1.status = PeerStatus::Down;
        registry.add_peer(peer1).await;

        // Add peer2 as healthy
        let peer2 = PeerInfo::new("peer2", "127.0.0.1:5002");
        registry.add_peer(peer2).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let failover_config = FailoverConfig {
            preferred_target_selection: TargetSelection::LeastLoaded,
            ..Default::default()
        };

        let manager = FailoverManager::new(failover_config, registry, replication);

        let result = manager
            .migrate_service("service1".to_string(), "peer1".to_string(), None)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "peer2");
    }

    #[tokio::test]
    async fn test_migrate_persists_sessions_and_target_can_load() {
        use std::collections::HashMap;
        use tempfile::tempdir;

        // Prepare session store with a session referencing the service
        let dir = tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf()).await.unwrap();

        let session = SessionData {
            session_id: "sx".to_string(),
            client_id: "cx".to_string(),
            tenant: "t1".to_string(),
            services: vec!["serviceX".to_string()],
            metadata: HashMap::new(),
            last_activity: 0,
            version: 1,
            state: vec![1, 2, 3],
        };

        store.create_session(session.clone()).await.unwrap();

        // Set up registry with a down source and a healthy target
        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);

        let mut peer1 = PeerInfo::new("peer1", "127.0.0.1:5001");
        peer1.status = PeerStatus::Down;
        registry.add_peer(peer1).await;

        let peer2 = PeerInfo::new("peer2", "127.0.0.1:5002");
        registry.add_peer(peer2).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let mut manager = FailoverManager::new(FailoverConfig::default(), registry, replication);

        // Attach session store to manager
        manager.set_session_store(Arc::new(store));

        // Trigger migration
        let result = manager
            .migrate_service("serviceX".to_string(), "peer1".to_string(), None)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "peer2");

        // Simulate target server loading persisted sessions from the same directory
        let target_store = SessionStore::new(dir.path().to_path_buf()).await.unwrap();
        let loaded = target_store.get_session("sx").await.unwrap();
        assert_eq!(loaded, session);
    }

    #[tokio::test]
    async fn test_multi_server_failover_with_session_restore() {
        use crate::server::session_manager::SessionManager;
        use std::collections::HashMap;
        use tempfile::tempdir;

        // === Setup: Shared directory for persisting sessions ===
        let shared_dir = tempdir().unwrap();

        // === Source Server (server1) Setup ===
        let source_store = SessionStore::new(shared_dir.path().to_path_buf())
            .await
            .unwrap();

        // Create a client session on server1
        let source_session = SessionData {
            session_id: "s_multi_1".to_string(),
            client_id: "client_multi_1".to_string(),
            tenant: "tenant_prod".to_string(),
            services: vec!["config_service".to_string()],
            metadata: {
                let mut m = HashMap::new();
                m.insert("role".to_string(), "admin".to_string());
                m
            },
            last_activity: 1000000,
            version: 2,
            state: vec![42, 43, 44],
        };

        source_store
            .create_session(source_session.clone())
            .await
            .unwrap();

        // === Registry Setup ===
        let peer_config = PeerConfig::default();
        let mut registry = PeerRegistry::new(peer_config);

        // Add server1 (marked as Down to simulate failure)
        let mut peer1 = PeerInfo::new("server1", "127.0.0.1:6001");
        peer1.status = PeerStatus::Down;
        registry.add_peer(peer1).await;

        // Add server2 (healthy)
        let peer2 = PeerInfo::new("server2", "127.0.0.1:6002");
        registry.add_peer(peer2).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        // === Source Server Failover Manager ===
        let mut source_failover = FailoverManager::new(
            FailoverConfig::default(),
            registry.clone(),
            replication.clone(),
        );
        source_failover.set_session_store(Arc::new(source_store));

        // === Trigger Migration on Source ===
        let migrate_result = source_failover
            .migrate_service(
                "config_service".to_string(),
                "server1".to_string(),
                Some("server2".to_string()),
            )
            .await;

        assert!(migrate_result.is_ok());
        let target = migrate_result.unwrap();
        assert_eq!(target, "server2");

        // === Complete Migration (Triggers Session Restore on Target) ===
        // Note: In real scenario, target server would call complete_migration
        let complete_result = source_failover
            .complete_migration(
                "server1".to_string(),
                "config_service".to_string(),
                "xfer_123".to_string(),
            )
            .await;
        assert!(complete_result.is_ok());

        // === Target Server (server2) Setup ===
        let target_store = SessionStore::new(shared_dir.path().to_path_buf())
            .await
            .unwrap();
        let target_session_manager = Arc::new(SessionManager::new());

        let mut target_failover =
            FailoverManager::new(FailoverConfig::default(), registry.clone(), replication);
        target_failover.set_session_store(Arc::new(target_store));
        target_failover.set_session_manager(target_session_manager.clone());

        // === Restore Sessions on Target ===
        let restore_result = target_failover
            .restore_sessions_for_service("config_service")
            .await;
        assert!(restore_result.is_ok());

        // === Verify Session Was Restored in Target's SessionManager ===
        let restored_session = target_session_manager.get_session("s_multi_1").await;
        assert!(restored_session.is_some());

        let restored = restored_session.unwrap();
        assert_eq!(restored.session_id, "s_multi_1");
        assert_eq!(restored.client_id, Some("client_multi_1".to_string()));
        assert_eq!(restored.tenant_id, Some("tenant_prod".to_string()));
        assert_eq!(restored.subscriptions.len(), 1);
        assert!(restored.subscriptions.contains("config_service"));
    }

    #[tokio::test]
    async fn test_restore_sessions_for_service_helper() {
        use std::collections::HashMap;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf()).await.unwrap();

        let session = SessionData {
            session_id: "sr".to_string(),
            client_id: "cr".to_string(),
            tenant: "t".to_string(),
            services: vec!["svcR".to_string()],
            metadata: HashMap::new(),
            last_activity: 0,
            version: 1,
            state: vec![],
        };

        store.create_session(session.clone()).await.unwrap();

        let config = PeerConfig::default();
        let mut registry = PeerRegistry::new(config);

        let peer = PeerInfo::new("peerA", "127.0.0.1:5001");
        registry.add_peer(peer).await;

        let registry = Arc::new(RwLock::new(registry));
        let replication = create_mock_replication();

        let mut manager = FailoverManager::new(FailoverConfig::default(), registry, replication);
        manager.set_session_store(Arc::new(store));

        // Should succeed and load sessions into the store's in-memory cache
        let res = manager.restore_sessions_for_service("svcR").await;
        assert!(res.is_ok());
    }
}
