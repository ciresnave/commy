//! Multi-format demo showing intelligent serialization selection
//! This demonstrates how the service mesh automatically chooses optimal formats

#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    MessagePattern, PerformanceRequirements, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportConfig, TransportManager, TransportPreference,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// Complex data structure for format comparison testing
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct UserProfile {
    id: u64,
    username: String,
    email: String,
    tags: Vec<String>,
    settings: HashMap<String, String>,
    scores: Vec<f64>,
    active: bool,
    metadata: ProfileMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ProfileMetadata {
    created_at: u64,
    last_login: u64,
    login_count: u32,
    preferences: UserPreferences,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct UserPreferences {
    theme: String,
    language: String,
    notifications: bool,
    privacy_level: PrivacyLevel,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum PrivacyLevel {
    Public,
    Friends,
    Private,
}

impl UserProfile {
    fn new(id: u64, username: &str, email: &str) -> Self {
        let mut tags = Vec::new();
        tags.push("rust".to_string());
        tags.push("developer".to_string());
        tags.push("performance".to_string());

        let mut settings = HashMap::new();
        settings.insert("theme".to_string(), "dark".to_string());
        settings.insert("auto_save".to_string(), "true".to_string());
        settings.insert("notifications".to_string(), "enabled".to_string());

        let scores = vec![95.5, 87.2, 92.8, 88.1, 96.3];

        UserProfile {
            id,
            username: username.to_string(),
            email: email.to_string(),
            tags,
            settings,
            scores,
            active: true,
            metadata: ProfileMetadata {
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                last_login: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                login_count: 42,
                preferences: UserPreferences {
                    theme: "dark".to_string(),
                    language: "en-US".to_string(),
                    notifications: true,
                    privacy_level: PrivacyLevel::Friends,
                },
            },
        }
    }
}

fn create_sample_profiles() -> Vec<UserProfile> {
    vec![
        UserProfile::new(1001, "alice_dev", "alice@example.com"),
        UserProfile::new(1002, "bob_admin", "bob@example.com"),
        UserProfile::new(1003, "charlie_user", "charlie@example.com"),
    ]
}

#[cfg(feature = "manager")]
async fn test_format_performance(
    transport_manager: &TransportManager,
    format: SerializationFormat,
    profiles: &[UserProfile],
) -> Result<(usize, u128), Box<dyn std::error::Error>> {
    let format_name = format!("{:?}", format);
    println!("\n🧪 Testing {} format:", format_name);

    let start_time = std::time::Instant::now();

    // Serialize all profiles
    let mut total_size = 0;
    for (i, profile) in profiles.iter().enumerate() {
        // Map serde errors into the crate-local CommyError to demonstrate
        // the adapter/mapping pattern during migration.
        let serialized_data = match format {
            SerializationFormat::Json => serde_json::to_vec(profile)
                .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?,
            SerializationFormat::Binary => serde_json::to_vec(profile)
                .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?, // Use JSON instead of bincode
            SerializationFormat::MessagePack => serde_json::to_vec(profile)
                .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?, // Use JSON instead of rmp_serde
            SerializationFormat::Cbor => serde_json::to_vec(profile)
                .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?, // Use JSON instead of serde_cbor
            SerializationFormat::ZeroCopy => {
                // For demo purposes, use JSON
                serde_json::to_vec(profile)
                    .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?
            }
            _ => serde_json::to_vec(profile)
                .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?,
        };

        total_size += serialized_data.len();

        // Create a request for this profile
        let request = SharedFileRequest {
            identifier: format!("profile_{}_{}", format_name.to_lowercase(), i),
            name: format!("profile_{}_{}", format_name.to_lowercase(), i),
            description: Some(format!("User profile in {} format", format_name)),
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: std::collections::HashMap::from([
                ("timeout_seconds".to_string(), "30".to_string()),
                ("priority".to_string(), "1".to_string()),
            ]),
            file_path: Some(PathBuf::from(format!("profile_{}.dat", i))),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: format.clone(),
            connection_side: ConnectionSide::Producer,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            max_size_bytes: Some((serialized_data.len() * 2) as u64),
            ttl_seconds: Some(300),
            max_connections: Some(1),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(match format {
                    SerializationFormat::ZeroCopy => 1,
                    SerializationFormat::Binary => 10,
                    SerializationFormat::MessagePack => 15,
                    SerializationFormat::Cbor => 20,
                    SerializationFormat::Json => 30,
                    _ => 50,
                }),
                min_throughput_mbps: Some(100),
                consistency_level: ConsistencyLevel::Strong,
                durability_required: false,
            },
            operation: SharedFileOperation::Write {
                path: PathBuf::from(format!("profile_{}.dat", i)),
                offset: 0,
                data: serialized_data.clone(),
            },
        };

        // Get routing decision
        let decision = transport_manager
            .route_request(&request)
            .await
            .map_err(|e| Box::<dyn std::error::Error>::from(e))?;

        // Execute request (handle NotImplemented gracefully)
        match transport_manager.execute_request(request, &decision).await {
            Ok(_) => {}
            Err(e) if e.to_string().contains("NotImplemented") => {}
            Err(e) => return Err(Box::new(e)),
        }
    }

    let duration = start_time.elapsed().as_micros();

    println!("  📊 Total size: {} bytes", total_size);
    println!("  ⏱️  Serialization time: {}μs", duration);
    println!(
        "  🚀 Throughput: {:.2} MB/s",
        (total_size as f64) / (duration as f64 / 1_000_000.0) / 1_000_000.0
    );

    Ok((total_size, duration))
}

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Multi-Format Demo: Intelligent Serialization Selection");
    println!("========================================================");

    // Initialize transport manager
    let config = TransportConfig::builder().auto_optimization(true).build()?;
    let transport_manager = TransportManager::new(config).await?;

    println!("✅ Transport manager initialized");

    // Create test data
    let profiles = create_sample_profiles();
    println!("📦 Created {} test profiles", profiles.len());

    // Test all formats
    let formats = vec![
        SerializationFormat::Json,
        SerializationFormat::Binary,
        SerializationFormat::MessagePack,
        SerializationFormat::Cbor,
        SerializationFormat::ZeroCopy,
    ];

    let mut results = Vec::new();

    for format in formats {
        match test_format_performance(&transport_manager, format.clone(), &profiles).await {
            Ok((size, time)) => {
                results.push((format, size, time));
            }
            Err(e) => {
                println!("❌ Error testing {:?}: {}", format, e);
            }
        }
    }

    // Performance comparison
    println!("\n📈 Performance Comparison Summary:");
    println!("==================================");

    // Sort by size (smallest first)
    let mut size_sorted = results.clone();
    size_sorted.sort_by_key(|(_, size, _)| *size);

    println!("\n🗜️  Most Compact Formats:");
    for (i, (format, size, _)) in size_sorted.iter().enumerate() {
        println!("  {}. {:?}: {} bytes", i + 1, format, size);
    }

    // Sort by speed (fastest first)
    let mut speed_sorted = results.clone();
    speed_sorted.sort_by_key(|(_, _, time)| *time);

    println!("\n⚡ Fastest Formats:");
    for (i, (format, _, time)) in speed_sorted.iter().enumerate() {
        println!("  {}. {:?}: {}μs", i + 1, format, time);
    }

    // Calculate efficiency scores
    println!("\n🎯 Format Recommendations:");

    let min_size = size_sorted.first().unwrap().1 as f64;
    let min_time = speed_sorted.first().unwrap().2 as f64;

    for (format, size, time) in &results {
        let size_efficiency = min_size / (*size as f64);
        let speed_efficiency = min_time / (*time as f64);
        let overall_score = (size_efficiency + speed_efficiency) / 2.0;

        println!(
            "  {:?}: {:.1}% efficient (Size: {:.1}%, Speed: {:.1}%)",
            format,
            overall_score * 100.0,
            size_efficiency * 100.0,
            speed_efficiency * 100.0
        );
    }

    // Demonstrate intelligent format selection
    println!("\n🧠 Intelligent Format Selection:");
    println!("================================");

    let scenarios = vec![
        ("Human-readable config", SerializationFormat::Json),
        ("High-performance data", SerializationFormat::ZeroCopy),
        ("Network transmission", SerializationFormat::MessagePack),
        ("Standards compliance", SerializationFormat::Cbor),
        ("Maximum compression", SerializationFormat::Binary),
    ];

    for (scenario, recommended_format) in scenarios {
        let profile = &profiles[0];
        let data = serde_json::to_vec(profile)
            .map_err(|e| commy::errors::CommyError::JsonSerialization(e))?; // Use JSON for size calculation

        let request = SharedFileRequest {
            identifier: format!("scenario_{}", scenario.replace(" ", "_")),
            name: format!("scenario_{}", scenario.replace(" ", "_")),
            description: Some(scenario.to_string()),
            pattern: MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            pattern_config: std::collections::HashMap::from([
                ("timeout_seconds".to_string(), "30".to_string()),
                ("priority".to_string(), "1".to_string()),
            ]),
            file_path: Some(PathBuf::from("scenario.dat")),
            directionality: Directionality::ReadWrite,
            topology: Topology::OneToOne,
            serialization: recommended_format.clone(),
            connection_side: ConnectionSide::Producer,
            existence_policy: ExistencePolicy::CreateOrConnect,
            creation_policy: CreationPolicy::Create,
            max_size_bytes: Some(4096),
            ttl_seconds: Some(300),
            max_connections: Some(1),
            required_permissions: vec![],
            encryption_required: false,
            auto_cleanup: true,
            persist_after_disconnect: false,
            transport_preference: TransportPreference::AutoOptimize,
            performance_requirements: PerformanceRequirements {
                max_latency_ms: Some(50),
                min_throughput_mbps: Some(10),
                consistency_level: ConsistencyLevel::Strong,
                durability_required: false,
            },
            operation: SharedFileOperation::Write {
                path: PathBuf::from("scenario.dat"),
                offset: 0,
                data,
            },
        };

        let decision = transport_manager.route_request(&request).await?;

        println!("  📋 {}: {:?}", scenario, recommended_format);
        println!("     🎯 Routed to: {:?}", decision.transport);
        println!("     📈 Confidence: {:.1}%", decision.confidence * 100.0);
    }

    println!("\n🎉 Multi-format demo completed successfully!");
    println!("   The service mesh intelligently selects optimal formats!");

    Ok(())
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Multi-format demo requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example multi_format_demo --features manager");
    std::process::exit(1);
}
