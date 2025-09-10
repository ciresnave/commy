# Getting Started with Commy

Welcome to Commy! This guide will walk you through setting up and using the Commy distributed communication mesh for the first time.

## Prerequisites

- **Rust**: Version 1.70 or later
- **Operating System**: Linux, macOS, or Windows
- **Memory**: Minimum 4GB RAM
- **Network**: Access to network interfaces for mesh communication

## Installation

### Core Rust Library

Add Commy to your `Cargo.toml`:

```toml
[dependencies]
commy = { version = "0.1.0", features = ["manager"] }
tokio = { version = "1.0", features = ["full"] }
```

### SDK Installation

Choose your preferred SDK:

#### Python

```bash
pip install commy-python
```

#### Node.js

```bash
npm install @commy/nodejs-sdk
```

#### Go

```bash
go get github.com/commy-project/commy/sdks/go
```

## Your First Commy Application

Let's create a simple producer-consumer application to demonstrate the basic concepts.

### 1. Basic File Manager (Rust)

Create a new Rust project:

```bash
cargo new my-commy-app
cd my-commy-app
```

Add the following to `src/main.rs`:

```rust
use commy::manager::core::{ManagerConfig, SharedFileManager};
use commy::manager::{MessagePattern, MessagePatternConfig, SharedFileRequest, SharedFileOperation};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Commy File Manager Demo");

    // Initialize SharedFileManager with default configuration
    let config = ManagerConfig::default();
    let file_manager = SharedFileManager::new(config).await?;
    println!("✅ SharedFileManager initialized");

    // Create a simple write request
    let write_request = SharedFileRequest {
        identifier: "hello-world".to_string(),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: MessagePatternConfig {
            max_retries: Some(3),
            timeout_duration: Some(Duration::from_secs(10)),
            priority: Some(1),
        },
        file_path: Some(PathBuf::from("hello.txt")),
        operation: SharedFileOperation::Write {
            path: PathBuf::from("hello.txt"),
            offset: 0,
            data: b"Hello, Commy! This is my first message.".to_vec(),
        },
    };

    // Execute the write request
    let response = file_manager.request_file(write_request).await?;
    println!("✅ Write completed: {:?}", response.file_path);

    // Create a read request
    let read_request = SharedFileRequest {
        identifier: "read-hello-world".to_string(),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: MessagePatternConfig {
            max_retries: Some(3),
            timeout_duration: Some(Duration::from_secs(10)),
            priority: Some(1),
        },
        file_path: Some(PathBuf::from("hello.txt")),
        operation: SharedFileOperation::Read {
            path: PathBuf::from("hello.txt"),
            offset: 0,
            length: 1024,
        },
    };

    // Execute the read request
    let read_response = file_manager.request_file(read_request).await?;
    println!("✅ Read completed: {:?}", read_response.file_path);

    println!("🎉 Demo completed successfully!");
    Ok(())
}
```

Run your application:

```bash
cargo run
```

### 2. Python SDK Example

Create a Python script `mesh_example.py`:

```python
import asyncio
from commy_async import AsyncCommyMesh, create_mesh_cluster, shutdown_mesh_cluster

async def main():
    print("🚀 Starting Python Commy Demo")

    # Create a simple mesh cluster
    node_configs = [
        ("producer-node", 8080),
        ("consumer-node", 8081),
    ]

    try:
        # Create mesh cluster
        meshes = await create_mesh_cluster(node_configs)
        print(f"✅ Created mesh cluster with {len(meshes)} nodes")

        # Get references to individual nodes
        producer_mesh = meshes[0]
        consumer_mesh = meshes[1]

        print("🔄 Mesh cluster is running...")

        # Add your mesh logic here:
        # - Service registration
        # - Message publishing/consuming
        # - Health checks

        # Keep running for a few seconds
        await asyncio.sleep(2)

    finally:
        # Clean shutdown
        await shutdown_mesh_cluster(meshes)
        print("✅ Mesh cluster shut down cleanly")

if __name__ == "__main__":
    asyncio.run(main())
```

Run the Python example:

```bash
python mesh_example.py
```

### 3. Node.js SDK Example

Create a Node.js script `mesh-example.js`:

```javascript
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');

async function main() {
    console.log('🚀 Starting Node.js Commy Demo');

    const mesh = new AsyncCommyMesh('demo-service', 8082);

    try {
        await mesh.start();
        console.log('✅ Mesh started successfully');

        // Add your mesh logic here:
        // - Service registration
        // - Event handling
        // - API endpoints

        // Keep running for a few seconds
        setTimeout(() => {
            console.log('🔄 Mesh is running...');
        }, 1000);

    } catch (error) {
        console.error('❌ Error:', error);
    } finally {
        await mesh.stop();
        console.log('✅ Mesh stopped cleanly');
    }
}

main().catch(console.error);
```

Run the Node.js example:

```bash
node mesh-example.js
```

## Core Concepts

### SharedFileManager

The `SharedFileManager` is the modern API for file-based communication in Commy. It provides:

- **Async Operations**: All operations are non-blocking
- **Message Patterns**: OneWay, RequestResponse, PublishSubscribe
- **Configuration**: Flexible timeout, retry, and priority settings
- **File Operations**: Read, Write, Delete, and metadata operations

### Message Patterns

Commy supports three primary communication patterns:

1. **OneWay**: Fire-and-forget messaging

   ```rust
   MessagePattern::OneWay { delivery_confirmation: false }
   ```

2. **RequestResponse**: Synchronous request-response pattern

   ```rust
   MessagePattern::RequestResponse { timeout: Duration::from_secs(30) }
   ```

3. **PublishSubscribe**: Event-driven messaging

   ```rust
   MessagePattern::PublishSubscribe { topic: "my-topic".to_string() }
   ```

### Configuration

The `ManagerConfig` allows you to customize the behavior:

```rust
let config = ManagerConfig {
    base_directory: PathBuf::from("/tmp/commy"),
    max_file_size_mb: 1024,
    enable_mesh_capabilities: true,
    ..Default::default()
};
```

## Next Steps

Now that you have Commy running, explore these advanced topics:

1. **[Service Discovery](service-discovery.md)** - Automatic peer discovery
2. **[Load Balancing](load-balancing.md)** - Intelligent request distribution
3. **[Security Configuration](security.md)** - Encryption and authentication
4. **[Performance Optimization](performance.md)** - Tuning for your workload
5. **[Monitoring & Observability](monitoring.md)** - Health checks and metrics

## Common Issues

### Build Errors

If you encounter build errors, ensure you have:

- Rust 1.70+ installed
- All required system dependencies
- The `manager` feature enabled in your `Cargo.toml`

### Network Issues

If mesh communication fails:

- Check firewall settings
- Verify port availability
- Review network interface configuration

### Performance Issues

For performance optimization:

- Review file size limits
- Adjust timeout configurations
- Monitor memory usage
- Consider async patterns

## Getting Help

- **Documentation**: [docs.commy.dev](https://docs.commy.dev)
- **GitHub Issues**: [Report bugs or request features](https://github.com/commy-project/commy/issues)
- **Community Discord**: [Real-time support](https://discord.gg/commy)

Welcome to the Commy community! 🎉
