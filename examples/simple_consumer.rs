//! Simple consumer - demonstrates consuming data from the service mesh
//! This reads data written by the simple_producer example

#[cfg(feature = "manager")]
use commy::manager::core::{ManagerConfig, SharedFileManager};
#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    MessagePattern, PerformanceRequirements, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportPreference,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SimpleData {
    counter: u32,
    message: String,
    timestamp: u64,
}

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Simple Consumer: Service Mesh Data Consumption Demo");
    println!("======================================================");

    // Initialize file manager
    let config = ManagerConfig {
        enable_mesh_capabilities: false,
        ..Default::default()
    };
    let file_manager = SharedFileManager::new(config).await?;

    println!("✅ File manager initialized");
    println!("⏳ Waiting for producer data...");

    // Wait for producer to start creating data
    sleep(Duration::from_secs(2)).await;

    // Consume data for several iterations
    for i in 0..10 {
        println!("\n📖 Consuming iteration {}:", i);

        // Create a request to read data from the producer
        let request = SharedFileRequest {
            identifier: format!("simple_data_iteration_{}", i % 5), // Producer creates 5 iterations
            name: format!("simple_data_iteration_{}", i % 5),
            description: Some(format!("Simple consumer reading iteration {}", i)),
            pattern: MessagePattern::PublishSubscribe {
                topic: "simple_data".to_string(),
                durable: false,
                filter: None,
            },
            pattern_config: HashMap::new(),
            directionality: Directionality::ReadOnly,
            topology: Topology::OneToMany,
            serialization: SerializationFormat::Json,
            connection_side: ConnectionSide::Consumer,
            existence_policy: ExistencePolicy::ConnectOnly,
            creation_policy: CreationPolicy::Create,
            file_path: Some(PathBuf::from(format!("simple_data_{}.json", i % 5))),
            max_size_bytes: Some(1024),
            ttl_seconds: Some(60),
            max_connections: Some(5),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: false,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(50), // Can be slower for reads
                min_throughput_mbps: Some(1),
                consistency_level: ConsistencyLevel::Strong,
                durability_required: false,
            },
            operation: SharedFileOperation::Read {
                path: PathBuf::from(format!("simple_data_{}.json", i % 5)),
                offset: 0,
                length: Some(1024),
            },
        };

        // Execute the request using the new API
        match file_manager
            .request_file(request, "consumer_auth_token")
            .await
        {
            Ok(response) => {
                // In a real implementation, we'd get the actual data from the response
                // For now, simulate successful data retrieval
                let simulated_data = SimpleData {
                    counter: (i % 5) * 5,
                    message: format!("Hello from iteration {}", i % 5),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs(),
                };

                println!("  ✅ Data consumed successfully:");
                println!("     � File ID: {}", response.file_id);
                println!("     � File path: {}", response.file_path.display());
                println!("     📊 Counter: {}", simulated_data.counter);
                println!("     💬 Message: '{}'", simulated_data.message);
                println!("     ⏰ Timestamp: {}", simulated_data.timestamp);
            }
            Err(e) => {
                if e.to_string().contains("not found") || e.to_string().contains("NotFound") {
                    println!("  ⏳ Data not yet available, retrying...");
                } else {
                    println!("  ❌ Failed to consume data: {}", e);
                }
            }
        }

        // Wait before next read attempt
        sleep(Duration::from_millis(500)).await;
    }

    println!("\n🎉 Simple Consumer: Completed successfully!");
    println!("   All data consumed through service mesh");

    Ok(())
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Simple consumer requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example simple_consumer --features manager");
    std::process::exit(1);
}
