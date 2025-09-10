use commy::manager::core::ManagerConfig;
use commy::manager::{
    ConnectionSide, CreationPolicy, Directionality, ExistencePolicy, Permission,
    SerializationFormat, SharedFileManager, SharedFileOperation, SharedFileRequest, Topology,
};
use tempfile::tempdir;

#[tokio::test]
async fn empty_token_is_denied() {
    let temp = tempdir().unwrap();
    let mut cfg = ManagerConfig::default();
    cfg.files_directory = temp.path().to_path_buf();

    // Use a mock provider that rejects tokens for strict validation
    let mock = commy::manager::auth_provider::MockAuthProvider::new(false);
    let manager = SharedFileManager::new_with_provider(cfg, std::sync::Arc::new(mock))
        .await
        .expect("manager init");

    let req = SharedFileRequest {
        identifier: "test_empty_token".to_string(),
        name: "".to_string(),
        description: None,
        pattern: commy::manager::MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: std::collections::HashMap::new(),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::ProducerConsumer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOnly,
        file_path: None,
        max_size_bytes: Some(1024),
        ttl_seconds: None,
        max_connections: None,
        required_permissions: vec![Permission::Read],
        encryption_required: false,
        auto_cleanup: false,
        persist_after_disconnect: false,
        transport_preference: commy::manager::TransportPreference::AutoOptimize,
        performance_requirements: commy::manager::PerformanceRequirements::default(),
        operation: SharedFileOperation::Create {
            path: std::path::PathBuf::from("/tmp"),
            size: 1024,
            initial_data: None,
            permissions: vec![Permission::Read],
        },
    };

    let res = manager.request_file(req, "").await;
    assert!(res.is_err(), "empty token should be denied");
}

#[tokio::test]
async fn non_empty_token_is_allowed() {
    let temp = tempdir().unwrap();
    let mut cfg = ManagerConfig::default();
    cfg.files_directory = temp.path().to_path_buf();

    // Use a mock provider that accepts tokens for strict validation
    let mock = commy::manager::auth_provider::MockAuthProvider::new(true);
    let manager = SharedFileManager::new_with_provider(cfg, std::sync::Arc::new(mock))
        .await
        .expect("manager init");

    let req = SharedFileRequest {
        identifier: "test_non_empty_token".to_string(),
        name: "".to_string(),
        description: None,
        pattern: commy::manager::MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: std::collections::HashMap::new(),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::ProducerConsumer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOnly,
        file_path: None,
        max_size_bytes: Some(1024),
        ttl_seconds: None,
        max_connections: None,
        required_permissions: vec![Permission::Read],
        encryption_required: false,
        auto_cleanup: false,
        persist_after_disconnect: false,
        transport_preference: commy::manager::TransportPreference::AutoOptimize,
        performance_requirements: commy::manager::PerformanceRequirements::default(),
        operation: SharedFileOperation::Create {
            path: std::path::PathBuf::from("/tmp"),
            size: 1024,
            initial_data: None,
            permissions: vec![Permission::Read],
        },
    };

    let res = manager.request_file(req, "valid_token").await;
    assert!(
        res.is_ok(),
        "non-empty token should be accepted by placeholder validator"
    );
}
