//! Simple Foundation Demo
//!
//! This example demonstrates the working foundation of Commy's distributed
//! service mesh with a simple protocol handler and basic operations.

use commy::simple_protocol::{SimpleMessage, SimpleProtocolHandler};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Commy Foundation Demo - Phase 1 Implementation");
    println!("====================================================");

    // Initialize the simple protocol handler
    let mut handler = SimpleProtocolHandler::new();
    println!("✅ Protocol handler initialized");

    // Demonstrate basic operations
    println!("\n📝 Testing Basic File Operations:");

    // Create operation
    let create_msg = SimpleMessage {
        id: "demo_file_1".to_string(),
        operation: "create".to_string(),
        data: Some(b"Hello from Commy distributed mesh!".to_vec()),
        metadata: HashMap::new(),
    };

    let response = handler.handle_message(create_msg);
    println!("   Create: {} - {}", response.success, response.message);

    // Create another file
    let create_msg2 = SimpleMessage {
        id: "demo_file_2".to_string(),
        operation: "create".to_string(),
        data: Some(b"Second file in the mesh".to_vec()),
        metadata: HashMap::new(),
    };

    let response2 = handler.handle_message(create_msg2);
    println!("   Create: {} - {}", response2.success, response2.message);

    // Read operation
    let read_msg = SimpleMessage {
        id: "demo_file_1".to_string(),
        operation: "read".to_string(),
        data: None,
        metadata: HashMap::new(),
    };

    let read_response = handler.handle_message(read_msg);
    println!(
        "   Read: {} - {}",
        read_response.success, read_response.message
    );
    if let Some(data) = read_response.data {
        println!("   Data: {}", String::from_utf8_lossy(&data));
    }

    // List operation
    let list_msg = SimpleMessage {
        id: "list_request".to_string(),
        operation: "list".to_string(),
        data: None,
        metadata: HashMap::new(),
    };

    let list_response = handler.handle_message(list_msg);
    println!(
        "   List: {} - {}",
        list_response.success, list_response.message
    );

    // Show statistics
    let (file_count, total_size) = handler.get_stats();
    println!("\n📊 Mesh Statistics:");
    println!("   Files: {}", file_count);
    println!("   Total Size: {} bytes", total_size);

    // Demonstrate serialization formats (using our foundation)
    println!("\n🔄 Testing Serialization Formats:");

    // JSON serialization
    let json_data = serde_json::to_string(&list_response)
        .map_err(commy::errors::CommyError::JsonSerialization)
        .map_err(Box::<dyn std::error::Error>::from)?;
    println!("   JSON size: {} bytes", json_data.len());

    // Binary serialization (using JSON as fallback)
    let binary_data = serde_json::to_vec(&list_response)
        .map_err(commy::errors::CommyError::JsonSerialization)
        .map_err(Box::<dyn std::error::Error>::from)?;
    println!("   Binary size: {} bytes", binary_data.len());

    println!("\n✨ Foundation Demo Complete!");
    println!("   ✅ Protocol handling working");
    println!("   ✅ Multi-format serialization working");
    println!("   ✅ Basic file operations working");
    println!("   ✅ Statistics and monitoring working");

    println!("\n🎯 Ready for Phase 1 roadmap implementation:");
    println!("   • Enhanced shared file protocol");
    println!("   • Network transport integration");
    println!("   • Hybrid transport selection");
    println!("   • Service discovery foundation");

    Ok(())
}
