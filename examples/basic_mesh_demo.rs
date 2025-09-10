//! Basic Mesh Demonstration
//!
//! This example shows the core distributed service mesh capabilities:
//! - Intelligent transport selection (shared memory vs network)
//! - Multi-format serialization
//! - Performance-based routing

use commy::manager::transport::{NetworkConfig, SharedMemoryConfig, SyncStrategy};
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    FallbackBehavior, MessagePattern, PerformanceRequirements, PerformanceThresholds,
    SerializationFormat, SharedFileRequest, TlsConfig, TlsVersion, Topology, TransportConfig,
    TransportManager, TransportPreference,
};
use commy::SharedFileManager;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Commy Distributed Service Mesh Demo");

    // Step 1: Create optimized transport configuration
    let transport_config = create_optimized_config().await?;

    // Step 2: Initialize the intelligent transport manager
    let transport_manager = TransportManager::new(transport_config).await?;

    // Step 3: Initialize the shared file manager with auth and config
    let manager_config = commy::manager::core::ManagerConfig {
        listen_port: 8080,
        bind_address: "127.0.0.1".to_string(),
        max_files: 1000,
        max_file_size: 100 * 1024 * 1024, // 100MB
        default_ttl_seconds: 3600,        // 1 hour
        heartbeat_timeout_seconds: 30,
        cleanup_interval_seconds: 60,
        database_path: std::path::PathBuf::from("./commy_data/db.sqlite"),
        files_directory: std::path::PathBuf::from("./commy_data"),
        tls_cert_path: None,
        tls_key_path: None,
        require_tls: false,
        performance_config: Default::default(),
        security_config: Default::default(),
        enable_mesh_capabilities: true,
    };
    let file_manager = SharedFileManager::new(manager_config).await?;

    // Step 4: Demonstrate intelligent transport selection
    demonstrate_transport_intelligence(&transport_manager).await?;

    // Step 5: Show multi-format serialization
    demonstrate_serialization_formats(&file_manager).await?;

    // Step 6: Performance-based optimization
    demonstrate_performance_optimization(&transport_manager).await?;

    println!("✅ Commy mesh demonstration completed successfully!");
    Ok(())
}

async fn create_optimized_config() -> Result<TransportConfig, Box<dyn std::error::Error>> {
    println!("⚙️  Creating optimized transport configuration...");

    let config = TransportConfig {
        default_preference: TransportPreference::AutoOptimize,
        performance_thresholds: PerformanceThresholds {
            latency_local_threshold_us: 100,    // 100 microseconds max for local
            latency_network_threshold_us: 5000, // 5ms max for network
            throughput_network_threshold_mbps: 100, // 100 MB/s minimum
            large_message_threshold_bytes: 1024 * 1024, // 1MB threshold
            high_connection_threshold: 100,     // 100 connections threshold
        },
        network_config: NetworkConfig {
            default_port: 8080,
            endpoints: vec!["127.0.0.1:8080".to_string()],
            connection_timeout_seconds: 30,
            read_timeout_seconds: 10,
            write_timeout_seconds: 10,
            tcp_keepalive: true,
            keepalive_interval_seconds: 60,
            max_connections: 100,
            connection_pool_size: 10,
            tcp_nodelay: true,
            tls_config: TlsConfig {
                enabled: false,
                required: false,
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
                server_name: None,
                min_version: TlsVersion::Tls12,
                cipher_suites: vec![],
                verify_certificates: true,
            },
        },
        shared_memory_config: SharedMemoryConfig {
            files_directory: PathBuf::from("./commy_shared"),
            default_file_size: 1024 * 1024,   // 1MB default
            max_file_size: 100 * 1024 * 1024, // 100MB in bytes
            file_permissions: 0o644,
            enable_locking: true,
            sync_strategy: SyncStrategy::Periodic { interval_ms: 1000 },
            enable_optimizations: true,
        },
        auto_optimization: true,
        fallback_behavior: FallbackBehavior::BestAvailable,
    };

    println!("✅ Transport configuration created with intelligent routing");
    Ok(config)
}

async fn demonstrate_transport_intelligence(
    transport_manager: &TransportManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🧠 Demonstrating Intelligent Transport Selection");

    // Test different scenarios and preferences
    let scenarios = vec![
        (
            "High-performance local file",
            TransportPreference::PreferLocal,
        ),
        (
            "Distributed network file",
            TransportPreference::PreferNetwork,
        ),
        ("Auto-optimized file", TransportPreference::AutoOptimize),
        ("Require local file", TransportPreference::LocalOnly),
    ];

    for (description, preference) in scenarios {
        println!("  📊 Testing: {}", description);

        // Create a request with specific preferences
        let request = SharedFileRequest {
            identifier: format!("test_{}", description.replace(" ", "_")),
            name: format!("test_{}", description.replace(" ", "_")),
            description: Some(description.to_string()),
            pattern: MessagePattern::RequestResponse {
                timeout_ms: Some(5000),
                retry_count: Some(3),
            },
            pattern_config: std::collections::HashMap::new(),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: SerializationFormat::Json,
            connection_side: ConnectionSide::Agnostic,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            file_path: Some(PathBuf::from(format!(
                "test_{}.dat",
                description.replace(" ", "_")
            ))),
            max_size_bytes: Some(1024 * 1024), // 1MB file
            ttl_seconds: None,
            max_connections: Some(10),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: preference,
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(1),
                min_throughput_mbps: Some(50),
                consistency_level: ConsistencyLevel::Eventual,
                durability_required: false,
            },
            operation: commy::manager::SharedFileOperation::Read {
                path: PathBuf::from(format!("test_{}.dat", description.replace(" ", "_"))),
                offset: 0,
                length: None,
            },
        };

        // Get routing decision from intelligent transport manager
        match transport_manager.route_request(&request).await {
            Ok(decision) => {
                println!("    ✅ Routed to: {:?}", decision.transport);
                println!("    📈 Confidence: {:.1}%", decision.confidence * 100.0);
                println!(
                    "    🕐 Est. Latency: {}μs",
                    decision.expected_performance.expected_latency_us
                );
                println!("    💡 Reason: {:?}", decision.reason);
            }
            Err(e) => println!("    ❌ Routing failed: {}", e),
        }
    }

    Ok(())
}

async fn demonstrate_serialization_formats(
    _file_manager: &SharedFileManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎯 Demonstrating Multi-Format Serialization Support");

    // Test data structure
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<f64>,
        metadata: std::collections::HashMap<String, String>,
    }

    let _test_data = TestData {
        id: 12345,
        name: "Commy Test Data".to_string(),
        values: vec![1.0, 2.5, std::f64::consts::PI, 42.0],
        metadata: [
            ("format".to_string(), "demo".to_string()),
            ("version".to_string(), "1.0".to_string()),
        ]
        .into_iter()
        .collect(),
    };

    // Test each serialization format
    let formats = vec![
        (
            "JSON (Human Readable)",
            commy::manager::SerializationFormat::Json,
        ),
        (
            "Binary (Compact)",
            commy::manager::SerializationFormat::Binary,
        ),
        (
            "MessagePack (Efficient)",
            commy::manager::SerializationFormat::MessagePack,
        ),
        (
            "CBOR (Standards-based)",
            commy::manager::SerializationFormat::Cbor,
        ),
        (
            "Zero-copy (Ultra-fast)",
            commy::manager::SerializationFormat::ZeroCopy,
        ),
    ];

    for (description, format) in formats {
        println!("  📦 Testing: {}", description);

        // Simulate serialization performance metrics
        let start = std::time::Instant::now();

        // Create a request for this format
        let _request = SharedFileRequest {
            identifier: format!("test_{:?}", format),
            name: format!("test_{:?}", format),
            description: Some("Serialization format test".to_string()),
            pattern: MessagePattern::RequestResponse {
                timeout_ms: Some(5000),
                retry_count: Some(3),
            },
            pattern_config: std::collections::HashMap::new(),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: format.clone(),
            connection_side: ConnectionSide::Agnostic,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            file_path: Some(PathBuf::from(format!("test_{:?}.dat", format))),
            max_size_bytes: Some(1024),
            ttl_seconds: None,
            max_connections: Some(1),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(100),
                min_throughput_mbps: Some(10),
                consistency_level: ConsistencyLevel::Eventual,
                durability_required: false,
            },
            operation: commy::manager::SharedFileOperation::Read {
                path: PathBuf::from(format!("test_{:?}.dat", format)),
                offset: 0,
                length: None,
            },
        };

        let duration = start.elapsed();
        println!("    ⚡ Format setup time: {:?}", duration);
        println!(
            "    📊 Est. efficiency: {}%",
            match format {
                commy::manager::SerializationFormat::Json => 60,
                commy::manager::SerializationFormat::Binary => 85,
                commy::manager::SerializationFormat::MessagePack => 80,
                commy::manager::SerializationFormat::Cbor => 75,
                commy::manager::SerializationFormat::ZeroCopy => 95,
                _ => 70,
            }
        );
    }

    Ok(())
}

async fn demonstrate_performance_optimization(
    transport_manager: &TransportManager,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏃 Demonstrating Performance-Based Optimization");

    // Simulate performance monitoring and optimization
    println!("  📊 Current performance snapshot:");

    // Get current performance metrics
    let perf_monitor = transport_manager.get_performance_monitor();
    let snapshot = perf_monitor.get_current_snapshot().await;

    println!("    🏠 Local latency: {}μs", snapshot.local.avg_latency_us);
    println!(
        "    🌐 Network latency: {}μs",
        snapshot.network.avg_latency_us
    );
    println!(
        "    📈 Local throughput: {:.1} MB/s",
        snapshot.local.avg_throughput_mbps
    );
    println!(
        "    📈 Network throughput: {:.1} MB/s",
        snapshot.network.avg_throughput_mbps
    );
    println!(
        "    ⚡ Success rate: {:.1}%",
        snapshot.local.success_rate * 100.0
    );

    // Demonstrate adaptive routing based on performance
    println!("  🎯 Adaptive routing decisions:");

    let high_perf_request = SharedFileRequest {
        identifier: "high_performance_file".to_string(),
        name: "high_performance_file".to_string(),
        description: Some("High performance test file".to_string()),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(1000),
            retry_count: Some(1),
        },
        pattern_config: std::collections::HashMap::new(),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::ZeroCopy,
        connection_side: ConnectionSide::Agnostic,
        existence_policy: ExistencePolicy::CreateOrConnect,
        creation_policy: CreationPolicy::Create,
        file_path: Some(PathBuf::from("high_performance_file.dat")),
        max_size_bytes: Some(10 * 1024 * 1024), // 10MB
        ttl_seconds: None,
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: true,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(1),        // Ultra-low latency required
            min_throughput_mbps: Some(200), // High throughput required
            consistency_level: ConsistencyLevel::Strong,
            durability_required: true,
        },
        operation: commy::manager::SharedFileOperation::Read {
            path: PathBuf::from("high_performance_file.dat"),
            offset: 0,
            length: None,
        },
    };

    match transport_manager.route_request(&high_perf_request).await {
        Ok(decision) => {
            println!("    ✅ High-performance routing:");
            println!("      🎯 Selected: {:?}", decision.transport);
            println!("      📊 Confidence: {:.1}%", decision.confidence * 100.0);
            println!(
                "      ⚡ Estimated latency: {}μs",
                decision.expected_performance.expected_latency_us
            );
        }
        Err(e) => println!("    ❌ High-performance routing failed: {}", e),
    }

    println!("  🔄 Performance history and learning:");
    let history = perf_monitor.get_performance_history().await;
    println!(
        "    📈 Tracked samples: {} local, {} network",
        history.local_samples.len(),
        history.network_samples.len()
    );
    println!("    🧠 Max samples: {}", history.max_samples);

    Ok(())
}
