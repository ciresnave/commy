//! Integration tests for consistency management in multi-server scenarios

#[cfg(test)]
mod consistency_integration_tests {
    use commy::server::clustering::{
        ConflictInfo, ConflictResolutionConfig, VariableVersion,
        VersionedServiceMetadata, VectorClock,
    };

    #[test]
    fn test_multi_server_write_causality() {
        // Simulate 3-server cluster where updates propagate
        let mut server1_metadata =
            VersionedServiceMetadata::new("config".to_string(), &["s1", "s2", "s3"], "s1".to_string());
        let mut server2_metadata =
            VersionedServiceMetadata::new("config".to_string(), &["s1", "s2", "s3"], "s2".to_string());
        let mut server3_metadata =
            VersionedServiceMetadata::new("config".to_string(), &["s1", "s2", "s3"], "s3".to_string());

        // Server1 writes "max_connections" = 100
        server1_metadata.record_write("max_connections".to_string(), "s1", 1);

        // Server1's write propagates to Server2
        server2_metadata.merge_metadata(&server1_metadata);
        // Server2 writes "timeout" = 5000
        server2_metadata.record_write("timeout".to_string(), "s2", 1);

        // Server2's state propagates to Server3
        server3_metadata.merge_metadata(&server2_metadata);
        // Server3 reads and verifies both variables exist in order
        assert!(server3_metadata.variable_versions.contains_key("max_connections"));
        assert!(server3_metadata.variable_versions.contains_key("timeout"));

        // Verify causality: max_connections happened before timeout
        let max_conn_version = &server3_metadata.variable_versions["max_connections"];
        let timeout_version = &server3_metadata.variable_versions["timeout"];

        assert_eq!(
            max_conn_version.happens_before(timeout_version),
            Some(true)
        );
    }

    #[test]
    fn test_concurrent_writes_conflict_detection() {
        // Simulate two servers writing to same variable independently
        let mut server1_metadata =
            VersionedServiceMetadata::new("cache".to_string(), &["s1", "s2"], "s1".to_string());
        let mut server2_metadata =
            VersionedServiceMetadata::new("cache".to_string(), &["s1", "s2"], "s2".to_string());

        // Both servers independently increment sequence number
        server1_metadata.record_write("seq_number".to_string(), "s1", 1);
        server2_metadata.record_write("seq_number".to_string(), "s2", 1);

        // When they merge, conflict is detected
        let conflicts = server1_metadata.detect_conflicts(&server2_metadata);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].variable_name, "seq_number");

        // Both versions are concurrent (neither happened before the other)
        assert!(conflicts[0]
            .local_version
            .is_concurrent(&conflicts[0].remote_version));
    }

    #[test]
    fn test_lww_resolution_on_conflict() {
        // Create conflict between two writes
        let mut clock1 = VectorClock::new(&["s1", "s2"]);
        clock1.increment("s1");
        clock1.increment("s1"); // Higher sum = 2

        let mut clock2 = VectorClock::new(&["s1", "s2"]);
        clock2.increment("s2"); // Lower sum = 1

        let conflict = ConflictInfo {
            variable_name: "cache_ttl".to_string(),
            local_version: VariableVersion::new(
                "cache_ttl".to_string(),
                clock1,
                "s1".to_string(),
                100,
            ),
            remote_version: VariableVersion::new(
                "cache_ttl".to_string(),
                clock2,
                "s2".to_string(),
                50,
            ),
        };

        // Last-Write-Wins chooses version with higher logical timestamp
        let winner = conflict.lww_winner();
        assert_eq!(winner.last_modified_by, "s1");
        assert_eq!(winner.sequence_number, 100);
    }

    #[test]
    fn test_deterministic_variable_ordering() {
        // Create metadata with variables written in random order
        let mut metadata =
            VersionedServiceMetadata::new("app_state".to_string(), &["s1"], "s1".to_string());

        // Write in random sequence
        metadata.record_write("auth_token".to_string(), "s1", 5);
        metadata.record_write("user_id".to_string(), "s1", 1);
        metadata.record_write("session_data".to_string(), "s1", 3);
        metadata.record_write("permissions".to_string(), "s1", 2);

        // Get causality order - should be sorted by sequence
        let order = metadata.causality_order();

        assert_eq!(order[0].0, "user_id");
        assert_eq!(order[1].0, "permissions");
        assert_eq!(order[2].0, "session_data");
        assert_eq!(order[3].0, "auth_token");
    }

    #[test]
    fn test_incremental_update_detection() {
        // Simulate Server1 having some state
        let mut server1_state =
            VersionedServiceMetadata::new("cache".to_string(), &["s1", "s2"], "s1".to_string());

        server1_state.record_write("key1".to_string(), "s1", 1);
        server1_state.record_write("key2".to_string(), "s1", 2);

        // Get server1's clock state
        let checkpoint_clock = server1_state.service_clock.clone();

        // Server1 makes more updates
        server1_state.record_write("key3".to_string(), "s1", 3);
        server1_state.record_write("key4".to_string(), "s1", 4);

        // Server2 can ask "what changed since checkpoint?"
        let new_variables = server1_state.variables_after(&checkpoint_clock);

        // Should include the new updates
        assert!(new_variables.contains(&"key3".to_string()));
        assert!(new_variables.contains(&"key4".to_string()));
        // Should NOT include old updates (they happened before checkpoint)
        assert!(!new_variables.contains(&"key1".to_string()));
        assert!(!new_variables.contains(&"key2".to_string()));
    }

    #[test]
    fn test_full_cluster_synchronization() {
        // Simulate 4-server cluster sync scenario
        let servers = vec!["s1", "s2", "s3", "s4"];

        // Initial state - each server has done some work
        let mut states: Vec<_> = servers
            .iter()
            .map(|server| {
                VersionedServiceMetadata::new(
                    "settings".to_string(),
                    servers.as_slice(),
                    server.to_string(),
                )
            })
            .collect();

        // Each server writes its own config
        for (i, state) in states.iter_mut().enumerate() {
            state.record_write(
                format!("config_{}", i),
                servers[i],
                (i + 1) as u64,
            );
        }

        // Simulate gossip: merge all states to one server
        let mut merged = states[0].clone();
        for state in &states[1..] {
            merged.merge_metadata(state);
        }

        // Verify all variables are present
        assert_eq!(merged.variable_versions.len(), 4);
        assert!(merged.variable_versions.contains_key("config_0"));
        assert!(merged.variable_versions.contains_key("config_1"));
        assert!(merged.variable_versions.contains_key("config_2"));
        assert!(merged.variable_versions.contains_key("config_3"));
    }

    #[test]
    fn test_conflict_resolution_config() {
        // Demonstrate different resolution strategies
        let _lww_config = ConflictResolutionConfig::LastWriteWins;
        let _sp_config = ConflictResolutionConfig::ServerPriority;
        let _app_config = ConflictResolutionConfig::ApplicationDefined;
        let _merge_config = ConflictResolutionConfig::Merge;

        // Services can choose their strategy based on needs
        let default_config = ConflictResolutionConfig::default();
        matches!(default_config, ConflictResolutionConfig::LastWriteWins);
    }

    #[test]
    fn test_server_priority_tiebreaker() {
        // When two writes have same logical time, server ID is tie-breaker
        let clock = VectorClock::new(&["alpha", "zeta"]);

        let conflict = ConflictInfo {
            variable_name: "priority_var".to_string(),
            local_version: VariableVersion::new(
                "priority_var".to_string(),
                clock.clone(),
                "alpha".to_string(),
                1,
            ),
            remote_version: VariableVersion::new(
                "priority_var".to_string(),
                clock,
                "zeta".to_string(),
                1,
            ),
        };

        // "zeta" > "alpha" lexicographically, so zeta wins
        let winner = conflict.lww_winner();
        assert_eq!(winner.last_modified_by, "zeta");
    }
}
