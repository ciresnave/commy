//! Simple producer - demonstrates basic service mesh functionality
//! This creates a simple service that produces data through the modern API

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
    println!("🚀 Simple Producer: Modern Service Mesh Demo");
    println!("============================================");

    // Initialize file manager
    let config = ManagerConfig {
        enable_mesh_capabilities: false,
        ..Default::default()
    };
    let file_manager = SharedFileManager::new(config).await?;

    println!("✅ File manager initialized");

    // Produce data for 5 iterations
    for i in 0..5 {
        let data = SimpleData {
            counter: i * 5,
            message: format!("Hello from iteration {}", i),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        println!(
            "📦 Producing: counter={}, message='{}'",
            data.counter, data.message
        );

        // Serialize the data
        // Map serde_json errors into crate-local CommyError (adapter pattern)
        let serialized_data = serde_json::to_vec(&data).map_err(|e| {
            Box::<dyn std::error::Error>::from(commy::errors::CommyError::JsonSerialization(e))
        })?;

        // Create a request for this data
        let request = SharedFileRequest {
            identifier: format!("simple_data_iteration_{}", i),
            name: format!("simple_data_iteration_{}", i),
            description: Some(format!("Simple producer data iteration {}", i)),
            pattern: MessagePattern::PublishSubscribe {
                topic: "simple_data".to_string(),
                durable: false,
                filter: None,
            },
            pattern_config: HashMap::new(),
            directionality: Directionality::WriteOnly,
            topology: Topology::OneToMany,
            serialization: SerializationFormat::Json,
            connection_side: ConnectionSide::Producer,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            file_path: Some(PathBuf::from(format!("simple_data_{}.json", i))),
            max_size_bytes: Some(1024), // Small data
            ttl_seconds: Some(60),      // Short-lived
            max_connections: Some(5),   // Multiple consumers
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(10), // Fast updates
                min_throughput_mbps: Some(1),
                consistency_level: ConsistencyLevel::Strong,
                durability_required: false,
            },
            operation: SharedFileOperation::Write {
                path: PathBuf::from(format!("simple_data_{}.json", i)),
                offset: 0,
                data: serialized_data.clone(),
            },
        };

        // Execute the request using the new API
        match file_manager
            .request_file(request, "producer_auth_token")
            .await
        {
            Ok(response) => {
                println!("  ✅ Data produced successfully");
                println!("  📁 File ID: {}", response.file_id);
                println!("  📂 File path: {}", response.file_path.display());
            }
            Err(e) => {
                println!("  ❌ Failed to produce data: {}", e);
            }
        }

        // Wait before next iteration
        sleep(Duration::from_secs(1)).await;
    }

    println!("🎉 Simple Producer: Completed successfully!");
    println!("   All data iterations produced through service mesh");

    Ok(())
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Simple producer requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example simple_producer --features manager");
    std::process::exit(1);
}
