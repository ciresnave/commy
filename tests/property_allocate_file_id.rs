use commy::manager::core::{ManagerConfig, SharedFileManager};
use commy::manager::{
    ConnectionSide, CreationPolicy, Directionality, ExistencePolicy, MessagePattern,
    PerformanceRequirements, Permission, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportPreference,
};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::collections::HashSet;
use std::sync::Arc;

// This test generates varying numbers of concurrent requests and asserts
// that allocated file IDs are unique.
proptest! {
    #[test]
    fn unique_ids_for_concurrent_allocs(concurrent in 1usize..50usize) {
        // Use a small tokio runtime for the test.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // Run the async workload and return a TestCaseResult so proptest can
        // surface failures correctly.
        let res: Result<(), TestCaseError> = rt.block_on(async move {
            let config = ManagerConfig::default();
            let manager = SharedFileManager::new(config).await.expect("create manager");
            let manager = Arc::new(manager);

            // Spawn concurrent tasks to request files which will allocate file IDs
            let mut handles = Vec::new();
            for i in 0..concurrent {
                let mgr = manager.clone();
                let filename = format!("prop_file_{}_{}.mmap", i, concurrent);
                let req = SharedFileRequest {
                    identifier: format!("prop-{}-{}", i, concurrent),
                    name: format!("prop-file-{}", i),
                    description: Some("proptest-generated".to_string()),
                    pattern: MessagePattern::RequestResponse { timeout_ms: None, retry_count: None },
                    pattern_config: std::collections::HashMap::new(),
                    directionality: Directionality::ReadWrite,
                    topology: Topology::OneToOne,
                    serialization: SerializationFormat::Binary,
                    connection_side: ConnectionSide::Agnostic,
                    creation_policy: CreationPolicy::CreateIfNotExists,
                    existence_policy: ExistencePolicy::CreateOrConnect,
                    file_path: None,
                    max_size_bytes: Some(1024),
                    ttl_seconds: None,
                    max_connections: Some(1),
                    required_permissions: vec![Permission::Read, Permission::Write],
                    encryption_required: false,
                    auto_cleanup: true,
                    persist_after_disconnect: false,
                    transport_preference: TransportPreference::PreferLocal,
                    performance_requirements: PerformanceRequirements::default(),
                    operation: SharedFileOperation::Create {
                        path: std::path::PathBuf::from(&filename),
                        size: 1024,
                        initial_data: None,
                        permissions: vec![Permission::Read, Permission::Write],
                    },
                };

                let token = "proptest_token".to_string();
                handles.push(tokio::spawn(async move {
                    // request_file expects an auth token &str
                    let resp = mgr
                        .request_file(req, &token)
                        .await
                        .expect("request_file");
                    resp.file_id
                }));
            }

            // Collect results
            let mut ids = Vec::new();
            for h in handles {
                let id = h.await.map_err(|e| TestCaseError::fail(format!("task panicked: {}", e)))?;
                ids.push(id);
            }

            let set: HashSet<_> = ids.iter().cloned().collect();
            if set.len() != ids.len() {
                return Err(TestCaseError::fail("duplicate ids allocated"));
            }

            Ok(())
        });

        // Propagate any failure into proptest
        res.unwrap();
    }
}
