// Top-level crate cfg removed so the example always compiles; individual
// behavior is controlled by per-main cfgs below. This prevents clippy from
// failing with `main` missing (E0601) when optional features are disabled.

//! Zero-Copy Serialization Demo
//!
//! This example demonstrates the performance benefits of the rkyv zero-copy serialization
//! backend compared to traditional JSON serialization for shared memory operations.

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use commy::serialization::*;

// Provide a fallback main when none of the formats are enabled to avoid
// clippy/E0601 errors. The real demo runs when features are enabled.

// When features are enabled, the example main below will execute the demo.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Dispatch demos based on enabled features. If no features enabled, print
    // an informative message. This keeps the example file compilable under
    // all feature combinations and avoids clippy E0601.

    #[cfg(any(feature = "zerocopy", feature = "json", feature = "compact"))]
    {
        run_demo()?;
    }

    #[cfg(not(any(feature = "zerocopy", feature = "json", feature = "compact")))]
    {
        println!("Example 'simple_zero_copy_demo' disabled: run with --features=all_formats");
    }

    Ok(())
}
use std::time::Instant;

// Complex data structure for performance testing
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct ComplexData {
    name: String,
    values: Vec<i32>,
    metadata: std::collections::HashMap<String, String>,
    nested: NestedData,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
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
            scores: (0..500)
                .map(|i: usize| (i as f64) * 1.5)
                .collect::<Vec<f64>>(),
            labels: (0..100).map(|i| format!("label_{}", i)).collect(),
        },
    }
}

fn benchmark_serialization<T: SerializationBackend>(
    name: &str,
    test_data: &ComplexData,
    iterations: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔬 Benchmarking {} Serialization", name);
    println!("{}", "=".repeat(40));

    let mut total_serialize_time = std::time::Duration::new(0, 0);
    let mut total_deserialize_time = std::time::Duration::new(0, 0);
    let mut serialized_size = 0;

    for _ in 0..iterations {
        // Serialize
        let start = Instant::now();
        let serialized = T::serialize(test_data)?;
        total_serialize_time += start.elapsed();

        if serialized_size == 0 {
            serialized_size = serialized.len();
        }

        // Deserialize
        let start = Instant::now();
        let _recovered: ComplexData = T::deserialize(&serialized)?;
        total_deserialize_time += start.elapsed();
    }

    let avg_serialize_time = total_serialize_time / iterations as u32;
    let avg_deserialize_time = total_deserialize_time / iterations as u32;

    println!("📊 {} Results (avg over {} iterations):", name, iterations);
    println!("   Serialize: {:?}", avg_serialize_time);
    println!("   Deserialize: {:?}", avg_deserialize_time);
    println!("   Total: {:?}", avg_serialize_time + avg_deserialize_time);
    println!("   Serialized size: {} bytes", serialized_size);

    Ok(())
}

fn run_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Commy Serialization Performance Comparison");
    println!("==============================================");
    println!();
    println!("This demo compares different serialization backends");
    println!("to show the performance benefits of zero-copy serialization.");
    println!();

    let test_data = create_test_data();
    let _iterations = 1000;

    println!("📋 Test data contains:");
    println!("   - {} integer values", test_data.values.len());
    println!("   - {} float scores", test_data.nested.scores.len());
    println!("   - {} string labels", test_data.nested.labels.len());
    println!("   - {} metadata entries", test_data.metadata.len());

    // Benchmark JSON serialization
    #[cfg(feature = "json")]
    {
        use commy::serialization::JsonBackend;
        benchmark_serialization::<JsonBackend>("JSON", &test_data, _iterations)?;
    }

    // Benchmark Binary serialization
    #[cfg(feature = "binary")]
    {
        use commy::serialization::BinaryBackend;
        benchmark_serialization::<BinaryBackend>("Binary", &test_data, _iterations)?;
    }

    // Benchmark MessagePack serialization
    #[cfg(feature = "messagepack")]
    {
        use commy::serialization::MessagePackBackend;
        benchmark_serialization::<MessagePackBackend>("MessagePack", &test_data, _iterations)?;
    }

    // Benchmark Compact serialization
    #[cfg(feature = "compact")]
    {
        use commy::serialization::CompactBackend;
        benchmark_serialization::<CompactBackend>("Compact", &test_data, _iterations)?;
    }

    // Benchmark Zero-Copy serialization (currently uses serde bridge)
    #[cfg(feature = "zerocopy")]
    {
        use commy::serialization::RkyvZeroCopyBackend;
        benchmark_serialization::<RkyvZeroCopyBackend>(
            "ZeroCopy (serde bridge)",
            &test_data,
            _iterations,
        )?;

        println!("\n📝 Note: ZeroCopy backend currently uses a serde bridge.");
        println!("   Full rkyv zero-copy implementation would be significantly faster!");
    }

    #[cfg(not(feature = "zerocopy"))]
    {
        println!("\n⚠️  ZeroCopy backend not available (zerocopy feature disabled)");
        println!("   Run with: cargo run --example simple_zero_copy_demo --features zerocopy");
    }

    println!("\n🔍 Analysis:");
    println!("- Binary and MessagePack are typically faster than JSON");
    println!("- Compact format provides good compression");
    println!("- Zero-copy (rkyv) would provide the best performance for shared memory");
    println!("- Memory-mapped files benefit greatly from zero-copy deserialization");

    Ok(())
}
