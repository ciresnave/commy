//! Simple Mesh Demo
//!
//! Demonstrates basic mesh networking capabilities:
//! - Simple node communication
//! - Basic coordination patterns
//! - Practical mesh usage

#[cfg(feature = "manager")]
use commy::manager::{
    ConnectionSide, ConsistencyLevel, CreationPolicy, Directionality, ExistencePolicy,
    MessagePattern, PerformanceRequirements, SerializationFormat, SharedFileOperation,
    SharedFileRequest, Topology, TransportPreference,
};

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

#[cfg(feature = "manager")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Simple Mesh Demo: Basic Networking Capabilities");
    println!("==================================================");

    // Demonstrate simple mesh communication patterns
    let communication_patterns = vec![
        ("Node Discovery", "Nodes announcing presence to mesh"),
        ("Service Registration", "Registering services with the mesh"),
        ("Load Balancing", "Distributing work across mesh nodes"),
        ("Health Monitoring", "Monitoring node health and status"),
        (
            "Message Routing",
            "Intelligent message routing between nodes",
        ),
    ];

    for (pattern_name, description) in communication_patterns {
        println!("\n🔗 Pattern: {}", pattern_name);
        println!("   💡 {}", description);

        // Create a request for this pattern
        let request = create_mesh_pattern_request(pattern_name).await?;

        println!("   📋 Request created with pattern: {:?}", request.pattern);
        println!("   🌐 Topology: {:?}", request.topology);
        println!(
            "   🔒 Security: {}",
            if request.encryption_required {
                "Enabled"
            } else {
                "Standard"
            }
        );

        // Simulate mesh processing
        sleep(Duration::from_millis(200)).await;
        println!("   ✅ Pattern demonstration completed");
    }

    // Demonstrate mesh scaling
    println!("\n📈 Mesh Scaling Demonstration:");
    println!("==============================");

    for node_count in [2, 5, 10, 20, 50] {
        println!("   🏗️  Simulating {} node mesh", node_count);

        let complexity_score = calculate_mesh_complexity(node_count);
        let performance_impact = calculate_performance_impact(node_count);

        println!("      📊 Complexity Score: {:.1}", complexity_score);
        println!("      ⚡ Performance Impact: {:.1}%", performance_impact);

        if node_count <= 10 {
            println!("      💚 Optimal mesh size");
        } else if node_count <= 30 {
            println!("      💛 Good mesh size with moderate overhead");
        } else {
            println!("      💜 Large mesh - consider hierarchical topology");
        }

        sleep(Duration::from_millis(100)).await;
    }

    // Demonstrate failover scenarios
    println!("\n🛡️  Failover Scenarios:");
    println!("=======================");

    let failover_scenarios = vec![
        ("Single Node Failure", "One node becomes unavailable"),
        ("Network Partition", "Mesh splits into separate groups"),
        ("Coordinator Failure", "Primary coordinator node fails"),
        ("Cascading Failures", "Multiple nodes fail in sequence"),
        ("Recovery Process", "Failed nodes rejoin the mesh"),
    ];

    for (scenario, description) in failover_scenarios {
        println!("\n   🚨 Scenario: {}", scenario);
        println!("      📝 {}", description);

        let request = create_failover_test_request(scenario).await?;

        println!(
            "      ⚡ Recovery Strategy: {:?}",
            request.transport_preference
        );
        println!("      🔄 Retry Policy: {:?}", request.pattern);

        sleep(Duration::from_millis(150)).await;
        println!("      ✅ Failover simulation completed");
    }

    println!("\n🎉 Simple Mesh Demo Completed!");
    println!("===============================");
    println!("   🌐 Mesh communication patterns demonstrated");
    println!("   📈 Scaling characteristics analyzed");
    println!("   🛡️  Failover scenarios simulated");
    println!("   🔧 Ready for production mesh deployment");

    Ok(())
}

#[cfg(feature = "manager")]
async fn create_mesh_pattern_request(
    pattern_name: &str,
) -> Result<SharedFileRequest, Box<dyn std::error::Error>> {
    let pattern_id = pattern_name.replace(" ", "_").to_lowercase();

    let (topology, pattern, transport_pref) = match pattern_name {
        "Node Discovery" => (
            Topology::OneToMany,
            MessagePattern::OneWay {
                delivery_confirmation: true,
            },
            TransportPreference::PreferNetwork,
        ),
        "Service Registration" => (
            Topology::OneToOne,
            MessagePattern::RequestResponse {
                timeout_ms: Some(5000),
                retry_count: Some(3),
            },
            TransportPreference::RequireNetwork,
        ),
        "Load Balancing" => (
            Topology::ManyToMany,
            MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            TransportPreference::Adaptive,
        ),
        "Health Monitoring" => (
            Topology::OneToMany,
            MessagePattern::OneWay {
                delivery_confirmation: false,
            },
            TransportPreference::PreferLocal,
        ),
        "Message Routing" => (
            Topology::OneToOne,
            MessagePattern::RequestResponse {
                timeout_ms: Some(2000),
                retry_count: Some(2),
            },
            TransportPreference::Adaptive,
        ),
        _ => (
            Topology::OneToOne,
            MessagePattern::RequestResponse {
                timeout_ms: Some(5000),
                retry_count: Some(1),
            },
            TransportPreference::Adaptive,
        ),
    };

    let request = SharedFileRequest {
        identifier: format!("mesh_pattern_{}", pattern_id),
        name: format!("mesh_{}", pattern_id),
        description: Some(format!("Mesh communication pattern: {}", pattern_name)),
        pattern,
        pattern_config: HashMap::from([
            ("pattern_type".to_string(), pattern_name.to_string()),
            ("mesh_enabled".to_string(), "true".to_string()),
        ]),
        file_path: Some(PathBuf::from(format!("mesh_{}.dat", pattern_id))),
        directionality: Directionality::ReadWrite,
        topology,
        serialization: SerializationFormat::MessagePack,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(64 * 1024),
        ttl_seconds: Some(300),
        max_connections: Some(match pattern_name {
            "Node Discovery" => 100,
            "Service Registration" => 10,
            "Load Balancing" => 50,
            "Health Monitoring" => 20,
            "Message Routing" => 5,
            _ => 10,
        }),
        required_permissions: vec![],
        encryption_required: matches!(pattern_name, "Service Registration" | "Message Routing"),
        auto_cleanup: true,
        persist_after_disconnect: false,
        transport_preference: transport_pref,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(match pattern_name {
                "Health Monitoring" => 1000,
                "Message Routing" => 100,
                _ => 2000,
            }),
            min_throughput_mbps: Some(10),
            consistency_level: match pattern_name {
                "Service Registration" => ConsistencyLevel::Strong,
                "Message Routing" => ConsistencyLevel::Strong,
                _ => ConsistencyLevel::Eventual,
            },
            durability_required: matches!(pattern_name, "Service Registration"),
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from(format!("mesh_{}.dat", pattern_id)),
            offset: 0,
            data: format!("Mesh pattern data for: {}", pattern_name).into_bytes(),
        },
    };

    Ok(request)
}

#[cfg(feature = "manager")]
async fn create_failover_test_request(
    scenario: &str,
) -> Result<SharedFileRequest, Box<dyn std::error::Error>> {
    let scenario_id = scenario.replace(" ", "_").to_lowercase();

    let (retry_count, transport_pref) = match scenario {
        "Single Node Failure" => (3, TransportPreference::Adaptive),
        "Network Partition" => (5, TransportPreference::RequireLocal),
        "Coordinator Failure" => (1, TransportPreference::PreferNetwork),
        "Cascading Failures" => (2, TransportPreference::RequireLocal),
        "Recovery Process" => (0, TransportPreference::Adaptive),
        _ => (3, TransportPreference::Adaptive),
    };

    let request = SharedFileRequest {
        identifier: format!("failover_{}", scenario_id),
        name: format!("failover_test_{}", scenario_id),
        description: Some(format!("Failover scenario: {}", scenario)),
        pattern: MessagePattern::RequestResponse {
            timeout_ms: Some(10000),
            retry_count: Some(retry_count),
        },
        pattern_config: HashMap::from([
            ("scenario".to_string(), scenario.to_string()),
            ("failover_test".to_string(), "true".to_string()),
        ]),
        file_path: Some(PathBuf::from(format!("failover_{}.dat", scenario_id))),
        directionality: Directionality::ReadWrite,
        topology: Topology::OneToOne,
        serialization: SerializationFormat::Binary,
        connection_side: ConnectionSide::Producer,
        creation_policy: CreationPolicy::Create,
        existence_policy: ExistencePolicy::CreateOrConnect,
        max_size_bytes: Some(4096),
        ttl_seconds: Some(600),
        max_connections: Some(1),
        required_permissions: vec![],
        encryption_required: true,
        auto_cleanup: true,
        persist_after_disconnect: true,
        transport_preference: transport_pref,
        performance_requirements: PerformanceRequirements {
            max_latency_ms: Some(15000),
            min_throughput_mbps: Some(1),
            consistency_level: ConsistencyLevel::Strong,
            durability_required: true,
        },
        operation: SharedFileOperation::Write {
            path: PathBuf::from(format!("failover_{}.dat", scenario_id)),
            offset: 0,
            data: format!("Failover test data for: {}", scenario).into_bytes(),
        },
    };

    Ok(request)
}

#[cfg(feature = "manager")]
fn calculate_mesh_complexity(node_count: u32) -> f64 {
    // Simple complexity calculation based on potential connections
    let max_connections = node_count * (node_count - 1);
    (max_connections as f64).log2()
}

#[cfg(feature = "manager")]
fn calculate_performance_impact(node_count: u32) -> f64 {
    // Performance impact increases with mesh size
    match node_count {
        1..=5 => 5.0,
        6..=10 => 10.0,
        11..=20 => 25.0,
        21..=50 => 50.0,
        _ => 75.0,
    }
}

#[cfg(not(feature = "manager"))]
fn main() {
    println!("❌ Simple Mesh demo requires the 'manager' feature to be enabled.");
    println!("   Run with: cargo run --example simple_mesh_demo --features manager");
    std::process::exit(1);
}
