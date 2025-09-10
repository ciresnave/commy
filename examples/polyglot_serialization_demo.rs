//! Polyglot Serialization Strategy Demo
//!
//! This example demonstrates Commy's strategic approach to supporting both
//! Rust-optimized (rkyv) and polyglot (Cap'n Proto) serialization formats
//! for different use cases in a multi-language service mesh.

// Top-level crate cfg removed so the example always compiles; individual
// behavior is controlled by the per-main cfgs below. This prevents clippy
// from failing with `main` missing (E0601) when optional features are disabled.

use commy::serialization::*;
use std::time::Instant;

// Test data structure compatible with multiple serialization formats
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct ServiceData {
    service_id: String,
    node_id: String,
    host: String,
    port: u16,
    tags: Vec<String>,
    metadata: std::collections::HashMap<String, String>,
    health_metrics: HealthMetrics,
    last_seen: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct HealthMetrics {
    cpu_usage: f64,
    memory_mb: u64,
    network_latency_ms: f64,
    request_rate: f64,
    error_rate: f64,
}

fn create_service_mesh_data() -> ServiceData {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("version".to_string(), "1.2.3".to_string());
    metadata.insert("environment".to_string(), "production".to_string());
    metadata.insert("team".to_string(), "platform".to_string());
    metadata.insert("region".to_string(), "us-west-2".to_string());

    ServiceData {
        service_id: "user-service-001".to_string(),
        node_id: "node-worker-42".to_string(),
        host: "10.0.1.42".to_string(),
        port: 8080,
        tags: vec![
            "api".to_string(),
            "user-management".to_string(),
            "critical".to_string(),
            "scalable".to_string(),
        ],
        metadata,
        health_metrics: HealthMetrics {
            cpu_usage: 65.3,
            memory_mb: 2048,
            network_latency_ms: 12.5,
            request_rate: 1250.0,
            error_rate: 0.01,
        },
        last_seen: 1693574400, // Unix timestamp
    }
}

fn benchmark_format<T: SerializationBackend>(
    name: &str,
    data: &ServiceData,
    iterations: usize,
    use_case: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔬 {} Serialization Benchmark", name);
    println!("   Use Case: {}", use_case);
    println!("   {}", "=".repeat(60));

    let mut total_serialize_time = std::time::Duration::new(0, 0);
    let mut total_deserialize_time = std::time::Duration::new(0, 0);
    let mut serialized_size = 0;

    for _ in 0..iterations {
        // Serialize
        let start = Instant::now();
        let serialized = T::serialize(data)?;
        total_serialize_time += start.elapsed();

        if serialized_size == 0 {
            serialized_size = serialized.len();
        }

        // Deserialize
        let start = Instant::now();
        let _recovered: ServiceData = T::deserialize(&serialized)?;
        total_deserialize_time += start.elapsed();
    }

    let avg_serialize_time = total_serialize_time / iterations as u32;
    let avg_deserialize_time = total_deserialize_time / iterations as u32;
    let total_time = avg_serialize_time + avg_deserialize_time;

    println!("   📊 Results (avg over {} iterations):", iterations);
    println!("      Serialize:   {:?}", avg_serialize_time);
    println!("      Deserialize: {:?}", avg_deserialize_time);
    println!("      Total:       {:?}", total_time);
    println!("      Size:        {} bytes", serialized_size);

    // Calculate throughput
    let throughput_mb_s = (serialized_size as f64 * iterations as f64)
        / (total_serialize_time.as_secs_f64() * 1024.0 * 1024.0);
    println!("      Throughput:  {:.2} MB/s", throughput_mb_s);

    Ok(())
}

fn print_polyglot_architecture() {
    println!("🏗️  Commy's Polyglot Serialization Architecture");
    println!("===============================================");
    println!();
    println!("📋 Strategy: Use the right tool for the right job");
    println!();

    println!("🦀 **rkyv (Rust Zero-Copy)**");
    println!("   ✅ Perfect for: Rust-to-Rust communication");
    println!("   ✅ Benefits: Maximum performance, true zero-copy");
    println!("   ✅ Use cases: Internal Rust services, hot paths");
    println!("   ❌ Limitation: Rust-only ecosystem");
    println!();

    println!("⚡ **Cap'n Proto (Polyglot Zero-Copy)**");
    println!("   ✅ Perfect for: Cross-language communication");
    println!("   ✅ Benefits: Zero-copy across languages, schema evolution");
    println!("   ✅ Use cases: Service mesh IPC, shared memory across SDKs");
    println!("   ✅ Languages: C++, Python, JavaScript, Go, Java, C#, Rust");
    println!("   ❌ Limitation: Requires schema compilation");
    println!();

    println!("🔄 **Traditional Formats (JSON, MessagePack, etc.)**");
    println!("   ✅ Perfect for: Legacy compatibility, debugging");
    println!("   ✅ Benefits: Human-readable (JSON), universal support");
    println!("   ✅ Use cases: API endpoints, configuration, development");
    println!("   ❌ Limitation: Copy-heavy, slower performance");
    println!();
}

fn print_use_case_matrix() {
    println!("📊 Use Case Decision Matrix");
    println!("===========================");
    println!();

    let use_cases = vec![
        ("Rust service → Rust service", "rkyv", "Maximum performance"),
        (
            "Python SDK → Node.js SDK",
            "Cap'n Proto",
            "Cross-language zero-copy",
        ),
        (
            "Shared memory files",
            "Cap'n Proto",
            "Multi-language access",
        ),
        ("REST API responses", "JSON", "Web compatibility"),
        ("Configuration files", "JSON", "Human readable"),
        ("High-frequency telemetry", "rkyv", "Zero-copy performance"),
        ("Service discovery data", "Cap'n Proto", "Schema evolution"),
        ("Debug/development", "JSON", "Inspection/debugging"),
        (
            "Compact network payloads",
            "MessagePack/Compact",
            "Size optimization",
        ),
        ("FFI data exchange", "Cap'n Proto", "ABI stability"),
    ];

    for (scenario, format, reason) in use_cases {
        println!("🎯 {:<25} → {:<15} ({})", scenario, format, reason);
    }
    println!();
}

fn print_language_support() {
    println!("🌍 Multi-Language Support Matrix");
    println!("=================================");
    println!();

    let languages = vec![
        ("Rust", "✅ rkyv", "✅ Cap'n Proto", "✅ All formats"),
        ("Python", "❌ rkyv", "✅ Cap'n Proto", "✅ JSON/MessagePack"),
        (
            "JavaScript/Node.js",
            "❌ rkyv",
            "✅ Cap'n Proto",
            "✅ JSON/MessagePack",
        ),
        ("Go", "❌ rkyv", "✅ Cap'n Proto", "✅ JSON/MessagePack"),
        ("Java", "❌ rkyv", "✅ Cap'n Proto", "✅ JSON/MessagePack"),
        (
            "C#/.NET",
            "❌ rkyv",
            "✅ Cap'n Proto",
            "✅ JSON/MessagePack",
        ),
        ("C/C++", "❌ rkyv", "✅ Cap'n Proto", "✅ JSON/MessagePack"),
        ("Browser/WASM", "❌ rkyv", "✅ Cap'n Proto", "✅ JSON"),
    ];

    println!(
        "{:<15} {:<15} {:<18} {}",
        "Language", "rkyv", "Cap'n Proto", "Traditional"
    );
    println!("{}", "-".repeat(70));

    for (lang, rkyv, capnp, trad) in languages {
        println!("{:<15} {:<15} {:<18} {}", lang, rkyv, capnp, trad);
    }
    println!();
}

// Provide a fallback main when none of the serialization features are enabled so
// clippy and other tooling don't fail with `main` missing (E0601). The real
// example main runs when features are present.
#[cfg(not(any(
    feature = "json",
    feature = "messagepack",
    feature = "compact",
    feature = "zerocopy",
    feature = "capnproto"
)))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 'polyglot_serialization_demo' disabled: run with --features=all_formats");
    Ok(())
}

#[cfg(any(
    feature = "json",
    feature = "messagepack",
    feature = "compact",
    feature = "zerocopy",
    feature = "capnproto",
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_polyglot_architecture();
    print_use_case_matrix();
    print_language_support();

    println!("🔬 Performance Benchmarks");
    println!("=========================");

    let service_data = create_service_mesh_data();
    let iterations = 1000;

    println!("\n📋 Test data: Service mesh discovery record");
    println!("   - Service ID: {}", service_data.service_id);
    println!(
        "   - {} tags, {} metadata entries",
        service_data.tags.len(),
        service_data.metadata.len()
    );
    println!("   - Health metrics included");

    // JSON - Web/API compatibility
    benchmark_format::<JsonBackend>(
        "JSON",
        &service_data,
        iterations,
        "Web APIs, debugging, human-readable config",
    )?;

    // MessagePack - Efficient traditional format
    benchmark_format::<MessagePackBackend>(
        "MessagePack",
        &service_data,
        iterations,
        "Compact binary, language-agnostic RPC",
    )?;

    // Compact - Size-optimized
    benchmark_format::<CompactBackend>(
        "Compact",
        &service_data,
        iterations,
        "Size-critical scenarios, embedded systems",
    )?;

    // rkyv - Rust zero-copy
    #[cfg(feature = "zerocopy")]
    {
        use commy::serialization::RkyvZeroCopyBackend;
        benchmark_format::<RkyvZeroCopyBackend>(
            "rkyv (Zero-Copy)",
            &service_data,
            iterations,
            "Rust-to-Rust high-performance communication",
        )?;
    }

    // Cap'n Proto - Polyglot zero-copy
    #[cfg(feature = "capnproto")]
    {
        use commy::serialization::CapnProtoBackend;
        benchmark_format::<CapnProtoBackend>(
            "Cap'n Proto",
            &service_data,
            iterations,
            "Cross-language zero-copy, schema evolution",
        )?;
    }

    println!("\n🎯 Strategic Recommendations");
    println!("============================");
    println!();
    println!("🏆 **For Maximum Performance (Rust-only)**:");
    println!("   → Use rkyv for Rust service-to-service communication");
    println!();
    println!("🌍 **For Polyglot Service Mesh**:");
    println!("   → Use Cap'n Proto for cross-language shared memory");
    println!("   → Enables zero-copy across Python, Node.js, Go, Java SDKs");
    println!();
    println!("🔧 **For Development & APIs**:");
    println!("   → JSON for REST APIs and human-readable data");
    println!("   → MessagePack for efficient RPC where schema evolution not needed");
    println!();
    println!("💡 **Commy's Advantage**:");
    println!("   → Unified interface supports all formats");
    println!("   → Choose the right tool for each use case");
    println!("   → Seamless integration across the entire tech stack");

    #[cfg(not(any(feature = "zerocopy", feature = "capnproto")))]
    {
        println!("\n⚠️  Advanced serialization features disabled");
        println!(
            "   Run with: cargo run --example polyglot_serialization_demo --features=all_formats"
        );
    }

    Ok(())
}
