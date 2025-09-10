# NOT PRODUCTION READY

This snapshot was backed up to GitHub automatically. Do not use in production.

# Commy - The Premier Distributed Communication Mesh

[![License](https://img.shields.io/crates/l/commy)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/github/workflow/status/commy-project/commy/CI)](https://github.com/commy-project/commy/actions)

> **THE** distributed communication mesh for microservices in any programming language.

Commy is a high-performance Rust distributed communication mesh designed to be the premier solution for microservice communication across multiple programming languages. It provides exceptional performance, comprehensive security, cross-platform compatibility, and multi-language SDK support.

## 🚀 Quick Start

### Rust (Core Library)

```rust
use commy::manager::core::{ManagerConfig, SharedFileManager};
use commy::manager::{MessagePattern, MessagePatternConfig, SharedFileRequest, SharedFileOperation};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize SharedFileManager
    let config = ManagerConfig::default();
    let file_manager = SharedFileManager::new(config).await?;

    // Create a file request
    let request = SharedFileRequest {
        identifier: "my-data".to_string(),
        pattern: MessagePattern::OneWay {
            delivery_confirmation: false,
        },
        pattern_config: MessagePatternConfig {
            max_retries: Some(3),
            timeout_duration: Some(Duration::from_secs(30)),
            priority: Some(1),
        },
        file_path: Some(PathBuf::from("data.txt")),
        operation: SharedFileOperation::Write {
            path: PathBuf::from("data.txt"),
            offset: 0,
            data: b"Hello, Commy!".to_vec(),
        },
    };

    // Execute the request
    let response = file_manager.request_file(request).await?;
    println!("✅ File operation completed: {:?}", response.file_path);

    Ok(())
}
```

### Python SDK

```python
import asyncio
from commy_async import AsyncCommyMesh, create_mesh_cluster

async def main():
    # Create a mesh cluster
    node_configs = [
        ("node-1", 8080),
        ("node-2", 8081),
    ]

    meshes = await create_mesh_cluster(node_configs)
    print(f"✅ Created mesh cluster with {len(meshes)} nodes")

    # Use the mesh for service discovery, communication, etc.
    # ... your application logic here ...

    # Cleanup
    await shutdown_mesh_cluster(meshes)

if __name__ == "__main__":
    asyncio.run(main())
```

### Node.js SDK

```javascript
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');

async function main() {
    const mesh = new AsyncCommyMesh('my-service', 8080);
    await mesh.start();

    console.log('✅ Mesh started successfully');

    // Register service, discover peers, etc.
    // ... your application logic here ...

    await mesh.stop();
}

main().catch(console.error);
```

## ✨ Key Features

### 🔥 Performance

- **Ultra-low Latency**: Optimized for minimal delay in all operations
- **Zero-Copy Operations**: Memory-efficient data handling where possible
- **Intelligent Transport Selection**: Automatic optimization based on workload

### 🔒 Security

- **Security by Default**: All communications encrypted with secure defaults
- **No Security Fallbacks**: Rejects insecure connections rather than downgrading
- **Configurable Security Levels**: Admin control over security requirements

### 🌐 Cross-Platform

- **Universal Compatibility**: Works on Linux, macOS, Windows
- **Multiple Architectures**: x86_64, ARM64, and more
- **Container Ready**: Docker and Kubernetes native support

### 🔌 Multi-Language Support

- **Rust** (native)
- **Python** (async/await + FastAPI integration)
- **Node.js** (async + Express middleware)
- **Go** (concurrent with goroutines)
- **C/C++** (FFI bindings)
- **Browser** (WebAssembly + WebSockets)

## 🛠️ Installation

### Rust

Add to your `Cargo.toml`:

```toml
[dependencies]
commy = { version = "0.1.0", features = ["manager"] }
tokio = { version = "1.0", features = ["full"] }
```

### Python

```bash
pip install commy-python
```

### Node.js

```bash
npm install @commy/nodejs-sdk
```

### Go

```bash
go get github.com/commy-project/commy/sdks/go
```

## 📚 SDKs

- [Python SDK](sdks/python/README.md) - AsyncIO and FastAPI integration
- [Node.js SDK](sdks/nodejs/README.md) - Express middleware and async patterns
- [Go SDK](sdks/go/README.md) - Concurrent patterns with goroutines
- [C++ SDK](sdks/cpp/README.md) - High-performance C++ bindings
- [Browser SDK](sdks/browser/README.md) - WebAssembly and WebSocket support

## 🧪 Testing

### Core Library

```bash
cargo test --lib --features manager
```

### SDKs

```bash
# Python
cd sdks/python && python -m pytest tests/ -v

# Node.js
cd sdks/nodejs && npm test

# Go
cd sdks/go && go test -v
```

## 📊 Performance

Commy is designed for exceptional performance:

- **Latency**: Sub-millisecond response times
- **Throughput**: Handles millions of requests per second
- **Memory**: Efficient memory usage with zero-copy operations
- **Scalability**: Linear scaling with cluster size

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
git clone https://github.com/commy-project/commy.git
cd commy
cargo build --features manager
cargo test --lib --features manager
```

## 📄 License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## 🙋 Support

- [GitHub Issues](https://github.com/commy-project/commy/issues) - Bug reports and feature requests
- [Documentation](https://docs.commy.dev) - Comprehensive guides and API docs
- [Community Discord](https://discord.gg/commy) - Real-time community support

---

Made with ❤️ by the Commy Development Team

*Commy: THE distributed communication mesh everyone uses.*

## Running Examples with All Serialization Features

To exercise optional serialization backends (including `rkyv`/zero-copy), enable the combined feature set when running examples. From the repository root:

```powershell
# Run the raw binary foundation demo with all serialization backends enabled
cargo run --example raw_binary_foundation_demo --features all_formats

# Run the polyglot demo (enable rkyv/capnproto/postcard as available)
cargo run --example polyglot_serialization_demo --features all_formats
```

Notes:

- The `all_formats` feature should enable optional backends such as `json`, `binary`, `messagepack`, `compact`, `zerocopy` (rkyv), and `capnproto` depending on the workspace features defined in Cargo.toml.
- Use this in CI or local dev to validate attribute macros (like `rkyv` derives) and backend-specific code paths.

