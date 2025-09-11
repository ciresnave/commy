//! Zero-Copy Serialization Demo
//!
//! This example demonstrates why rkyv is essential for true zero-copy operations
//! over shared memory-mapped files. It shows the performance difference between
//! traditional serialization and zero-copy serialization.

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
use commy::serialization::*;
use std::time::Instant;

#[cfg(feature = "zerocopy")]
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

// For true zero-copy, we need to derive both serde and rkyv traits.
// Note: attribute macros like `rkyv::archive_attr(...)` were intentionally
// removed from this file to avoid unresolved attribute errors when the
// optional `rkyv` crate is not enabled. The crate's `rkyv` dependency is
// optional and only present when the `zerocopy` feature is enabled in Cargo.
//
// To re-enable additional rkyv-specific attributes (for example archive_attr
// derives), run the example with the `zerocopy` feature or update Cargo.toml
// to provide a guarded feature that ensures `rkyv` (and its attribute macros)
// are available to the compiler and editor.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "zerocopy", derive(Archive, RkyvSerialize, RkyvDeserialize))]
struct ComplexData {
    name: String,
    values: Vec<i32>,
    metadata: std::collections::HashMap<String, String>,
    nested: NestedData,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "zerocopy", derive(Archive, RkyvSerialize, RkyvDeserialize))]
struct NestedData {
    id: u64,
    scores: Vec<f64>,
    labels: Vec<String>,
}

fn create_test_data() -> ComplexData {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("version".to_string(), "1.0".to_string());
    metadata.insert("author".to_string(), "Commy".to_string());
    metadata.insert("type".to_string(), "performance_test".to_string());

    ComplexData {
        name: "Zero-Copy Performance Test Data".to_string(),
        values: (0..1000).collect(),
        metadata,
        nested: NestedData {
            id: 12345,
            scores: (0..500).map(|i| i as f64 * 1.5).collect(),
            labels: (0..100).map(|i| format!("label_{}", i)).collect(),
        },
    }
}

#[cfg(feature = "zerocopy")]
fn demo_zero_copy_serialization() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Zero-Copy Serialization Demo");
    println!("================================");

    let test_data = create_test_data();
    println!(
        "📊 Test data created with {} values",
        test_data.values.len()
    );

    // Benchmark zero-copy serialization
    let start = Instant::now();
    let zero_copy_data: ZeroCopyData<ComplexData> = ZeroCopyData::new(test_data.clone())?;
    let zero_copy_serialize_time = start.elapsed();

    let start = Instant::now();
    let recovered_data = zero_copy_data.get()?;
    let zero_copy_deserialize_time = start.elapsed();

    println!("⚡ Zero-Copy Results:");
    println!("   Serialize: {:?}", zero_copy_serialize_time);
    println!("   Deserialize: {:?}", zero_copy_deserialize_time);
    println!("   Buffer usage: {:?}", zero_copy_data.buffer_usage());
    println!("   Data integrity: {}", test_data == recovered_data);

    Ok(())
}

#[cfg(feature = "json")]
fn demo_traditional_serialization() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📝 Traditional JSON Serialization Demo");
    println!("======================================");

    let test_data = create_test_data();

    // Benchmark JSON serialization
    let start = Instant::now();
    let json_data: JsonData<ComplexData> = JsonData::new(test_data.clone())?;
    let json_serialize_time = start.elapsed();

    let start = Instant::now();
    let recovered_data = json_data.get()?;
    let json_deserialize_time = start.elapsed();

    println!("📄 JSON Results:");
    println!("   Serialize: {:?}", json_serialize_time);
    println!("   Deserialize: {:?}", json_deserialize_time);
    println!("   Buffer usage: {:?}", json_data.buffer_usage());
    println!("   Data integrity: {}", test_data == recovered_data);

    Ok(())
}

#[cfg(feature = "compact")]
fn demo_compact_serialization() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📦 Compact Binary Serialization Demo");
    println!("====================================");

    let test_data = create_test_data();

    // Benchmark compact serialization
    let start = Instant::now();
    let compact_data: CompactData<ComplexData> = CompactData::new(test_data.clone())?;
    let compact_serialize_time = start.elapsed();

    let start = Instant::now();
    let recovered_data = compact_data.get()?;
    let compact_deserialize_time = start.elapsed();

    println!("📦 Compact Results:");
    println!("   Serialize: {:?}", compact_serialize_time);
    println!("   Deserialize: {:?}", compact_deserialize_time);
    println!("   Buffer usage: {:?}", compact_data.buffer_usage());
    println!("   Data integrity: {}", test_data == recovered_data);

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Commy Serialization Performance Comparison");
    println!("==============================================");
    println!();
    println!("This demo shows why zero-copy serialization is crucial");
    println!("for high-performance shared memory communication.");
    println!();

    #[cfg(feature = "zerocopy")]
    demo_zero_copy_serialization()?;

    #[cfg(feature = "json")]
    demo_traditional_serialization()?;

    #[cfg(feature = "compact")]
    demo_compact_serialization()?;

    println!("\n🔍 Analysis:");
    println!("- Zero-copy serialization minimizes CPU overhead");
    println!("- Shared memory files can directly reference serialized data");
    println!("- Perfect for high-frequency IPC with large data structures");
    println!("- rkyv enables true zero-copy deserialization from memory maps");

    #[cfg(not(any(feature = "zerocopy", feature = "json", feature = "compact")))]
    {
        println!("⚠️  No serialization features enabled!");
        println!("   Run with: cargo run --example zero_copy_demo --features=all_formats");
    }

    Ok(())
}
