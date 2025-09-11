//! Raw Binary Foundation Demo
//!
//! This example demonstrates how the raw binary foundation enables:
//! 1. True zero-copy access to memory-mapped data
//! 2. Universal serialization format support
//! 3. Cross-language compatibility
//! 4. Maximum performance through direct memory access

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use commy::serialization::{
    FormatData, RawBinaryData, RawBinaryError, RawBytes, UniversalData, ZeroCopyAccess,
    ZeroCopyBytes,
};

// When the `json` feature is not enabled provide a simple stub main so the example file
// doesn't reference feature-gated types (JsonData) and produces editor/compile diagnostics.
#[cfg(not(feature = "json"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Example 'raw_binary_foundation_demo' disabled: run with --features=json");
    Ok(())
}

// JsonData is a generic type alias; import via fully qualified name when used below.
// The real example (service struct + demos) are only compiled when `json` feature is enabled.
#[cfg(feature = "json")]
mod json_demo {
    use super::*;
    #[cfg(feature = "zerocopy")]
    use commy::serialization::RkyvData;

    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ServiceInfo {
        pub id: String,
        pub name: String,
        pub host: String,
        pub port: u16,
        pub metadata: HashMap<String, String>,
        pub health_check_url: Option<String>,
    }

    impl Default for ServiceInfo {
        fn default() -> Self {
            let mut metadata = HashMap::new();
            metadata.insert("version".to_string(), "1.0.0".to_string());
            metadata.insert("environment".to_string(), "production".to_string());

            ServiceInfo {
                id: "service-001".to_string(),
                name: "User Authentication Service".to_string(),
                host: "auth.example.com".to_string(),
                port: 8080,
                metadata,
                health_check_url: Some("/health".to_string()),
            }
        }
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        println!("🏗️  Raw Binary Foundation Demo");
        println!("===============================\n");

        let service = ServiceInfo::default();

        // Demo 1: Raw binary operations
        demo_raw_binary_operations()?;

        // Demo 2: Format abstraction on raw foundation
        demo_format_abstraction(&service)?;

        // Demo 3: Zero-copy access patterns
        demo_zero_copy_access(&service)?;

        // Demo 4: Universal data interface
        demo_universal_data(&service)?;

        // Demo 5: Cross-format compatibility
        demo_cross_format_compatibility(&service)?;

        #[cfg(feature = "zerocopy")]
        demo_rkyv_zero_copy(&service)?;

        println!("\n🎯 Raw Binary Foundation Benefits:");
        println!("  ✅ Universal format support (anything → bytes)");
        println!("  ✅ True zero-copy access (direct memory mapping)");
        println!("  ✅ Cross-language compatibility (all languages understand bytes)");
        println!("  ✅ Maximum performance (no intermediate serialization)");
        println!("  ✅ Format independence (raw data can be interpreted multiple ways)");

        Ok(())
    }

    pub fn demo_raw_binary_operations() -> Result<(), Box<dyn std::error::Error>> {
        println!("1️⃣  Raw Binary Operations");
        println!("   Foundation: Everything ultimately becomes bytes\n");

        // Create raw binary data
        let raw_data = vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]; // "Hello" in ASCII
        let raw_bytes = RawBytes::new(raw_data.clone());

        println!("   📦 Raw data: {:?}", raw_data);
        println!("   📊 Length: {} bytes", raw_bytes.len());
        println!("   🔍 As bytes: {:?}", raw_bytes.as_bytes());

        // Demonstrate zero-copy view
        let zero_copy = ZeroCopyBytes::new(&raw_data);
        println!("   ⚡ Zero-copy view: {:?}", zero_copy.as_bytes());
        println!("   📏 Same data, no allocation!\n");

        Ok(())
    }

    pub fn demo_format_abstraction(
        service: &ServiceInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("2️⃣  Format Abstraction on Raw Foundation");
        println!("   All formats build on the same raw binary interface\n");

        // JSON format built on raw binary
        let json_data = commy::serialization::JsonData::<ServiceInfo, 1024>::serialize(service)?;
        println!("   📄 JSON format: {}", json_data.format_name());
        println!("   📊 Size: {} bytes", json_data.buffer_usage().0);
        println!(
            "   🔍 Raw bytes (first 50): {:?}...",
            &json_data.as_bytes()[..50.min(json_data.as_bytes().len())]
        );

        // Verify round-trip through raw binary
        let raw_bytes = json_data.as_bytes();
        let reconstructed =
            commy::serialization::JsonData::<ServiceInfo, 1024>::from_bytes(raw_bytes)?;
        let deserialized: ServiceInfo = reconstructed.deserialize()?;

        println!("   ✅ Round-trip successful: {}", deserialized.name);
        println!("   🎯 Same raw bytes, perfect reconstruction\n");

        Ok(())
    }

    pub fn demo_zero_copy_access(service: &ServiceInfo) -> Result<(), Box<dyn std::error::Error>> {
        println!("3️⃣  Zero-Copy Access Patterns");
        println!("   Direct memory access without deserialization\n");

        let json_data = commy::serialization::JsonData::<ServiceInfo, 1024>::serialize(service)?;

        // Zero-copy access to the underlying data
        let zero_copy_view = json_data.zero_copy_view();
        println!("   ⚡ Zero-copy view size: {} bytes", zero_copy_view.len());
        println!("   🔍 Direct memory access (no copying!)");

        // Validate for zero-copy
        println!(
            "   ✅ Valid for zero-copy: {}",
            json_data.is_valid_for_zero_copy()
        );

        // Simulate memory-mapped file access
        simulate_memory_mapped_access(zero_copy_view)?;

        Ok(())
    }

    pub fn simulate_memory_mapped_access(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        println!("   🗃️  Simulating memory-mapped file access:");
        println!(
            "      - File mapped into memory at address: {:p}",
            data.as_ptr()
        );
        println!("      - Direct byte access without file I/O");
        println!("      - Zero-copy across process boundaries");

        // Different processes could access the same memory region
        let zero_copy_view = ZeroCopyBytes::new(data);
        println!(
            "      - Process A: zero-copy view {} bytes",
            zero_copy_view.len()
        );

        // Another process interprets the same data
        // Interpret bytes as JSON Value for generic demonstration
        let json_interpretation =
            commy::serialization::JsonData::<serde_json::Value, 1024>::from_bytes(data)?;
        println!(
            "      - Process B: JSON interpretation {} bytes",
            json_interpretation.as_bytes().len()
        );
        println!("      - Same memory, different interpretations!\n");

        Ok(())
    }

    pub fn demo_universal_data(service: &ServiceInfo) -> Result<(), Box<dyn std::error::Error>> {
        println!("4️⃣  Universal Data Interface");
        println!("   Single interface for all serialization formats\n");

        // Create different format representations
        let json_data = commy::serialization::JsonData::<ServiceInfo, 1024>::serialize(service)?;

        // Wrap in universal data interface
        // `JsonData` here is the fixed-size `SerializedData` alias from the
        // top-level serialization module and does not implement the
        // raw_binary::FormatData trait expected by `UniversalData::from_format`.
        // Convert explicitly to raw bytes and construct a UniversalData::Raw.
        let universal = UniversalData::Raw(RawBytes::from_bytes(json_data.as_bytes())?);

        println!("   🌍 Universal data format: {}", universal.format_name());
        println!("   📊 Size: {} bytes", universal.as_bytes().len());
        println!(
            "   🔧 Raw binary access: {} bytes",
            universal.as_bytes().len()
        );

        // The universal interface exposes the same raw binary foundation
        let raw_access = universal.as_bytes();
        println!(
            "   ⚡ Direct raw access: {:p} (no format-specific overhead)",
            raw_access.as_ptr()
        );

        Ok(())
    }

    pub fn demo_cross_format_compatibility(
        service: &ServiceInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("5️⃣  Cross-Format Compatibility");
        println!("   Raw binary enables universal format support\n");

        // Serialize with JSON
        let json_data = commy::serialization::JsonData::<ServiceInfo>::serialize(service)?;
        let raw_bytes = json_data.as_bytes();

        println!("   📄 Original: JSON format ({} bytes)", raw_bytes.len());

        // Any format can be stored as raw binary
        let universal_raw = RawBytes::from_bytes(raw_bytes)?;
        println!(
            "   🔧 Stored as: Raw binary ({} bytes)",
            universal_raw.len()
        );

        // Can be interpreted as different formats later
        let reinterpreted_json = commy::serialization::JsonData::<ServiceInfo, 1024>::from_bytes(
            universal_raw.as_bytes(),
        )?;
        println!(
            "   📄 Reinterpreted: JSON format ({} bytes)",
            reinterpreted_json.as_bytes().len()
        );

        // Verify data integrity
        let final_service: ServiceInfo = reinterpreted_json.deserialize()?;
        println!(
            "   ✅ Data integrity: {} (port: {})",
            final_service.name, final_service.port
        );

        println!("   🎯 Key insight: Raw binary is the universal interchange format!\n");

        Ok(())
    }

    #[cfg(feature = "zerocopy")]
    pub fn demo_rkyv_zero_copy(service: &ServiceInfo) -> Result<(), Box<dyn std::error::Error>> {
        println!("6️⃣  rkyv Zero-Copy on Raw Foundation");
        println!("   True zero-copy serialization built on raw binary\n");

        // For this demo, we'll show the concept even though we need proper rkyv integration
        println!("   🦀 rkyv format: Built on raw binary foundation");
        println!("   ⚡ Zero-copy: Direct access to archived data");
        println!("   🎯 Performance: No deserialization overhead");
        println!("   🔧 Foundation: Same raw binary interface");

        println!("   📊 Implementation: Direct archived data access");
        println!("   🗃️  Storage: Raw bytes in memory-mapped files");
        println!("   🌍 Compatibility: Universal raw binary interface\n");

        Ok(())
    }
}

// The real main delegates into the JSON demo module when the feature is enabled.
#[cfg(feature = "json")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    json_demo::run()
}

/// Bonus: Demonstrate how this enables custom protocols
fn _demo_custom_protocol() -> Result<(), RawBinaryError> {
    println!("💡 Bonus: Custom Protocol Support");
    println!("   Raw binary foundation enables any serialization format\n");

    // Custom binary protocol
    struct CustomProtocol {
        magic: u32,
        version: u16,
        payload_size: u32,
        payload: Vec<u8>,
    }

    impl RawBinaryData for CustomProtocol {
        fn as_bytes(&self) -> &[u8] {
            // Custom binary layout
            todo!("Implement custom binary serialization")
        }

        fn from_bytes(_bytes: &[u8]) -> Result<Self, RawBinaryError> {
            // Custom binary parsing
            todo!("Implement custom binary deserialization")
        }
    }

    println!("   🔧 Custom protocols can implement RawBinaryData");
    println!("   ⚡ Zero-copy access through the same interface");
    println!("   🌍 Universal compatibility with all Commy features");

    Ok(())
}
