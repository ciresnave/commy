//! Consumer service - demonstrates reading data using the modern Commy service mesh
//! This example shows how a service can consume data through intelligent transport selection

use commy::manager::core::{ManagerConfig, SharedFileManager};
#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    MessagePattern, PerformanceRequirements, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportConfig, TransportManager, TransportPreference,
};

use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📥 Consumer Service: Starting data consumption...");

    // Initialize the SharedFileManager with optimized configuration
    let config = ManagerConfig::default();
    let file_manager = SharedFileManager::new(config).await?;

    println!("✅ SharedFileManager initialized");

    // Wait a bit for producer to start creating data
    println!("⏳ Waiting for producer to start...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Consume data packets produced by the producer
    for i in 0..10 {
        println!("📖 Consumer: Attempting to read data packet {}", i);

        // Create a shared file request to read this data packet using modern API
        let request = SharedFileRequest {
            identifier: format!("data_packet_{}", i),
            name: format!("consumer_read_{}", i),
            description: Some("Consumer reading data packet".to_string()),
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: std::collections::HashMap::from([
                ("max_retries".to_string(), "3".to_string()),
                ("timeout_seconds".to_string(), "30".to_string()),
                ("priority".to_string(), "1".to_string()),
            ]),
            directionality: Directionality::ReadOnly,
            topology: Topology::OneToOne,
            serialization: SerializationFormat::Json,
            connection_side: ConnectionSide::Consumer,
            creation_policy: CreationPolicy::NeverCreate,
            existence_policy: ExistencePolicy::CreateOrConnect,
            file_path: Some(PathBuf::from(format!("producer_data_{}.json", i))),
            max_size_bytes: Some(1024 * 1024), // 1MB
            ttl_seconds: Some(3600),           // 1 hour
            max_connections: Some(1),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements::default(),
            operation: SharedFileOperation::Read {
                path: PathBuf::from(format!("producer_data_{}.json", i)),
                offset: 0,
                length: None,
            },
        };

        // Execute the request using SharedFileManager
        match file_manager.request_file(request, "consumer").await {
            Ok(response) => {
                println!("✅ Consumer: Successfully read data packet {}", i);
                println!("  📄 File path: {:?}", response.file_path);

                // In a real implementation, you would process the actual data here
                println!("  💾 Data processed successfully");
            }
            Err(e) => {
                println!("❌ Consumer: Failed to read data packet {}: {}", i, e);

                // If the file doesn't exist yet, wait a bit more
                if e.to_string().contains("NotImplemented") {
                    println!("  ⏳ Waiting for producer to create packet...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }
        }

        // Small delay between reads
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Demonstrate different data access patterns
    println!("\n🔄 Consumer: Testing different access patterns...");

    // Create a transport manager for routing decisions
    let transport_config = TransportConfig::builder().auto_optimization(true).build()?;
    let transport_manager = TransportManager::new(transport_config).await?;

    let access_patterns = vec![
        ("high_priority_data", TransportPreference::RequireLocal),
        ("distributed_cache", TransportPreference::PreferNetwork),
        ("adaptive_stream", TransportPreference::Adaptive),
    ];

    for (pattern_name, preference) in access_patterns {
        let request = SharedFileRequest {
            identifier: format!("pattern_{}", pattern_name),
            name: pattern_name.to_string(),
            description: Some(format!("Testing {} access pattern", pattern_name)),
            pattern: MessagePattern::RequestResponse {
                timeout_ms: Some(5000),
                retry_count: Some(3),
            },
            pattern_config: std::collections::HashMap::new(),
            file_path: Some(PathBuf::from(format!("{}.dat", pattern_name))),
            directionality: Directionality::ReadOnly,
            topology: match pattern_name {
                "high_priority_data" => Topology::OneToOne,
                "distributed_cache" => Topology::OneToMany,
                "adaptive_stream" => Topology::ManyToMany,
                _ => Topology::OneToOne,
            },
            serialization: match pattern_name {
                "high_priority_data" => SerializationFormat::ZeroCopy,
                "distributed_cache" => SerializationFormat::Binary,
                "adaptive_stream" => SerializationFormat::MessagePack,
                _ => SerializationFormat::Json,
            },
            connection_side: ConnectionSide::Consumer,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::NeverCreate,
            max_size_bytes: Some(2048),
            ttl_seconds: None,
            max_connections: Some(1),
            required_permissions: vec![],
            encryption_required: pattern_name == "high_priority_data",
            auto_cleanup: false,
            persist_after_disconnect: false,
            transport_preference: preference,
            performance_requirements: match pattern_name {
                "high_priority_data" => PerformanceRequirements {
                    max_latency_ms: Some(1),
                    min_throughput_mbps: Some(1000),
                    consistency_level: ConsistencyLevel::Linearizable,
                    durability_required: true,
                },
                "distributed_cache" => PerformanceRequirements {
                    max_latency_ms: Some(100),
                    min_throughput_mbps: Some(50),
                    consistency_level: ConsistencyLevel::Eventual,
                    durability_required: false,
                },
                "adaptive_stream" => PerformanceRequirements {
                    max_latency_ms: Some(10),
                    min_throughput_mbps: Some(200),
                    consistency_level: ConsistencyLevel::Strong,
                    durability_required: false,
                },
                _ => PerformanceRequirements::default(),
            },
            operation: SharedFileOperation::Read {
                path: PathBuf::from(format!("{}.dat", pattern_name)),
                offset: 0,
                length: None,
            },
        };

        let decision = transport_manager.route_request(&request).await?;
        println!(
            "📈 Pattern '{}' → {:?} (latency: {}μs)",
            pattern_name, decision.transport, decision.expected_performance.expected_latency_us
        );
    }

    println!("🏁 Consumer Service: Finished consuming data");
    Ok(())
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Consumer example requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example consumer --features manager");
    std::process::exit(1);
}
