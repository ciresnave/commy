//! Phase 1 Integration Test
//!
//! Tests the type system and basic validation without hanging dependencies

use commy::manager::{
    ConnectionSide, CreationPolicy, Directionality, ExistencePolicy, MessagePattern,
    PerformanceRequirements, Permission, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportPreference,
};
use std::path::PathBuf;

#[test]
fn test_phase1_type_system() {
    // Test SharedFileRequest creation and validation
    let request = SharedFileRequest {
        identifier: "test_file".to_string(),
        name: "test_file".to_string(),
        description: Some("Test file for phase 1".to_string()),
        file_path: Some(PathBuf::from("test.mmap")),
        max_size_bytes: Some(1024 * 1024),
        existence_policy: ExistencePolicy::CreateOrConnect,
        required_permissions: vec![Permission::Read, Permission::Write],
        // New required fields
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: Default::default(),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Binary,
        connection_side: ConnectionSide::Agnostic,
        creation_policy: CreationPolicy::Create,
        ttl_seconds: Some(300),
        max_connections: Some(1),
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Create {
            path: PathBuf::from("test.mmap"),
            size: 1024 * 1024,
            initial_data: None,
            permissions: Default::default(),
        },
    };

    // Verify essential fields are set correctly
    assert_eq!(request.identifier, "test_file");
    assert_eq!(request.name, "test_file");
    assert_eq!(request.file_path, Some(PathBuf::from("test.mmap")));
    assert_eq!(request.max_size_bytes, Some(1024 * 1024));
    assert_eq!(request.existence_policy, ExistencePolicy::CreateOrConnect);
    assert_eq!(request.required_permissions.len(), 2);
    assert!(request.required_permissions.contains(&Permission::Read));
    assert!(request.required_permissions.contains(&Permission::Write));

    println!("✅ Phase 1 type system test passed!");
}

#[test]
fn test_phase1_existence_policies() {
    // Test all existence policy variants
    let policies = vec![
        ExistencePolicy::CreateOrConnect,
        ExistencePolicy::CreateOnly,
        ExistencePolicy::ConnectOnly,
    ];

    for policy in policies {
        let request = SharedFileRequest {
            identifier: "policy_test".to_string(),
            name: "policy_test".to_string(),
            description: Some("Test existence policy".to_string()),
            file_path: None,
            max_size_bytes: Some(4096),
            existence_policy: policy.clone(),
            required_permissions: vec![Permission::Read],
            // Required fields with defaults
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: Default::default(),
            directionality: Directionality::ReadOnly,
            topology: Topology::OneToOne,
            serialization: SerializationFormat::Binary,
            connection_side: ConnectionSide::Consumer,
            creation_policy: CreationPolicy::Create,
            ttl_seconds: Some(300),
            max_connections: Some(1),
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements::default(),
            operation: SharedFileOperation::Read {
                path: PathBuf::from("policy_test.dat"),
                offset: 0,
                length: None,
            },
        };

        assert_eq!(request.existence_policy, policy);
    }

    println!("✅ Phase 1 existence policies test passed!");
}

#[test]
fn test_phase1_permissions() {
    // Test all permission variants
    let permissions = vec![Permission::Read, Permission::Write, Permission::Admin];

    for permission in permissions {
        let request = SharedFileRequest {
            identifier: "permission_test".to_string(),
            name: "permission_test".to_string(),
            description: Some("Test permission".to_string()),
            file_path: None,
            max_size_bytes: Some(1024),
            existence_policy: ExistencePolicy::CreateOrConnect,
            required_permissions: vec![permission.clone()],
            // Required fields with defaults
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: Default::default(),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: SerializationFormat::Binary,
            connection_side: ConnectionSide::Agnostic,
            creation_policy: CreationPolicy::Create,
            ttl_seconds: Some(300),
            max_connections: Some(1),
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements::default(),
            operation: SharedFileOperation::Read {
                path: PathBuf::from("permission_test.dat"),
                offset: 0,
                length: None,
            },
        };

        assert_eq!(request.required_permissions[0], permission);
    }

    println!("✅ Phase 1 permissions test passed!");
}

#[test]
fn test_phase1_serialization() {
    // Test that types can be serialized/deserialized
    let request = SharedFileRequest {
        identifier: "serialize_test".to_string(),
        name: "serialize_test".to_string(),
        description: Some("Test serialization".to_string()),
        file_path: Some(PathBuf::from("serialize.mmap")),
        max_size_bytes: Some(2048),
        existence_policy: ExistencePolicy::CreateOnly,
        required_permissions: vec![Permission::Admin],
        // Required fields with defaults
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: Default::default(),
        directionality: Directionality::WriteOnly,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        ttl_seconds: Some(300),
        max_connections: Some(1),
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Create {
            path: PathBuf::from("serialize.mmap"),
            size: 2048,
            initial_data: None,
            permissions: Default::default(),
        },
    };

    // Test JSON serialization
    let json = serde_json::to_string(&request).expect("Failed to serialize to JSON");
    let deserialized: SharedFileRequest =
        serde_json::from_str(&json).expect("Failed to deserialize from JSON");

    assert_eq!(deserialized.identifier, request.identifier);
    assert_eq!(deserialized.name, request.name);
    assert_eq!(deserialized.file_path, request.file_path);
    assert_eq!(deserialized.max_size_bytes, request.max_size_bytes);
    assert_eq!(deserialized.existence_policy, request.existence_policy);
    assert_eq!(
        deserialized.required_permissions,
        request.required_permissions
    );

    println!("✅ Phase 1 serialization test passed!");
}
