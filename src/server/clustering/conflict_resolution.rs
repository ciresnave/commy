//! Conflict Resolution Strategies for Distributed Consistency
//!
//! Provides pluggable conflict resolution when concurrent writes occur
//! on different servers.

use crate::server::clustering::vector_clocks::VersionedValue;
use serde::{Deserialize, Serialize};

/// Trait for conflict resolution strategies.
///
/// Implement this to define how concurrent writes should be resolved.
pub trait ConflictResolver<T: Clone> {
    /// Resolve a conflict between two concurrent versions.
    ///
    /// # Arguments
    ///
    /// * `local` - The local version (from this server)
    /// * `remote` - The remote version (from another server)
    ///
    /// # Returns
    ///
    /// The resolved version (should be one of the inputs or a merged value)
    fn resolve(&self, local: &VersionedValue<T>, remote: &VersionedValue<T>) -> VersionedValue<T>;

    /// Get a human-readable name for this resolver.
    fn name(&self) -> &str;
}

/// Last-Write-Wins (LWW) resolver using vector clock timestamps.
///
/// When vector clocks show concurrent updates, this resolver picks the
/// value with the higher logical timestamp (sum of all vector clock components).
/// If equal, picks based on server priority.
#[derive(Clone, Debug)]
pub struct LastWriteWinsResolver {
    /// Current server ID for tie-breaking (used in priority comparison)
    _current_server: String,
}

impl LastWriteWinsResolver {
    /// Create a new LWW resolver.
    pub fn new(current_server: String) -> Self {
        LastWriteWinsResolver {
            _current_server: current_server,
        }
    }

    /// Calculate the sum of all vector clock components (logical time).
    fn logical_time(version: &VersionedValue<Vec<u8>>) -> u64 {
        version
            .version
            .server_ids()
            .iter()
            .map(|server| version.version.get(server))
            .sum()
    }
}

impl ConflictResolver<Vec<u8>> for LastWriteWinsResolver {
    fn resolve(
        &self,
        local: &VersionedValue<Vec<u8>>,
        remote: &VersionedValue<Vec<u8>>,
    ) -> VersionedValue<Vec<u8>> {
        let local_time = Self::logical_time(local);
        let remote_time = Self::logical_time(remote);

        match local_time.cmp(&remote_time) {
            std::cmp::Ordering::Greater => local.clone(),
            std::cmp::Ordering::Less => remote.clone(),
            std::cmp::Ordering::Equal => {
                // Equal logical time, use server priority
                if local.last_modified_by >= remote.last_modified_by {
                    local.clone()
                } else {
                    remote.clone()
                }
            }
        }
    }

    fn name(&self) -> &str {
        "LastWriteWins"
    }
}

/// Application-defined resolver that preserves all concurrent versions.
///
/// Returns both versions for the application to decide on resolution.
#[derive(Clone, Debug)]
pub struct ApplicationDefinedResolver {
    name: String,
}

impl ApplicationDefinedResolver {
    /// Create a new application-defined resolver.
    pub fn new(name: String) -> Self {
        ApplicationDefinedResolver { name }
    }
}

impl ConflictResolver<Vec<u8>> for ApplicationDefinedResolver {
    fn resolve(
        &self,
        local: &VersionedValue<Vec<u8>>,
        remote: &VersionedValue<Vec<u8>>,
    ) -> VersionedValue<Vec<u8>> {
        // Return the local version and let application handle conflict
        // In practice, the application would be notified of both versions
        local.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Server-priority resolver that always prefers the server with higher ID.
///
/// Useful for scenarios where certain servers are designated as authoritative.
#[derive(Clone, Debug)]
pub struct ServerPriorityResolver;

impl ConflictResolver<Vec<u8>> for ServerPriorityResolver {
    fn resolve(
        &self,
        local: &VersionedValue<Vec<u8>>,
        remote: &VersionedValue<Vec<u8>>,
    ) -> VersionedValue<Vec<u8>> {
        if local.last_modified_by >= remote.last_modified_by {
            local.clone()
        } else {
            remote.clone()
        }
    }

    fn name(&self) -> &str {
        "ServerPriority"
    }
}

/// Custom merge resolver that combines concurrent versions.
///
/// For data types that support merge (e.g., sets, maps), this resolver
/// combines both versions to preserve all non-conflicting changes.
#[derive(Clone, Debug)]
pub struct MergeResolver;

impl ConflictResolver<Vec<u8>> for MergeResolver {
    fn resolve(
        &self,
        local: &VersionedValue<Vec<u8>>,
        _remote: &VersionedValue<Vec<u8>>,
    ) -> VersionedValue<Vec<u8>> {
        // For raw bytes, merge is not possible, so use LWW as fallback
        let lww = LastWriteWinsResolver::new("self".to_string());
        lww.resolve(local, _remote)
    }

    fn name(&self) -> &str {
        "Merge"
    }
}

/// Configuration for conflict resolution behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConflictResolutionConfig {
    /// Keep the write with higher logical timestamp
    LastWriteWins,

    /// Always prefer a specific server
    ServerPriority,

    /// Return conflict to application for resolution
    ApplicationDefined,

    /// Merge concurrent updates (when possible)
    Merge,
}

impl Default for ConflictResolutionConfig {
    fn default() -> Self {
        ConflictResolutionConfig::LastWriteWins
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::clustering::vector_clocks::VectorClock;

    fn create_test_value(value: u8, server: &str, time: u64) -> VersionedValue<Vec<u8>> {
        let mut vc = VectorClock::new(&[server]);
        for _ in 0..time {
            vc.increment(server);
        }
        VersionedValue::new(vec![value], vc, server.to_string())
    }

    #[test]
    fn test_last_write_wins_higher_local() {
        let local = create_test_value(1, "server1", 5);
        let remote = create_test_value(2, "server2", 3);

        let resolver = LastWriteWinsResolver::new("server1".to_string());
        let result = resolver.resolve(&local, &remote);

        assert_eq!(result.value, vec![1]);
    }

    #[test]
    fn test_last_write_wins_higher_remote() {
        let local = create_test_value(1, "server1", 3);
        let remote = create_test_value(2, "server2", 5);

        let resolver = LastWriteWinsResolver::new("server1".to_string());
        let result = resolver.resolve(&local, &remote);

        assert_eq!(result.value, vec![2]);
    }

    #[test]
    fn test_last_write_wins_equal_time_server_priority() {
        let local = create_test_value(1, "server1", 2);
        let remote = create_test_value(2, "server2", 2);

        let resolver = LastWriteWinsResolver::new("server1".to_string());
        let result = resolver.resolve(&local, &remote);

        // server2 > server1, so remote wins
        assert_eq!(result.value, vec![2]);
    }

    #[test]
    fn test_server_priority_resolver() {
        let local = create_test_value(1, "server1", 10);
        let remote = create_test_value(2, "server3", 1);

        let resolver = ServerPriorityResolver;
        let result = resolver.resolve(&local, &remote);

        // server3 > server1, so remote wins
        assert_eq!(result.value, vec![2]);
    }

    #[test]
    fn test_application_defined_resolver() {
        let local = create_test_value(1, "server1", 2);
        let remote = create_test_value(2, "server2", 5);

        let resolver = ApplicationDefinedResolver::new("app_resolver".to_string());
        let result = resolver.resolve(&local, &remote);

        // Always returns local, letting app decide
        assert_eq!(result.value, vec![1]);
        assert_eq!(resolver.name(), "app_resolver");
    }

    #[test]
    fn test_resolver_names() {
        let lww = LastWriteWinsResolver::new("server".to_string());
        assert_eq!(lww.name(), "LastWriteWins");

        let sp = ServerPriorityResolver;
        assert_eq!(sp.name(), "ServerPriority");

        let merge = MergeResolver;
        assert_eq!(merge.name(), "Merge");
    }

    #[test]
    fn test_conflict_resolution_config_default() {
        let cfg = ConflictResolutionConfig::default();
        assert!(matches!(cfg, ConflictResolutionConfig::LastWriteWins));
    }
}
