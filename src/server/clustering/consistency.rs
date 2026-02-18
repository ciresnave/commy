//! Consistency Integration for Multi-Server Coordination
//!
//! Integrates vector clocks and conflict resolution into service replication
//! and failover flows to ensure deterministic handling of concurrent writes.

use crate::server::clustering::vector_clocks::VectorClock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Versioned metadata about a service's variables
///
/// This tracks causality information for each variable to enable
/// deterministic conflict resolution during multi-server replication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedServiceMetadata {
    /// Service name
    pub service_name: String,

    /// Current vector clock for this service
    pub service_clock: VectorClock,

    /// Causality information for each variable (var_name -> VersionedValue metadata)
    pub variable_versions: HashMap<String, VariableVersion>,

    /// Which server is the source of this metadata
    pub source_server: String,

    /// Timestamp when this metadata was created (ms since epoch)
    pub created_at: u64,
}

/// Version information for a single variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableVersion {
    /// Variable name
    pub name: String,

    /// Vector clock when this variable was last modified
    pub last_modified_clock: VectorClock,

    /// Which server last modified this variable
    pub last_modified_by: String,

    /// Logical sequence number (for ordering within a server)
    pub sequence_number: u64,
}

impl VariableVersion {
    /// Create a new variable version record
    pub fn new(name: String, clock: VectorClock, server: String, sequence: u64) -> Self {
        VariableVersion {
            name,
            last_modified_clock: clock,
            last_modified_by: server,
            sequence_number: sequence,
        }
    }

    /// Check if this variable version happened before another
    pub fn happens_before(&self, other: &VariableVersion) -> Option<bool> {
        self.last_modified_clock
            .happens_before(&other.last_modified_clock)
    }

    /// Check if concurrent with another version
    pub fn is_concurrent(&self, other: &VariableVersion) -> bool {
        self.last_modified_clock
            .is_concurrent(&other.last_modified_clock)
    }
}

impl VersionedServiceMetadata {
    /// Create new versioned service metadata
    pub fn new(service_name: String, server_ids: &[&str], source_server: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        VersionedServiceMetadata {
            service_name,
            service_clock: VectorClock::new(server_ids),
            variable_versions: HashMap::new(),
            source_server,
            created_at: now,
        }
    }

    /// Record a variable write with causality information
    pub fn record_write(&mut self, var_name: String, server: &str, sequence: u64) {
        self.service_clock.increment(server);

        self.variable_versions.insert(
            var_name.clone(),
            VariableVersion::new(
                var_name,
                self.service_clock.clone(),
                server.to_string(),
                sequence,
            ),
        );
    }

    /// Merge with another service's metadata (for replication)
    pub fn merge_metadata(&mut self, other: &VersionedServiceMetadata) {
        // Merge service-level clock
        self.service_clock.merge(&other.service_clock);

        // Merge variable-level versions, keeping the latest per variable
        for (var_name, other_version) in &other.variable_versions {
            match self.variable_versions.get(other_version.name.as_str()) {
                Some(local_version) => {
                    // Keep the version that happened later
                    if let Some(true) = other_version.happens_before(local_version) {
                        // Local version happened after, keep it
                    } else {
                        // Other happened after or concurrent, update to other
                        self.variable_versions
                            .insert(var_name.clone(), other_version.clone());
                    }
                }
                None => {
                    // New variable, add it
                    self.variable_versions
                        .insert(var_name.clone(), other_version.clone());
                }
            }
        }
    }

    /// Check for conflicts between local and remote versions
    pub fn detect_conflicts(&self, other: &VersionedServiceMetadata) -> Vec<ConflictInfo> {
        let mut conflicts = Vec::new();

        for (var_name, local_version) in &self.variable_versions {
            if let Some(remote_version) = other.variable_versions.get(var_name) {
                if local_version.is_concurrent(remote_version) {
                    conflicts.push(ConflictInfo {
                        variable_name: var_name.clone(),
                        local_version: local_version.clone(),
                        remote_version: remote_version.clone(),
                    });
                }
            }
        }

        conflicts
    }

    /// Get the causality order of variables (for deterministic replay)
    pub fn causality_order(&self) -> Vec<(String, u64)> {
        let mut ordered: Vec<_> = self
            .variable_versions
            .iter()
            .map(|(name, version)| (name.clone(), version.sequence_number))
            .collect();

        // Sort by sequence number (stable ordering)
        ordered.sort_by_key(|(_name, seq)| *seq);
        ordered
    }

    /// Get all variables that happened after a specific clock
    pub fn variables_after(&self, clock: &VectorClock) -> Vec<String> {
        self.variable_versions
            .iter()
            .filter_map(|(name, version)| {
                if let Some(true) = version.last_modified_clock.happens_after(clock) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Represents a conflict detected during merge
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    /// Variable name where conflict occurred
    pub variable_name: String,

    /// Local version
    pub local_version: VariableVersion,

    /// Remote version
    pub remote_version: VariableVersion,
}

impl ConflictInfo {
    /// Get which version should win via Last-Write-Wins
    pub fn lww_winner(&self) -> &VariableVersion {
        // Compare logical timestamps (sum of vector clock)
        let local_sum: u64 = self
            .local_version
            .last_modified_clock
            .server_ids()
            .iter()
            .map(|sid| self.local_version.last_modified_clock.get(sid))
            .sum();

        let remote_sum: u64 = self
            .remote_version
            .last_modified_clock
            .server_ids()
            .iter()
            .map(|sid| self.remote_version.last_modified_clock.get(sid))
            .sum();

        if remote_sum > local_sum {
            &self.remote_version
        } else if local_sum > remote_sum {
            &self.local_version
        } else {
            // Equal sum, use server ID as tie-breaker
            if self.remote_version.last_modified_by >= self.local_version.last_modified_by {
                &self.remote_version
            } else {
                &self.local_version
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_versioned_service_metadata_creation() {
        let metadata = VersionedServiceMetadata::new(
            "test_service".to_string(),
            &["server1", "server2"],
            "server1".to_string(),
        );

        assert_eq!(metadata.service_name, "test_service");
        assert_eq!(metadata.source_server, "server1");
        assert_eq!(metadata.variable_versions.len(), 0);
    }

    #[test]
    fn test_record_write() {
        let mut metadata = VersionedServiceMetadata::new(
            "test_service".to_string(),
            &["server1", "server2"],
            "server1".to_string(),
        );

        metadata.record_write("var1".to_string(), "server1", 1);
        metadata.record_write("var2".to_string(), "server1", 2);

        assert_eq!(metadata.variable_versions.len(), 2);
        assert!(metadata.variable_versions.contains_key("var1"));
        assert!(metadata.variable_versions.contains_key("var2"));
    }

    #[test]
    fn test_merge_metadata() {
        let mut local =
            VersionedServiceMetadata::new("service".to_string(), &["s1", "s2"], "s1".to_string());
        let mut remote =
            VersionedServiceMetadata::new("service".to_string(), &["s1", "s2"], "s2".to_string());

        local.record_write("var1".to_string(), "s1", 1);
        remote.record_write("var2".to_string(), "s2", 1);

        local.merge_metadata(&remote);

        assert_eq!(local.variable_versions.len(), 2);
        assert!(local.variable_versions.contains_key("var1"));
        assert!(local.variable_versions.contains_key("var2"));
    }

    #[test]
    fn test_detect_conflicts() {
        let mut local =
            VersionedServiceMetadata::new("service".to_string(), &["s1", "s2"], "s1".to_string());
        let mut remote =
            VersionedServiceMetadata::new("service".to_string(), &["s1", "s2"], "s2".to_string());

        // Create concurrent writes to same variable
        local.record_write("var1".to_string(), "s1", 1);
        // Reset remote clock and write independently
        let mut remote_clock = VectorClock::new(&["s1", "s2"]);
        remote_clock.increment("s2");
        remote.variable_versions.insert(
            "var1".to_string(),
            VariableVersion::new("var1".to_string(), remote_clock, "s2".to_string(), 1),
        );

        let conflicts = local.detect_conflicts(&remote);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].variable_name, "var1");
    }

    #[test]
    fn test_causality_order() {
        let mut metadata =
            VersionedServiceMetadata::new("service".to_string(), &["s1"], "s1".to_string());

        metadata.record_write("var3".to_string(), "s1", 3);
        metadata.record_write("var1".to_string(), "s1", 1);
        metadata.record_write("var2".to_string(), "s1", 2);

        let order = metadata.causality_order();
        assert_eq!(order[0].0, "var1");
        assert_eq!(order[1].0, "var2");
        assert_eq!(order[2].0, "var3");
    }

    #[test]
    fn test_conflict_lww_winner() {
        let mut local_clock = VectorClock::new(&["s1", "s2"]);
        local_clock.increment("s1");
        local_clock.increment("s1");

        let mut remote_clock = VectorClock::new(&["s1", "s2"]);
        remote_clock.increment("s2");

        let conflict = ConflictInfo {
            variable_name: "var1".to_string(),
            local_version: VariableVersion::new(
                "var1".to_string(),
                local_clock,
                "s1".to_string(),
                1,
            ),
            remote_version: VariableVersion::new(
                "var1".to_string(),
                remote_clock,
                "s2".to_string(),
                1,
            ),
        };

        let winner = conflict.lww_winner();
        assert_eq!(winner.last_modified_by, "s1"); // Higher sum (2 vs 1)
    }

    #[test]
    fn test_variables_after_clock() {
        let mut metadata =
            VersionedServiceMetadata::new("service".to_string(), &["s1", "s2"], "s1".to_string());

        metadata.record_write("var1".to_string(), "s1", 1);
        metadata.record_write("var2".to_string(), "s1", 2);

        let earlier_clock = VectorClock::new(&["s1", "s2"]);
        let after = metadata.variables_after(&earlier_clock);

        assert_eq!(after.len(), 2); // Both variables happened after empty clock
    }
}
