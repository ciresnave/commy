//! Permission revocation protocol for COMMY
//! 
//! Handles revoking client permissions and migrating them to new service files
//! with optional honeypot detection.

use crate::protocol::{ClientSessionId, WssMessage};
use crate::auth::AuthError;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use chrono::Utc;
use std::collections::HashMap;

/// Errors that can occur during permission revocation
#[derive(Error, Debug)]
pub enum RevocationError {
    #[error("Client not found: {0}")]
    ClientNotFound(String),

    #[error("File operation failed: {0}")]
    FileError(String),

    #[error("Migration failed: {0}")]
    MigrationFailed(String),

    #[error("Auth error: {0}")]
    AuthError(#[from] AuthError),
}

pub type RevocationResult<T> = Result<T, RevocationError>;

/// Revocation reason
#[derive(Clone, Debug)]
pub enum RevocationReason {
    /// Client exceeded max failed login attempts
    MaxAttemptsExceeded,
    /// Token expired
    TokenExpired,
    /// Explicit admin revocation
    AdminRevocation,
    /// Security policy violation
    PolicyViolation,
    /// Suspicious activity detected
    SuspiciousActivity,
}

impl RevocationReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MaxAttemptsExceeded => "max_attempts_exceeded",
            Self::TokenExpired => "token_expired",
            Self::AdminRevocation => "admin_revocation",
            Self::PolicyViolation => "policy_violation",
            Self::SuspiciousActivity => "suspicious_activity",
        }
    }
}

/// Information about a revoked client
#[derive(Clone, Debug)]
pub struct RevokedClientInfo {
    /// Client ID
    pub client_id: String,
    /// Tenant ID
    pub tenant_id: String,
    /// Revocation reason
    pub reason: RevocationReason,
    /// Timestamp
    pub revoked_at: String,
    /// Optional detail message
    pub detail: String,
}

/// Permission revocation manager
/// 
/// Handles:
/// 1. Revoking client permissions
/// 2. Migrating clients to new service files
/// 3. Notifying other clients
/// 4. Optional honeypot detection (maintaining old file)
pub struct RevocationManager {
    /// Revoked clients (honeypot detection)
    revoked_clients: Arc<RwLock<HashMap<ClientSessionId, RevokedClientInfo>>>,
    /// Migration in progress
    migrations_in_progress: Arc<RwLock<HashMap<String, MigrationState>>>,
}

/// State of a file migration
#[derive(Clone, Debug)]
pub struct MigrationState {
    /// Service name
    pub service_name: String,
    /// Old file path
    pub old_path: PathBuf,
    /// New file path
    pub new_path: PathBuf,
    /// Clients to migrate
    pub clients_to_migrate: Vec<ClientSessionId>,
    /// Clients that have ack'd migration
    pub clients_migrated: Vec<ClientSessionId>,
    /// Timestamp
    pub started_at: String,
}

impl RevocationManager {
    /// Create a new revocation manager
    pub fn new() -> Self {
        Self {
            revoked_clients: Arc::new(RwLock::new(HashMap::new())),
            migrations_in_progress: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Revoke a client's permissions
    /// 
    /// # Steps:
    /// 1. Add client to revoked list (for honeypot)
    /// 2. Create migration to new service file
    /// 3. Notify other clients of file migration
    pub async fn revoke_permission(
        &self,
        client_id: String,
        tenant_id: String,
        reason: RevocationReason,
        detail: String,
    ) -> RevocationResult<()> {
        let revoked_info = RevokedClientInfo {
            client_id: client_id.clone(),
            tenant_id: tenant_id.clone(),
            reason,
            revoked_at: Utc::now().to_rfc3339(),
            detail,
        };

        let mut revoked = self.revoked_clients.write().await;
        revoked.insert(client_id, revoked_info);

        Ok(())
    }

    /// Start file migration for a service
    /// 
    /// Creates new file and tracks which clients need to migrate
    pub async fn start_migration(
        &self,
        service_name: String,
        old_path: PathBuf,
        new_path: PathBuf,
        clients_to_migrate: Vec<ClientSessionId>,
    ) -> RevocationResult<()> {
        let migration = MigrationState {
            service_name: service_name.clone(),
            old_path,
            new_path,
            clients_to_migrate,
            clients_migrated: Vec::new(),
            started_at: Utc::now().to_rfc3339(),
        };

        let mut migrations = self.migrations_in_progress.write().await;
        migrations.insert(service_name, migration);

        Ok(())
    }

    /// Record client migration acknowledgment
    pub async fn record_migration_ack(
        &self,
        service_name: &str,
        client_session_id: ClientSessionId,
    ) -> RevocationResult<()> {
        let mut migrations = self.migrations_in_progress.write().await;
        if let Some(migration) = migrations.get_mut(service_name) {
            if !migration.clients_migrated.contains(&client_session_id) {
                migration.clients_migrated.push(client_session_id);
            }
            Ok(())
        } else {
            Err(RevocationError::MigrationFailed(
                format!("No migration for service {}", service_name),
            ))
        }
    }

    /// Check if migration is complete (all clients ack'd)
    pub async fn is_migration_complete(&self, service_name: &str) -> RevocationResult<bool> {
        let migrations = self.migrations_in_progress.read().await;
        if let Some(migration) = migrations.get(service_name) {
            Ok(migration.clients_migrated.len() == migration.clients_to_migrate.len())
        } else {
            Err(RevocationError::MigrationFailed(
                format!("No migration for service {}", service_name),
            ))
        }
    }

    /// Finalize migration and cleanup old file
    /// 
    /// # Optional: Keep old file as honeypot
    pub async fn finalize_migration(
        &self,
        service_name: &str,
        keep_honeypot: bool,
    ) -> RevocationResult<()> {
        let mut migrations = self.migrations_in_progress.write().await;
        if let Some(_migration) = migrations.remove(service_name) {
            // In production, would delete old file or move to honeypot directory
            if !keep_honeypot {
                // TODO: Delete migration.old_path
            } else {
                // TODO: Move to honeypot directory with detection logic
            }
            Ok(())
        } else {
            Err(RevocationError::MigrationFailed(
                format!("No migration for service {}", service_name),
            ))
        }
    }

    /// Generate migration notification message
    pub fn create_migration_message(
        old_path: &Path,
        new_path: &Path,
        service_name: &str,
        reason: &str,
    ) -> WssMessage {
        WssMessage::FileMigration {
            old_service_path: old_path.to_string_lossy().to_string(),
            new_service_path: new_path.to_string_lossy().to_string(),
            service_name: service_name.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Generate permission revocation message
    pub fn create_revocation_message(reason: &str, detail: &str) -> WssMessage {
        WssMessage::PermissionRevoked {
            reason: reason.to_string(),
            detail: detail.to_string(),
        }
    }

    /// Check if client is revoked (honeypot detection)
    pub async fn is_client_revoked(&self, client_id: &str) -> bool {
        let revoked = self.revoked_clients.read().await;
        revoked.iter().any(|(_, info)| info.client_id == client_id)
    }

    /// Get revocation info for honeypot analysis
    pub async fn get_revoked_client_info(&self, client_id: &str) -> Option<RevokedClientInfo> {
        let revoked = self.revoked_clients.read().await;
        revoked
            .iter()
            .find(|(_, info)| info.client_id == client_id)
            .map(|(_, info)| info.clone())
    }

    /// Get all active migrations
    pub async fn get_migrations(&self) -> Vec<MigrationState> {
        let migrations = self.migrations_in_progress.read().await;
        migrations.values().cloned().collect()
    }

    /// Cleanup old revoked client entries (optional retention)
    pub async fn cleanup_old_revocations(&self, max_entries: usize) {
        let mut revoked = self.revoked_clients.write().await;
        if revoked.len() > max_entries {
            // Keep only most recent entries
            let to_remove = revoked.len() - max_entries;
            let keys: Vec<_> = revoked.keys().take(to_remove).cloned().collect();
            for key in keys {
                revoked.remove(&key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_revoke_permission() {
        let manager = RevocationManager::new();

        manager
            .revoke_permission(
                "client1".to_string(),
                "tenant1".to_string(),
                RevocationReason::AdminRevocation,
                "Test revocation".to_string(),
            )
            .await
            .unwrap();

        assert!(manager.is_client_revoked("client1").await);
    }

    #[tokio::test]
    async fn test_start_migration() {
        let manager = RevocationManager::new();
        let old_path = PathBuf::from("/tmp/old.mem");
        let new_path = PathBuf::from("/tmp/new.mem");

        manager
            .start_migration(
                "service1".to_string(),
                old_path,
                new_path,
                vec!["client1".to_string()],
            )
            .await
            .unwrap();

        let migrations = manager.get_migrations().await;
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].service_name, "service1");
    }

    #[tokio::test]
    async fn test_migration_completion() {
        let manager = RevocationManager::new();
        let clients = vec!["client1".to_string(), "client2".to_string()];

        manager
            .start_migration(
                "service1".to_string(),
                PathBuf::from("/tmp/old.mem"),
                PathBuf::from("/tmp/new.mem"),
                clients.clone(),
            )
            .await
            .unwrap();

        // Record one client ack
        manager
            .record_migration_ack("service1", clients[0].clone())
            .await
            .unwrap();

        assert!(!manager.is_migration_complete("service1").await.unwrap());

        // Record second client ack
        manager
            .record_migration_ack("service1", clients[1].clone())
            .await
            .unwrap();

        assert!(manager.is_migration_complete("service1").await.unwrap());
    }
}
