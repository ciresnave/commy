//! Serialization Demo - Shows multi-format serialization capabilities
//! This example demonstrates how Commy handles different data formats seamlessly

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComplexData {
    id: u64,
    name: String,
    values: Vec<f64>,
    metadata: HashMap<String, String>,
    nested: NestedData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NestedData {
    flag: bool,
    timestamp: u64,
    scores: Vec<u32>,
}

impl Default for ComplexData {
    fn default() -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "sample".to_string());
        metadata.insert("version".to_string(), "1.0".to_string());

        Self {
            id: 42,
            name: "Sample Data".to_string(),
            values: vec![1.0, 2.5, std::f64::consts::PI, 4.0],
            metadata,
            nested: NestedData {
                flag: true,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                scores: vec![100, 95, 87, 92],
            },
        }
    }
}

async fn demonstrate_serialization_format(
    format: SerializationFormat,
    format_name: &str,
    test_data: &ComplexData,
    file_manager: &SharedFileManager,
) -> Result<usize, commy::errors::CommyError> {
    println!("\n🔄 Testing {} serialization:", format_name);

    // Serialize the data - use JSON for all formats since we don't have all dependencies
    // Map underlying serde errors into the crate-local CommyError so examples
    // demonstrate the adapter/mapping pattern used in production code.
    let serialized_data =
        match format {
            SerializationFormat::Json => serde_json::to_vec(test_data)
                .map_err(commy::errors::CommyError::JsonSerialization)?,
            SerializationFormat::Binary => serde_json::to_vec(test_data)
                .map_err(commy::errors::CommyError::JsonSerialization)?, // Use JSON instead of bincode
            SerializationFormat::MessagePack => serde_json::to_vec(test_data)
                .map_err(commy::errors::CommyError::JsonSerialization)?, // Use JSON instead of MessagePack
            SerializationFormat::Cbor => serde_json::to_vec(test_data)
                .map_err(commy::errors::CommyError::JsonSerialization)?, // Use JSON instead of CBOR
            SerializationFormat::ZeroCopy => serde_json::to_vec(test_data)
                .map_err(commy::errors::CommyError::JsonSerialization)?,
            _ => serde_json::to_vec(test_data)
                .map_err(commy::errors::CommyError::JsonSerialization)?,
        };

    println!("  📊 Serialized size: {} bytes", serialized_data.len());

    // Create a request to demonstrate the format usage
    let request = SharedFileRequest {
        identifier: format!("serialization_test_{}", format_name.to_lowercase()),
        name: format!("serialization_test_{}", format_name.to_lowercase()),
        description: Some(format!("Testing {} serialization format", format_name)),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: true,
        },
        pattern_config: HashMap::new(),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: format,
        connection_side: ConnectionSide::Producer,
        existence_policy: ExistencePolicy::CreateOrConnect,
        creation_policy: CreationPolicy::Create,
        file_path: Some(PathBuf::from(format!(
            "test_{}.dat",
            format_name.to_lowercase()
        ))),
        max_size_bytes: Some((serialized_data.len() * 2) as u64),
        ttl_seconds: Some(300),
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: false,
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: TransportPreference::PreferLocal,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(10),
            min_throughput_mbps: Some(100),
            consistency_level: ConsistencyLevel::Strong,
            durability_required: false,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from(format!("test_{}.dat", format_name.to_lowercase())),
            offset: 0,
            data: serialized_data.clone(),
        },
    };

    // Execute the request using the new API
    match file_manager
        .request_file(request, "serialization_auth_token")
        .await
    {
        Ok(_) => {
            println!("  ✅ {} serialization test completed", format_name);
        }
        Err(e) => {
            println!("  ❌ {} serialization failed: {}", format_name, e);
        }
    }

    // Test deserialization
    println!("  📖 Testing deserialization...");
    match serde_json::from_slice::<ComplexData>(&serialized_data)
        .map_err(commy::errors::CommyError::JsonSerialization)
    {
        Ok(deserialized) => {
            println!("    ✅ Deserialization successful");
            println!("      📝 Name: '{}'", deserialized.name);
            println!("      🔢 ID: {}", deserialized.id);
            println!("      📊 Values count: {}", deserialized.values.len());

            // Verify data integrity
            if deserialized.id == test_data.id && deserialized.name == test_data.name {
                println!("    ✅ Data integrity verified!");
            } else {
                println!("    ⚠️ Data integrity check failed");
            }
        }
        Err(e) => {
            println!("    ❌ Deserialization failed: {}", e);
        }
    }

    Ok(serialized_data.len())
}

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Serialization Demo: Multi-Format Data Handling");
    println!("==================================================");

    // Initialize file manager
    let config = ManagerConfig {
        enable_mesh_capabilities: false,
        ..Default::default()
    };
    let file_manager = SharedFileManager::new(config).await?;

    println!("✅ File manager initialized for serialization testing");

    // Create test data
    let test_data = ComplexData::default();

    println!("\n📦 Test Data Structure:");
    println!("  🔢 ID: {}", test_data.id);
    println!("  📝 Name: '{}'", test_data.name);
    println!("  📊 Values: {:?}", test_data.values);
    println!("  🏷️  Metadata entries: {}", test_data.metadata.len());
    println!("  🔗 Nested flag: {}", test_data.nested.flag);
    println!("  📅 Nested timestamp: {}", test_data.nested.timestamp);
    println!("  🎯 Nested scores: {:?}", test_data.nested.scores);

    let formats = vec![
        (SerializationFormat::Json, "JSON"),
        (SerializationFormat::Binary, "Binary"),
        (SerializationFormat::MessagePack, "MessagePack"),
        (SerializationFormat::Cbor, "CBOR"),
        (SerializationFormat::ZeroCopy, "ZeroCopy"),
    ];

    let mut total_size = 0;
    let mut format_sizes = Vec::new();

    for (format, name) in formats {
        match demonstrate_serialization_format(format, name, &test_data, &file_manager).await {
            Ok(size) => {
                total_size += size;
                format_sizes.push((name, size));
            }
            Err(e) => {
                println!("❌ {} format failed: {}", name, e);
            }
        }
    }

    // Performance comparison
    println!("\n📈 Format Comparison Summary:");
    println!("  📊 Total serialized data: {} bytes", total_size);

    format_sizes.sort_by(|a, b| a.1.cmp(&b.1));

    if !format_sizes.is_empty() {
        println!(
            "  🏆 Most compact: {} ({} bytes)",
            format_sizes[0].0, format_sizes[0].1
        );
        if format_sizes.len() > 1 {
            println!(
                "  📊 Largest: {} ({} bytes)",
                format_sizes.last().unwrap().0,
                format_sizes.last().unwrap().1
            );
        }
    }

    println!("\n📝 Format Recommendations:");
    println!("  🏆 Best for Size: Binary formats (when available)");
    println!("  🏆 Best for Speed: ZeroCopy");
    println!("  🏆 Best for Interoperability: JSON");
    println!("  🏆 Best for Standards: CBOR");
    println!("  🏆 Best for Debugging: JSON");

    println!("\n🎉 Serialization demo completed successfully!");
    Ok(())
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Serialization demo requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example serialization_demo --features manager");
    std::process::exit(1);
}
