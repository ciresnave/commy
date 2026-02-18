//! Comprehensive Clustering Integration Tests
//!
//! These tests simulate realistic multi-server scenarios including:
//! - 3+ server clusters with replication
//! - Concurrent writes and conflict resolution
//! - Causality chains and eventual consistency
//! - Conflict detection and deterministic resolution
//! - Network partition recovery

#[cfg(test)]
mod clustering_comprehensive_tests {
    use commy::server::clustering::{
        ConflictInfo, VariableVersion, VectorClock, VersionedServiceMetadata,
    };

    /// Simulates a 3-server cluster configuration
    struct ThreeServerCluster {
        server1_metadata: VersionedServiceMetadata,
        server2_metadata: VersionedServiceMetadata,
        server3_metadata: VersionedServiceMetadata,
    }

    impl ThreeServerCluster {
        fn new(service_name: &str) -> Self {
            let servers = &["s1", "s2", "s3"];
            ThreeServerCluster {
                server1_metadata: VersionedServiceMetadata::new(
                    service_name.to_string(),
                    servers,
                    "s1".to_string(),
                ),
                server2_metadata: VersionedServiceMetadata::new(
                    service_name.to_string(),
                    servers,
                    "s2".to_string(),
                ),
                server3_metadata: VersionedServiceMetadata::new(
                    service_name.to_string(),
                    servers,
                    "s3".to_string(),
                ),
            }
        }

        /// Simulate Server1 writing and replicating to Server2
        fn server1_writes_to_s2(&mut self, var_name: &str, seq: u64) {
            self.server1_metadata
                .record_write(var_name.to_string(), "s1", seq);
            self.server2_metadata.merge_metadata(&self.server1_metadata);
        }

        /// Simulate Server2 writing and replicating to Server3
        fn server2_writes_to_s3(&mut self, var_name: &str, seq: u64) {
            self.server2_metadata
                .record_write(var_name.to_string(), "s2", seq);
            self.server3_metadata.merge_metadata(&self.server2_metadata);
        }

        /// Full sync: All servers converge on state
        fn converge_all_servers(&mut self) {
            let s1_state = self.server1_metadata.clone();
            let s2_state = self.server2_metadata.clone();
            let s3_state = self.server3_metadata.clone();

            self.server1_metadata.merge_metadata(&s2_state);
            self.server1_metadata.merge_metadata(&s3_state);

            self.server2_metadata.merge_metadata(&s1_state);
            self.server2_metadata.merge_metadata(&s3_state);

            self.server3_metadata.merge_metadata(&s1_state);
            self.server3_metadata.merge_metadata(&s2_state);
        }

        /// Check if all servers have same variables
        fn is_consistent(&self) -> bool {
            self.server1_metadata
                .variable_versions
                .keys()
                .collect::<std::collections::HashSet<_>>()
                == self
                    .server2_metadata
                    .variable_versions
                    .keys()
                    .collect::<std::collections::HashSet<_>>()
                && self
                    .server2_metadata
                    .variable_versions
                    .keys()
                    .collect::<std::collections::HashSet<_>>()
                    == self
                        .server3_metadata
                        .variable_versions
                        .keys()
                        .collect::<std::collections::HashSet<_>>()
        }
    }

    #[test]
    fn test_3server_cluster_linear_replication() {
        // Test: Server1 -> Server2 -> Server3 replication chain
        let mut cluster = ThreeServerCluster::new("config");

        // Server1 writes config_version
        cluster.server1_writes_to_s2("config_version", 1);
        assert!(
            cluster
                .server2_metadata
                .variable_versions
                .contains_key("config_version")
        );

        // Server2 replicates to Server3
        cluster.server2_writes_to_s3("db_url", 1);
        assert!(
            cluster
                .server3_metadata
                .variable_versions
                .contains_key("db_url")
        );

        // Full convergence
        cluster.converge_all_servers();
        assert_eq!(cluster.server1_metadata.variable_versions.len(), 2);
        assert_eq!(cluster.server2_metadata.variable_versions.len(), 2);
        assert_eq!(cluster.server3_metadata.variable_versions.len(), 2);
    }

    #[test]
    fn test_3server_cluster_parallel_writes() {
        // Test: All servers write independently, then converge
        let mut cluster = ThreeServerCluster::new("app_state");

        // Each server writes independently
        cluster
            .server1_metadata
            .record_write("var1".to_string(), "s1", 1);
        cluster
            .server2_metadata
            .record_write("var2".to_string(), "s2", 1);
        cluster
            .server3_metadata
            .record_write("var3".to_string(), "s3", 1);

        // Before convergence, each only sees its own write
        assert_eq!(cluster.server1_metadata.variable_versions.len(), 1);
        assert_eq!(cluster.server2_metadata.variable_versions.len(), 1);
        assert_eq!(cluster.server3_metadata.variable_versions.len(), 1);

        // Full convergence via gossip
        cluster.converge_all_servers();

        // Now all see all writes
        assert!(cluster.is_consistent());
        assert_eq!(cluster.server1_metadata.variable_versions.len(), 3);
        assert_eq!(cluster.server2_metadata.variable_versions.len(), 3);
        assert_eq!(cluster.server3_metadata.variable_versions.len(), 3);
    }

    #[test]
    fn test_3server_concurrent_writes_same_variable() {
        // Test: All servers write to same variable, detect conflicts
        let mut cluster = ThreeServerCluster::new("shared_config");

        // All servers independently write to cache_ttl (concurrent writes)
        cluster
            .server1_metadata
            .record_write("cache_ttl".to_string(), "s1", 1);
        cluster
            .server2_metadata
            .record_write("cache_ttl".to_string(), "s2", 1);
        cluster
            .server3_metadata
            .record_write("cache_ttl".to_string(), "s3", 1);

        // Server2 detects conflicts when receiving from Server1 and Server3
        let conflicts_1_3 = cluster
            .server2_metadata
            .detect_conflicts(&cluster.server3_metadata);
        assert!(conflicts_1_3.len() > 0);

        // Conflict should be on cache_ttl
        assert!(conflicts_1_3.iter().any(|c| c.variable_name == "cache_ttl"));

        // Merge to resolve conflicts (LWW)
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server3_metadata);

        // After merge, should have one value (conflict resolved)
        assert!(
            cluster
                .server2_metadata
                .variable_versions
                .contains_key("cache_ttl")
        );
    }

    #[test]
    fn test_cascading_replication() {
        // Test: Write propagates through multi-hop path
        let mut cluster = ThreeServerCluster::new("cascading");

        // Server1 writes, replicates to Server2
        cluster
            .server1_metadata
            .record_write("step1".to_string(), "s1", 1);
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Server2 writes and replicates to Server3
        cluster
            .server2_metadata
            .record_write("step2".to_string(), "s2", 2);
        cluster
            .server3_metadata
            .merge_metadata(&cluster.server2_metadata);

        // Server3 should now see both Server1's and Server2's writes
        assert!(
            cluster
                .server3_metadata
                .variable_versions
                .contains_key("step1")
        );
        assert!(
            cluster
                .server3_metadata
                .variable_versions
                .contains_key("step2")
        );

        // Verify causality: step1 happened before step2
        let step1_ver = &cluster.server3_metadata.variable_versions["step1"];
        let step2_ver = &cluster.server3_metadata.variable_versions["step2"];
        assert_eq!(step1_ver.happens_before(step2_ver), Some(true));
    }

    #[test]
    fn test_server_failure_detection() {
        // Test: Detecting missing updates from a failed server
        let mut cluster = ThreeServerCluster::new("failure_detection");

        // Server1 writes and replicates to Server2
        cluster
            .server1_metadata
            .record_write("before_failure".to_string(), "s1", 1);
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Get checkpoint after Server2 last saw Server1
        let checkpoint_clock = cluster.server2_metadata.service_clock.clone();

        // Server1 continues (simulating Server2 being down)
        cluster
            .server1_metadata
            .record_write("after_failure1".to_string(), "s1", 2);
        cluster
            .server1_metadata
            .record_write("after_failure2".to_string(), "s1", 3);

        // Server2 comes back and asks "what's new?"
        let new_vars = cluster.server1_metadata.variables_after(&checkpoint_clock);

        // Should get the new updates, not the old ones
        assert!(new_vars.contains(&"after_failure1".to_string()));
        assert!(new_vars.contains(&"after_failure2".to_string()));
        assert!(!new_vars.contains(&"before_failure".to_string()));
    }

    #[test]
    fn test_network_partition_recovery() {
        // Test: Cluster splits, then rejoins with conflict resolution
        let mut cluster = ThreeServerCluster::new("partition");

        // Partition 1: S1 and S2
        cluster
            .server1_metadata
            .record_write("partition1_var".to_string(), "s1", 1);
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Partition 2: S3 alone
        cluster
            .server3_metadata
            .record_write("partition2_var".to_string(), "s3", 1);

        // Verify partitions are separate
        assert!(
            !cluster
                .server2_metadata
                .variable_versions
                .contains_key("partition2_var")
        );
        assert!(
            !cluster
                .server3_metadata
                .variable_versions
                .contains_key("partition1_var")
        );

        // Partition heals: All converge
        cluster.converge_all_servers();

        // All servers should now have both variables
        assert!(
            cluster
                .server1_metadata
                .variable_versions
                .contains_key("partition1_var")
        );
        assert!(
            cluster
                .server1_metadata
                .variable_versions
                .contains_key("partition2_var")
        );
        assert!(
            cluster
                .server2_metadata
                .variable_versions
                .contains_key("partition1_var")
        );
        assert!(
            cluster
                .server2_metadata
                .variable_versions
                .contains_key("partition2_var")
        );
        assert!(
            cluster
                .server3_metadata
                .variable_versions
                .contains_key("partition1_var")
        );
        assert!(
            cluster
                .server3_metadata
                .variable_versions
                .contains_key("partition2_var")
        );
    }

    #[test]
    fn test_incremental_sync_after_failure() {
        // Test: Server rejoins cluster, syncs only new updates
        let mut cluster = ThreeServerCluster::new("recovery");

        // Initial state: all servers in sync
        cluster
            .server1_metadata
            .record_write("base_config".to_string(), "s1", 1);
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);
        cluster
            .server3_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Get checkpoint of Server2
        let checkpoint = cluster.server2_metadata.service_clock.clone();

        // Server1 and Server3 continue (Server2 down)
        cluster
            .server1_metadata
            .record_write("update1".to_string(), "s1", 2);
        cluster
            .server1_metadata
            .record_write("update2".to_string(), "s1", 3);
        cluster
            .server3_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Server2 comes back online, asks "what changed?"
        let new_updates = cluster.server1_metadata.variables_after(&checkpoint);

        // Should get only the new updates, not the base config
        assert_eq!(new_updates.len(), 2);
        assert!(new_updates.contains(&"update1".to_string()));
        assert!(new_updates.contains(&"update2".to_string()));
        assert!(!new_updates.contains(&"base_config".to_string()));
    }

    #[test]
    fn test_deterministic_conflict_resolution() {
        // Test: Same conflict always resolves the same way
        let mut metadata1 =
            VersionedServiceMetadata::new("test".to_string(), &["s1", "s2"], "s1".to_string());
        let mut metadata2 =
            VersionedServiceMetadata::new("test".to_string(), &["s1", "s2"], "s2".to_string());

        // Create identical concurrent writes
        metadata1.record_write("key".to_string(), "s1", 1);
        metadata2.record_write("key".to_string(), "s2", 1);

        // Get conflict
        let conflicts = metadata1.detect_conflicts(&metadata2);
        assert_eq!(conflicts.len(), 1);

        // Resolve conflict (should be deterministic)
        let winner1 = conflicts[0].lww_winner();

        // Create same scenario again
        let mut metadata1b =
            VersionedServiceMetadata::new("test".to_string(), &["s1", "s2"], "s1".to_string());
        let mut metadata2b =
            VersionedServiceMetadata::new("test".to_string(), &["s1", "s2"], "s2".to_string());

        metadata1b.record_write("key".to_string(), "s1", 1);
        metadata2b.record_write("key".to_string(), "s2", 1);

        let conflicts_b = metadata1b.detect_conflicts(&metadata2b);
        let winner2 = conflicts_b[0].lww_winner();

        // Both should have same winner
        assert_eq!(winner1.last_modified_by, winner2.last_modified_by);
    }

    #[test]
    fn test_bidirectional_replication() {
        // Test: Servers replicate updates to each other (gossip protocol)
        let mut cluster = ThreeServerCluster::new("gossip");

        // Round 1: Server1 writes
        cluster
            .server1_metadata
            .record_write("s1_var".to_string(), "s1", 1);

        // Server1 replicates to Server2
        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Server2 also writes
        cluster
            .server2_metadata
            .record_write("s2_var".to_string(), "s2", 1);

        // Server2 replicates back to Server1 (bidirectional)
        cluster
            .server1_metadata
            .merge_metadata(&cluster.server2_metadata);

        // Server1 should now have both variables
        assert_eq!(cluster.server1_metadata.variable_versions.len(), 2);
        assert!(
            cluster
                .server1_metadata
                .variable_versions
                .contains_key("s1_var")
        );
        assert!(
            cluster
                .server1_metadata
                .variable_versions
                .contains_key("s2_var")
        );

        // Server1 replicates to Server3
        cluster
            .server3_metadata
            .merge_metadata(&cluster.server1_metadata);

        // Server3 has both as well
        assert_eq!(cluster.server3_metadata.variable_versions.len(), 2);
    }

    #[test]
    fn test_high_concurrency_variable_count() {
        // Test: Cluster handles many concurrent variables
        let mut cluster = ThreeServerCluster::new("many_vars");

        // 100 concurrent writes from each server
        for i in 0..100 {
            cluster
                .server1_metadata
                .record_write(format!("s1_var_{}", i), "s1", i);
            cluster
                .server2_metadata
                .record_write(format!("s2_var_{}", i), "s2", i);
            cluster
                .server3_metadata
                .record_write(format!("s3_var_{}", i), "s3", i);
        }

        // Before convergence: each server sees 100 variables
        assert_eq!(cluster.server1_metadata.variable_versions.len(), 100);
        assert_eq!(cluster.server2_metadata.variable_versions.len(), 100);
        assert_eq!(cluster.server3_metadata.variable_versions.len(), 100);

        // Full convergence
        cluster.converge_all_servers();

        // After convergence: all see 300 variables (100 from each server)
        assert_eq!(cluster.server1_metadata.variable_versions.len(), 300);
        assert_eq!(cluster.server2_metadata.variable_versions.len(), 300);
        assert_eq!(cluster.server3_metadata.variable_versions.len(), 300);
    }

    #[test]
    fn test_split_brain_scenario() {
        // Test: Two servers in conflict, third acts as arbitrator
        let mut cluster = ThreeServerCluster::new("split_brain");

        // Server1 and Server2 split on a critical value
        cluster
            .server1_metadata
            .record_write("db_host".to_string(), "s1", 10);
        cluster
            .server2_metadata
            .record_write("db_host".to_string(), "s2", 10);

        // Server3 acts as arbitrator, sees both
        cluster
            .server3_metadata
            .merge_metadata(&cluster.server1_metadata);
        let s2_state = cluster.server2_metadata.clone();
        cluster.server3_metadata.merge_metadata(&s2_state);

        // Detects the conflict
        let conflicts = cluster
            .server3_metadata
            .detect_conflicts(&cluster.server1_metadata);
        assert!(conflicts.len() > 0);

        // Conflict has LWW winner
        let conflict = &conflicts[0];
        let winner = conflict.lww_winner();
        assert!(!winner.last_modified_by.is_empty());

        // Server3 broadcasts resolved value to both
        let s3_state = cluster.server3_metadata.clone();
        cluster.server1_metadata.merge_metadata(&s3_state);
        cluster.server2_metadata.merge_metadata(&s3_state);
    }

    #[test]
    fn test_vector_clock_properties_preserved() {
        // Test: Vector clock mathematical properties hold in cluster
        let mut cluster = ThreeServerCluster::new("vc_properties");

        // Create causality chain: S1 -> S2 -> S3
        cluster
            .server1_metadata
            .record_write("a".to_string(), "s1", 1);

        cluster
            .server2_metadata
            .merge_metadata(&cluster.server1_metadata);
        cluster
            .server2_metadata
            .record_write("b".to_string(), "s2", 1);

        cluster
            .server3_metadata
            .merge_metadata(&cluster.server2_metadata);
        cluster
            .server3_metadata
            .record_write("c".to_string(), "s3", 1);

        // Verify transitivity of causality
        let a = &cluster.server3_metadata.variable_versions["a"];
        let b = &cluster.server3_metadata.variable_versions["b"];
        let c = &cluster.server3_metadata.variable_versions["c"];

        // a -> b -> c
        assert_eq!(a.happens_before(b), Some(true));
        assert_eq!(b.happens_before(c), Some(true));

        // Transitivity: a -> c
        assert_eq!(a.happens_before(c), Some(true));

        // Verify causality is stable (idempotent)
        let a_check = &cluster.server3_metadata.variable_versions["a"];
        let c_check = &cluster.server3_metadata.variable_versions["c"];
        assert_eq!(a_check.happens_before(c_check), Some(true));
    }
}
