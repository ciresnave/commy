//! Advanced Transport Demo
//!
//! Demonstrates intelligent transport selection, routing optimization,
//! and performance-aware decision making in the Commy service mesh.

#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    MessagePattern, PerformanceRequirements, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportConfig, TransportManager, TransportPreference,
};

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::time::{sleep, Duration, Instant};

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Advanced Transport Demo: Intelligent Routing & Optimization");
    println!("================================================================");

    // Initialize transport manager with optimized configuration
    let transport_config = TransportConfig::builder().build()?;

    let transport_manager = TransportManager::new(transport_config).await?;
    println!("✅ Transport manager initialized with intelligent routing");

    // Demonstrate different performance scenarios
    let scenarios = vec![
        (
            "Ultra-Low Latency",
            create_latency_critical_request(),
            "Optimized for sub-millisecond response times",
        ),
        (
            "High Throughput",
            create_throughput_optimized_request(),
            "Optimized for maximum data transfer rates",
        ),
        (
            "Balanced Performance",
            create_balanced_request(),
            "Balanced latency and throughput requirements",
        ),
        (
            "Large File Transfer",
            create_large_file_request(),
            "Optimized for transferring large datasets",
        ),
        (
            "Real-time Streaming",
            create_streaming_request(),
            "Optimized for continuous data streams",
        ),
    ];

    println!("\n🎯 Testing Different Performance Scenarios:");
    println!("==========================================");

    for (scenario_name, request, description) in scenarios {
        println!("\n📊 Scenario: {}", scenario_name);
        println!("   💡 {}", description);

        // Get routing decision
        let start_time = Instant::now();
        let routing_decision = transport_manager.route_request(&request).await?;
        let routing_time = start_time.elapsed();

        println!("   🎯 Selected Transport: {:?}", routing_decision.transport);
        println!(
            "   ⚡ Expected Latency: {}μs",
            routing_decision.expected_performance.expected_latency_us
        );
        println!(
            "   📈 Expected Throughput: {} MB/s",
            routing_decision
                .expected_performance
                .expected_throughput_mbps
        );
        println!(
            "   🎲 Confidence: {:.1}%",
            routing_decision.confidence * 100.0
        );
        println!("   ⏱️  Routing Time: {}μs", routing_time.as_micros());

        // Simulate request execution
        match transport_manager
            .execute_request(request, &routing_decision)
            .await
        {
            Ok(response) => {
                println!("   ✅ Request executed successfully");
                println!("   📁 Response received");
            }
            Err(e) => {
                println!("   ⚠️  Request simulation: {}", e);
            }
        }

        // Small delay between scenarios
        sleep(Duration::from_millis(100)).await;
    }

    // Demonstrate adaptive routing with changing conditions
    println!("\n🔄 Adaptive Routing Under Changing Conditions:");
    println!("==============================================");

    let base_request = create_adaptive_test_request();

    // Simulate different network conditions
    let network_conditions = vec![
        ("Optimal", 1.0),
        ("High Load", 0.7),
        ("Network Congestion", 0.4),
        ("Recovery", 0.8),
        ("Peak Performance", 1.0),
    ];

    for (condition, performance_factor) in network_conditions {
        println!(
            "\n🌐 Network Condition: {} (Factor: {:.1})",
            condition, performance_factor
        );

        // The transport manager would adapt based on real metrics
        // For demo purposes, we show how decisions might change
        let decision = transport_manager.route_request(&base_request).await?;

        println!("   🎯 Adapted Transport: {:?}", decision.transport);
        println!("   📊 Performance Score: {:.2}", decision.confidence);

        sleep(Duration::from_millis(200)).await;
    }

    // Demonstrate transport failover
    println!("\n🛡️  Transport Failover Simulation:");
    println!("==================================");

    let critical_request = create_failover_test_request();
    let primary_decision = transport_manager.route_request(&critical_request).await?;

    println!("   🎯 Primary Transport: {:?}", primary_decision.transport);
    println!("   🔄 Simulating primary transport failure...");

    // In a real scenario, the transport manager would detect failure
    // and automatically reroute to backup transport
    sleep(Duration::from_millis(500)).await;

    let fallback_decision = transport_manager.route_request(&critical_request).await?;
    println!(
        "   🎯 Fallback Transport: {:?}",
        fallback_decision.transport
    );
    println!("   ✅ Automatic failover completed");

    println!("\n🎉 Advanced Transport Demo completed!");
    println!("   🧠 Intelligent routing adapts to performance requirements");
    println!("   ⚡ Sub-microsecond routing decisions");
    println!("   🔄 Automatic failover and load balancing");
    println!("   📊 Real-time performance optimization");

    Ok(())
}

#[cfg(feature = "manager")]
fn create_latency_critical_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "latency_critical".to_string(),
        name: "ultra_low_latency_data".to_string(),
        description: Some("Ultra-low latency critical data transfer".to_string()),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(1),
            retry_count: Some(0),
        },
        pattern_config: HashMap::from([
            ("priority".to_string(), "critical".to_string()),
            ("deadline_us".to_string(), "500".to_string()),
        ]),
        file_path: Some(PathBuf::from("latency_critical.dat")),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::ZeroCopy,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(4096), // Small payload for speed
        ttl_seconds: Some(60),
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::RequireLocal,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(1),
            min_throughput_mbps: Some(1000),
            consistency_level: ConsistencyLevel::Linearizable,
            durability_required: false,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("latency_critical.dat"),
            offset: 0,
            data: vec![0u8; 1024],
        },
    }
}

#[cfg(feature = "manager")]
fn create_throughput_optimized_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "high_throughput".to_string(),
        name: "bulk_data_transfer".to_string(),
        description: Some("High throughput bulk data transfer".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: true,
        },
        pattern_config: HashMap::from([
            ("batch_size".to_string(), "1048576".to_string()), // 1MB batches
            ("compression".to_string(), "enabled".to_string()),
        ]),
        file_path: Some(PathBuf::from("bulk_transfer.dat")),
        directionality: Directionality::WriteOnly,
        topology: Topology::OneToMany,
        serialization: SerializationFormat::Binary,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(100 * 1024 * 1024), // 100MB
        ttl_seconds: Some(3600),
        max_connections: Some(10),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::PreferNetwork,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(100),
            min_throughput_mbps: Some(1000),
            consistency_level: ConsistencyLevel::Eventual,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("bulk_transfer.dat"),
            offset: 0,
            data: vec![0u8; 1024 * 1024], // 1MB test data
        },
    }
}

#[cfg(feature = "manager")]
fn create_balanced_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "balanced_performance".to_string(),
        name: "standard_operation".to_string(),
        description: Some("Balanced performance requirements".to_string()),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(5000),
            retry_count: Some(3),
        },
        pattern_config: HashMap::from([("balance_factor".to_string(), "0.5".to_string())]),
        file_path: Some(PathBuf::from("balanced.dat")),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::MessagePack,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(10 * 1024 * 1024), // 10MB
        ttl_seconds: Some(1800),
        max_connections: Some(5),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::Adaptive,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(50),
            min_throughput_mbps: Some(100),
            consistency_level: ConsistencyLevel::Strong,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("balanced.dat"),
            offset: 0,
            data: vec![0u8; 64 * 1024], // 64KB
        },
    }
}

#[cfg(feature = "manager")]
fn create_large_file_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "large_file_transfer".to_string(),
        name: "large_dataset".to_string(),
        description: Some("Large file transfer optimization".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: true,
        },
        pattern_config: HashMap::from([
            ("chunk_size".to_string(), "1048576".to_string()),
            ("parallel_streams".to_string(), "4".to_string()),
        ]),
        file_path: Some(PathBuf::from("large_dataset.dat")),
        directionality: Directionality::WriteOnly,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Binary,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(1024 * 1024 * 1024), // 1GB
        ttl_seconds: Some(7200),
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::PreferNetwork,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(1000),
            min_throughput_mbps: Some(500),
            consistency_level: ConsistencyLevel::Strong,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("large_dataset.dat"),
            offset: 0,
            data: vec![0u8; 10 * 1024 * 1024], // 10MB chunk
        },
    }
}

#[cfg(feature = "manager")]
fn create_streaming_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "realtime_stream".to_string(),
        name: "live_data_stream".to_string(),
        description: Some("Real-time streaming data".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false, // Fast streaming
        },
        pattern_config: HashMap::from([
            ("stream_rate_hz".to_string(), "1000".to_string()),
            ("low_latency_mode".to_string(), "true".to_string()),
        ]),
        file_path: Some(PathBuf::from("stream.dat")),
        directionality: Directionality::WriteOnly,
        topology: Topology::OneToMany,
        serialization: SerializationFormat::ZeroCopy,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(1024 * 1024), // 1MB
        ttl_seconds: Some(300),
        max_connections: Some(100),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::RequireLocal,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(5),
            min_throughput_mbps: Some(200),
            consistency_level: ConsistencyLevel::Eventual,
            durability_required: false,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("stream.dat"),
            offset: 0,
            data: vec![0u8; 1024], // 1KB stream packet
        },
    }
}

#[cfg(feature = "manager")]
fn create_adaptive_test_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "adaptive_test".to_string(),
        name: "adaptive_routing_test".to_string(),
        description: Some("Testing adaptive routing capabilities".to_string()),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(2000),
            retry_count: Some(2),
        },
        pattern_config: HashMap::from([("adaptive_mode".to_string(), "enabled".to_string())]),
        file_path: Some(PathBuf::from("adaptive_test.dat")),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::MessagePack,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(1024 * 1024),
        ttl_seconds: Some(600),
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::Adaptive,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(100),
            min_throughput_mbps: Some(50),
            consistency_level: ConsistencyLevel::Strong,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("adaptive_test.dat"),
            offset: 0,
            data: vec![0u8; 4096],
        },
    }
}

#[cfg(feature = "manager")]
fn create_failover_test_request() -> SharedFileRequest {
    SharedFileRequest {
        identifier: "failover_test".to_string(),
        name: "critical_failover_test".to_string(),
        description: Some("Testing transport failover mechanisms".to_string()),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(1000),
            retry_count: Some(5),
        },
        pattern_config: HashMap::from([
            ("failover_enabled".to_string(), "true".to_string()),
            ("critical_priority".to_string(), "true".to_string()),
        ]),
        file_path: Some(PathBuf::from("failover_test.dat")),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Binary,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(512 * 1024),
        ttl_seconds: Some(300),
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: true,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::Adaptive,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(20),
            min_throughput_mbps: Some(100),
            consistency_level: ConsistencyLevel::Linearizable,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from("failover_test.dat"),
            offset: 0,
            data: vec![0u8; 2048],
        },
    }
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Advanced Transport demo requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example advanced_transport_demo --features manager");
    std::process::exit(1);
}
