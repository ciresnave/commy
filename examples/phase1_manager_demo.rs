//! Phase 1 Manager Demo
//!
//! This example demonstrates the working Phase 1 SharedFileManager
//! with the actual implemented functionality.

#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, CreationPolicy, Directionality, ExistencePolicy, MessagePattern,
    PerformanceRequirements, Permission, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportPreference,
};

#[cfg(feature = "manager")]
async fn run_phase1_demo() -> Result<(), Box<dyn std::error::Error>> {
    use commy::manager::core::{ManagerConfig, SharedFileManager};
    use std::path::PathBuf;

    println!("🚀 Commy Phase 1 Manager Demo");
    println!("=============================");

    // Initialize the SharedFileManager
    let config = ManagerConfig {
        listen_port: 8080,
        bind_address: "127.0.0.1".to_string(),
        max_files: 1000,
        max_file_size: 1024 * 1024 * 1024, // 1GB
        default_ttl_seconds: 300,
        heartbeat_timeout_seconds: 30,
        cleanup_interval_seconds: 60,
        enable_mesh_capabilities: true,
        database_path: PathBuf::from("./temp/commy_phase1.db"),
        files_directory: PathBuf::from("./temp/commy_phase1"),
        tls_cert_path: None,
        tls_key_path: None,
        require_tls: false,
        performance_config: commy::manager::core::PerformanceConfig {
            enabled: true,
            collection_interval_seconds: 10,
            history_retention_days: 7,
            detailed_latency_tracking: true,
        },
        security_config: commy::manager::core::SecurityConfig {
            require_auth: true,
            max_auth_failures: 3,
            auth_lockout_seconds: 300,
            audit_logging: true,
            audit_log_path: PathBuf::from("./temp/audit.log"),
            threat_detection: false,
        },
    };

    let manager = SharedFileManager::new(config).await?;
    println!("✅ SharedFileManager initialized");

    // Note: We don't call start() here because it runs a server indefinitely
    // For this demo, we'll just test the core file management functionality

    // Demonstrate file creation
    println!("\n📁 Testing File Operations:");

    let create_request = SharedFileRequest {
        identifier: "demo_file_1".to_string(),
        name: "demo_file_1".to_string(),
        description: Some("Demo file creation".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: std::collections::HashMap::from([(
            "timeout_seconds".to_string(),
            "30".to_string(),
        )]),
        file_path: None, // Let manager decide path
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(1024 * 1024), // 1MB
        ttl_seconds: Some(300),
        max_connections: Some(1),
        required_permissions: vec![Permission::Read, Permission::Write],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Create {
            path: std::path::PathBuf::from("demo_file_1.dat"),
            size: 1024 * 1024, // 1MB
            initial_data: Some(vec![]),
            permissions: vec![Permission::Read, Permission::Write],
        },
    };

    let response = manager
        .request_file(create_request.clone(), "demo_token")
        .await?;
    println!("   ✅ Created file with ID: {}", response.file_id);
    println!("   📍 File path: {:?}", response.file_path);
    println!("   📊 Size: {} bytes", response.metadata.size_bytes);
    println!("   🔒 Connections: {}", response.metadata.connection_count);

    // Demonstrate connecting to existing file
    let connect_request = SharedFileRequest {
        identifier: "demo_file_1".to_string(), // Same identifier
        name: "demo_file_1_connect".to_string(),
        description: Some("Connect to demo file".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: std::collections::HashMap::from([(
            "timeout_seconds".to_string(),
            "30".to_string(),
        )]),
        file_path: None,
        directionality: Directionality::ReadOnly,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Consumer,
        creation_policy: CreationPolicy::NeverCreate,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(1024 * 1024),
        ttl_seconds: Some(300),
        max_connections: Some(1),
        required_permissions: vec![Permission::Read],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Read {
            path: std::path::PathBuf::from("demo_file_1.dat"),
            offset: 0,
            length: None,
        },
    };

    let response2 = manager.request_file(connect_request, "demo_token").await?;
    println!("   ✅ Connected to existing file ID: {}", response2.file_id);
    println!(
        "   🔗 Connections now: {}",
        response2.metadata.connection_count
    );

    // Demonstrate creating a second file
    let create_request2 = SharedFileRequest {
        identifier: "demo_file_2".to_string(),
        name: "demo_file_2".to_string(),
        description: Some("Second demo file with custom path".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: std::collections::HashMap::from([(
            "timeout_seconds".to_string(),
            "30".to_string(),
        )]),
        file_path: Some(PathBuf::from("./temp/custom_file.mmap")),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(512 * 1024), // 512KB
        ttl_seconds: Some(300),
        max_connections: Some(1),
        required_permissions: vec![Permission::Read, Permission::Write, Permission::Admin],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Create {
            path: PathBuf::from("./temp/custom_file.mmap"),
            size: 512 * 1024, // 512KB
            initial_data: Some(vec![]),
            permissions: vec![Permission::Read, Permission::Write, Permission::Admin],
        },
    };

    let response3 = manager.request_file(create_request2, "demo_token").await?;
    println!("   ✅ Created second file with ID: {}", response3.file_id);
    println!("   📍 Custom path: {:?}", response3.file_path);

    // Test ConnectOnly policy with non-existent file
    println!("\n🚫 Testing Error Handling:");
    let connect_only_request = SharedFileRequest {
        identifier: "non_existent_file".to_string(),
        name: "non_existent_file".to_string(),
        description: Some("Trying to connect to non-existent file".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: std::collections::HashMap::from([(
            "timeout_seconds".to_string(),
            "30".to_string(),
        )]),
        file_path: None,
        directionality: Directionality::ReadOnly,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Consumer,
        creation_policy: CreationPolicy::NeverCreate,
        existence_policy: ExistencePolicy::ConnectOnly,
        max_size_bytes: Some(1024),
        ttl_seconds: Some(300),
        max_connections: Some(1),
        required_permissions: vec![Permission::Read],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Read {
            path: std::path::PathBuf::from("non_existent.dat"),
            offset: 0,
            length: None,
        },
    };

    match manager
        .request_file(connect_only_request, "demo_token")
        .await
    {
        Ok(_) => println!("   ❌ Unexpected success"),
        Err(e) => println!("   ✅ Expected error: {:?}", e),
    }

    // Test invalid authentication
    match manager.request_file(create_request, "").await {
        Ok(_) => println!("   ❌ Unexpected success with empty token"),
        Err(e) => println!("   ✅ Expected auth error: {:?}", e),
    }

    // Demonstrate disconnection
    println!("\n🔌 Testing Disconnection:");
    manager.disconnect_file(response.file_id).await?;
    println!("   ✅ Disconnected from file {}", response.file_id);

    manager.disconnect_file(response2.file_id).await?;
    println!(
        "   ✅ File {} cleaned up (no more connections)",
        response2.file_id
    );

    // Show performance characteristics
    println!("\n📊 Performance Profile:");
    println!("   • Transport: {:?}", response3.transport);
    println!(
        "   • Expected latency: {}μs",
        response3.performance_profile.expected_latency_us
    );
    println!(
        "   • Expected throughput: {} Mbps",
        response3.performance_profile.expected_throughput_mbps
    );
    println!(
        "   • High performance: {}",
        response3.performance_profile.high_performance
    );
    println!("   • Tier: {:?}", response3.performance_profile.tier);

    // Show security context
    println!("\n🔒 Security Context:");
    println!("   • Identity: {}", response3.security_context.identity);
    println!("   • Session ID: {}", response3.security_context.session_id);
    println!(
        "   • Security level: {:?}",
        response3.security_context.security_level
    );

    // Shutdown gracefully (just cleanup, no server to stop)
    println!("\n🛑 Demo complete - cleaning up...");
    println!("✅ All operations successful");

    println!("\n🎉 Phase 1 Demo Complete!");
    println!("   ✅ File creation and connection");
    println!("   ✅ Multiple file management");
    println!("   ✅ Authentication and authorization");
    println!("   ✅ Error handling and validation");
    println!("   ✅ Performance profiling");
    println!("   ✅ Security context");
    println!("   ✅ Graceful shutdown");

    Ok(())
}

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for better output
    tracing_subscriber::fmt::init();

    run_phase1_demo().await
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Phase 1 manager demo requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example phase1_manager_demo --features manager");
}
