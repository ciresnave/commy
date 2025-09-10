#[cfg(all(test, feature = "network"))]
mod tests {
    use super::*;
    use crate::manager::{
        core::ManagerConfig, ConnectionSide, CreationPolicy, Directionality, ExistencePolicy,
        MessagePattern, PerformanceRequirements, Permission, SerializationFormat,
        SharedFileManager, SharedFileOperation, SharedFileRequest, Topology, TransportPreference,
    };
    use crate::tests::test_utils::{TestCleanupGuard, TestEnvironment};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_request_file_with_filename() {
        let test_env = TestEnvironment::new().expect("Failed to create test environment");
        let _cleanup_guard = TestCleanupGuard::new(|| {
            println!("🧹 Cleaning up test resources for test_request_file_with_filename");
        });

        let config = crate::tests::phase2_memory_mapping_test::create_test_config(&test_env);
        let manager = SharedFileManager::new(config)
            .await
            .expect("Failed to create SharedFileManager");

        let filename = "requested_test_file.mmap";

        let request = SharedFileRequest {
            identifier: "requested_file_test".to_string(),
            file_path: Some(PathBuf::from(filename)),
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: Default::default(),
            name: "requested_file_test".to_string(),
            description: Some("Requested filename test".to_string()),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: SerializationFormat::Binary,
            connection_side: ConnectionSide::Agnostic,
            creation_policy: CreationPolicy::Create,
            max_size_bytes: Some(1024),
            ttl_seconds: None,
            max_connections: Some(1),
            existence_policy: ExistencePolicy::CreateOrConnect,
            required_permissions: vec![Permission::Read, Permission::Write],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::PreferLocal,
            performance_requirements: PerformanceRequirements::default(),
            operation: SharedFileOperation::Create {
                path: PathBuf::from(filename),
                size: 1024,
                permissions: vec![Permission::Read, Permission::Write],
                initial_data: None,
            },
        };

        let response = manager
            .request_file(request, "test_token")
            .await
            .expect("Failed to request memory-mapped file");

        // Response path should exist on disk
        assert!(response.file_path.exists());

        // Active files should contain the file_id and mapped file
        let has_mapped = if let Some(info) = manager.active_files.get(&response.file_id) {
            info.value().mapped_file.is_some()
        } else {
            false
        };
        assert!(
            has_mapped,
            "Manager should have the mapped file in active_files"
        );

        println!(
            "✅ Requested-file allocation test passed: {:?}",
            response.file_path
        );
    }
}
