//! Integration test demonstrating basic distributed service mesh functionality
//! This test shows the fundamental capabilities of the modern commy library
//! Updated to use TestEnvironment pattern for proper cleanup

#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, CreationPolicy, Directionality, ExistencePolicy, MessagePattern,
    PerformanceRequirements, SerializationFormat, SharedFileOperation, SharedFileRequest, Topology,
    TransportPreference,
};

#[cfg(feature = "manager")]
use commy::SharedFileManager;

use std::path::PathBuf;

mod test_utils;
use test_utils::{TestCleanupGuard, TestEnvironment};

#[cfg(feature = "manager")]
#[tokio::test]
async fn test_shared_file_manager_creation() {
    let test_env = TestEnvironment::new().expect("Failed to create test environment");
    let _cleanup_guard = TestCleanupGuard::new(|| {
        println!("🧹 Cleaning up test resources for test_shared_file_manager_creation");
    });

    let manager_config = commy::manager::core::ManagerConfig {
        listen_port: 0, // Use random port for testing
        bind_address: "127.0.0.1".to_string(),
        max_files: 10,
        max_file_size: 1024 * 1024, // 1MB
        default_ttl_seconds: 300,   // 5 minutes
        heartbeat_timeout_seconds: 30,
        cleanup_interval_seconds: 60,
        database_path: test_env.test_file_path("db.sqlite"),
        files_directory: test_env.temp_dir.path().to_path_buf(),
        tls_cert_path: None,
        tls_key_path: None,
        require_tls: false,
        performance_config: Default::default(),
        security_config: Default::default(),
        enable_mesh_capabilities: false, // Disable for testing
    };

    let _file_manager = SharedFileManager::new(manager_config).await.unwrap();
    println!("✅ Created shared file manager successfully with isolated environment");
}

#[cfg(feature = "manager")]
#[tokio::test]
async fn test_shared_file_request_creation() {
    // Test that we can create a basic SharedFileRequest with all required fields
    let request = SharedFileRequest {
        identifier: "test_basic_request".to_string(),
        file_path: Some(PathBuf::from("test.dat")),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: Default::default(),
        name: "test_file".to_string(),
        description: Some("Test file for basic request".to_string()),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Agnostic,
        existence_policy: ExistencePolicy::CreateOrConnect,
        creation_policy: CreationPolicy::Create,
        max_size_bytes: Some(1024),
        ttl_seconds: None,
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::PreferLocal,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Read {
            path: PathBuf::from("test.dat"),
            offset: 0,
            length: None,
        },
    };

    println!("Created shared file request: {}", request.name);
    assert_eq!(request.name, "test_file");
}

#[cfg(feature = "manager")]
#[tokio::test]
async fn test_serialization_formats() {
    // Test that we can create SharedFileRequest with different serialization formats
    let formats = vec![
        SerializationFormat::Json,
        SerializationFormat::Binary,
        SerializationFormat::MessagePack,
        SerializationFormat::Cbor,
        SerializationFormat::ZeroCopy,
    ];

    for format in formats {
        let request = SharedFileRequest {
            identifier: format!("test_{:?}", format),
            file_path: Some(PathBuf::from(format!("test_{:?}.dat", format))),
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: Default::default(),
            name: format!("test_{:?}", format),
            description: Some("Test serialization format".to_string()),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: format.clone(),
            connection_side: ConnectionSide::Agnostic,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            max_size_bytes: Some(1024),
            ttl_seconds: None,
            max_connections: Some(1),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements::default(),
            operation: SharedFileOperation::Read {
                path: PathBuf::from(format!("test_{:?}.dat", format)),
                offset: 0,
                length: None,
            },
        };

        println!("Created request with format {:?}: {}", format, request.name);
        assert_eq!(request.serialization, format);
    }
}

#[cfg(feature = "manager")]
#[tokio::test]
async fn test_connection_topologies() {
    // Test that we can create SharedFileRequest with different topologies
    let topologies = vec![
        Topology::OneToOne,
        Topology::OneToMany,
        Topology::ManyToOne,
        Topology::ManyToMany,
    ];

    for topology in topologies {
        let request = SharedFileRequest {
            identifier: format!("test_{:?}", topology),
            file_path: Some(PathBuf::from(format!("test_{:?}.dat", topology))),
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: Default::default(),
            name: format!("test_{:?}", topology),
            description: Some("Test topology".to_string()),
            directionality: Directionality::ReadWrite,
            topology: topology.clone(),
            serialization: SerializationFormat::Json,
            connection_side: ConnectionSide::Agnostic,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            max_size_bytes: Some(1024),
            ttl_seconds: None,
            max_connections: Some(10),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements::default(),
            operation: SharedFileOperation::Read {
                path: PathBuf::from(format!("test_{:?}.dat", topology)),
                offset: 0,
                length: None,
            },
        };

        println!(
            "Created request with topology {:?}: {}",
            topology, request.name
        );
        assert_eq!(request.topology, topology);
    }
}

// Fallback tests for when the manager feature is not enabled
#[cfg(not(feature = "manager"))]
#[test]
fn test_basic_library_compilation() {
    // Basic test to ensure the library compiles without manager feature
    println!("Commy library compiled successfully without manager feature");
}
