//! Minimal working example - demonstrates core Commy service mesh functionality
//! This example shows the simplest way to use the modern Commy API

#[cfg(feature = "manager")]
use commy::manager::core::{ManagerConfig, SharedFileManager};
#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, CreationPolicy, Directionality, ExistencePolicy, MessagePattern,
    PerformanceRequirements, SerializationFormat, SharedFileOperation, SharedFileRequest, Topology,
    TransportPreference,
};

use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Minimal Commy Example: Starting...");

    // Create the simplest possible configuration
    let config = ManagerConfig {
        enable_mesh_capabilities: false,
        ..Default::default()
    };
    let file_manager = SharedFileManager::new(config).await?;

    println!("✅ File manager created successfully");

    // Create a minimal shared file request
    let request = SharedFileRequest {
        identifier: "minimal_test".to_string(),
        name: "minimal_test".to_string(),
        description: Some("Minimal test file".to_string()),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: true,
        },
        pattern_config: HashMap::new(),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Json,
        connection_side: ConnectionSide::Agnostic,
        existence_policy: ExistencePolicy::CreateOrConnect,
        creation_policy: CreationPolicy::Create,
        file_path: Some(PathBuf::from("minimal_test.json")),
        max_size_bytes: Some(1024), // 1KB
        ttl_seconds: Some(300),     // 5 minutes
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::AutoOptimize,
        performance_requirements: PerformanceRequirements::default(),
        operation: SharedFileOperation::Write {
            path: PathBuf::from("minimal_test.json"),
            offset: 0,
            data: r#"{"message": "Hello from Commy!", "counter": 42}"#
                .as_bytes()
                .to_vec(),
        },
    };

    // Execute the request using the new API
    match file_manager
        .request_file(request, "example_auth_token")
        .await
    {
        Ok(response) => {
            println!("✅ Successfully allocated shared file!");
            println!("📁 File ID: {}", response.file_id);
            println!("📂 File path: {}", response.file_path.display());
            println!("🎉 Commy service mesh is working correctly!");
        }
        Err(e) => {
            println!("❌ Request failed: {}", e);
        }
    }

    println!("🏁 Minimal example completed successfully!");
    Ok(())
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Minimal example requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example minimal --features manager");
    std::process::exit(1);
}
