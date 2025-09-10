# Rust API Documentation

This document provides comprehensive documentation for the Commy Rust API, including all modules, types, and functions.

## Core Modules

### `commy::manager::core`

The core module contains the primary APIs for the Commy distributed communication mesh.

#### `SharedFileManager`

The main entry point for file-based operations in Commy.

```rust
use commy::manager::core::{ManagerConfig, SharedFileManager};

// Create a new SharedFileManager
let config = ManagerConfig::default();
let manager = SharedFileManager::new(config).await?;
```

##### Methods

###### `new(config: ManagerConfig) -> Result<Self, ManagerError>`

Creates a new `SharedFileManager` instance with the provided configuration.

**Parameters:**

- `config`: A `ManagerConfig` instance specifying the manager behavior

**Returns:**

- `Result<SharedFileManager, ManagerError>`: The manager instance or an error

**Example:**

```rust
let config = ManagerConfig {
    base_directory: PathBuf::from("/tmp/commy"),
    max_file_size_mb: 512,
    enable_mesh_capabilities: true,
    ..Default::default()
};
let manager = SharedFileManager::new(config).await?;
```

###### `request_file(request: SharedFileRequest) -> Result<SharedFileResponse, ManagerError>`

Executes a file operation request asynchronously.

**Parameters:**

- `request`: A `SharedFileRequest` specifying the operation to perform

**Returns:**

- `Result<SharedFileResponse, ManagerError>`: The operation response or an error

**Example:**

```rust
let request = SharedFileRequest {
    identifier: "my-operation".to_string(),
    pattern: MessagePattern::OneWay { delivery_confirmation: false },
    pattern_config: MessagePatternConfig::default(),
    file_path: Some(PathBuf::from("data.txt")),
    operation: SharedFileOperation::Read {
        path: PathBuf::from("data.txt"),
        offset: 0,
        length: 1024,
    },
};

let response = manager.request_file(request).await?;
```

#### `ManagerConfig`

Configuration structure for `SharedFileManager`.

```rust
pub struct ManagerConfig {
    pub base_directory: PathBuf,
    pub max_file_size_mb: usize,
    pub enable_mesh_capabilities: bool,
    pub default_timeout_duration: Duration,
    pub max_concurrent_operations: usize,
}
```

##### Fields

- **`base_directory`**: Base directory for file operations (default: `/tmp/commy`)
- **`max_file_size_mb`**: Maximum file size in megabytes (default: 1024)
- **`enable_mesh_capabilities`**: Enable distributed mesh features (default: true)
- **`default_timeout_duration`**: Default timeout for operations (default: 30 seconds)
- **`max_concurrent_operations`**: Maximum concurrent operations (default: 100)

### `commy::manager`

The manager module contains request/response types and message patterns.

#### `SharedFileRequest`

Represents a file operation request.

```rust
pub struct SharedFileRequest {
    pub identifier: String,
    pub pattern: MessagePattern,
    pub pattern_config: MessagePatternConfig,
    pub file_path: Option<PathBuf>,
    pub operation: SharedFileOperation,
}
```

##### Fields

- **`identifier`**: Unique identifier for the request
- **`pattern`**: Communication pattern to use
- **`pattern_config`**: Configuration for the pattern
- **`file_path`**: Optional file path for the operation
- **`operation`**: The specific file operation to perform

#### `SharedFileResponse`

Represents the response from a file operation.

```rust
pub struct SharedFileResponse {
    pub identifier: String,
    pub success: bool,
    pub file_path: Option<PathBuf>,
    pub data: Option<Vec<u8>>,
    pub error_message: Option<String>,
    pub metadata: Option<FileMetadata>,
}
```

##### Fields

- **`identifier`**: The request identifier this response corresponds to
- **`success`**: Whether the operation was successful
- **`file_path`**: The file path that was operated on
- **`data`**: Optional data returned from read operations
- **`error_message`**: Error message if operation failed
- **`metadata`**: Optional file metadata

#### `MessagePattern`

Defines communication patterns for operations.

```rust
pub enum MessagePattern {
    OneWay {
        delivery_confirmation: bool,
    },
    RequestResponse {
        timeout: Duration,
    },
    PublishSubscribe {
        topic: String,
    },
}
```

##### Variants

- **`OneWay`**: Fire-and-forget messaging
  - `delivery_confirmation`: Whether to confirm delivery
- **`RequestResponse`**: Synchronous request-response pattern
  - `timeout`: Timeout for the response
- **`PublishSubscribe`**: Event-driven messaging
  - `topic`: Topic name for pub/sub

#### `MessagePatternConfig`

Configuration for message patterns.

```rust
pub struct MessagePatternConfig {
    pub max_retries: Option<u32>,
    pub timeout_duration: Option<Duration>,
    pub priority: Option<u8>,
}
```

##### Fields

- **`max_retries`**: Maximum number of retry attempts
- **`timeout_duration`**: Timeout for the operation
- **`priority`**: Priority level (1-10, higher is more priority)

#### `SharedFileOperation`

Defines specific file operations.

```rust
pub enum SharedFileOperation {
    Read {
        path: PathBuf,
        offset: u64,
        length: usize,
    },
    Write {
        path: PathBuf,
        offset: u64,
        data: Vec<u8>,
    },
    Delete {
        path: PathBuf,
    },
    GetMetadata {
        path: PathBuf,
    },
}
```

##### Variants

- **`Read`**: Read data from a file
  - `path`: File path to read from
  - `offset`: Byte offset to start reading
  - `length`: Number of bytes to read
- **`Write`**: Write data to a file
  - `path`: File path to write to
  - `offset`: Byte offset to start writing
  - `data`: Data to write
- **`Delete`**: Delete a file
  - `path`: File path to delete
- **`GetMetadata`**: Get file metadata
  - `path`: File path to get metadata for

### `commy::mesh`

The mesh module provides distributed coordination capabilities.

#### `CommyMesh`

Main mesh coordination interface.

```rust
use commy::mesh::CommyMesh;

let mesh = CommyMesh::new("my-service").await?;
mesh.start().await?;
```

##### Methods

###### `new(service_name: &str) -> Result<Self, MeshError>`

Creates a new mesh instance.

###### `start() -> Result<(), MeshError>`

Starts the mesh and begins service discovery.

###### `stop() -> Result<(), MeshError>`

Stops the mesh and cleans up resources.

###### `register_service(service: ServiceInfo) -> Result<(), MeshError>`

Registers a service with the mesh.

###### `discover_services(service_type: &str) -> Result<Vec<ServiceInfo>, MeshError>`

Discovers services of a specific type.

### `commy::config`

Configuration management for Commy.

#### `CommyConfig`

Global configuration structure.

```rust
pub struct CommyConfig {
    pub mesh: MeshConfig,
    pub security: SecurityConfig,
    pub logging: LoggingConfig,
}
```

#### `MeshConfig`

Mesh-specific configuration.

```rust
pub struct MeshConfig {
    pub node_id: String,
    pub bind_address: String,
    pub discovery_port: u16,
    pub heartbeat_interval: Duration,
}
```

#### `SecurityConfig`

Security configuration.

```rust
pub struct SecurityConfig {
    pub enable_encryption: bool,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    pub require_authentication: bool,
}
```

## Error Handling

### `ManagerError`

Errors from the SharedFileManager.

```rust
pub enum ManagerError {
    ConfigurationError(String),
    FileOperationError(String),
    NetworkError(String),
    TimeoutError,
    SerializationError(String),
}
```

### `MeshError`

Errors from mesh operations.

```rust
pub enum MeshError {
    InitializationError(String),
    NetworkError(String),
    ServiceRegistrationError(String),
    DiscoveryError(String),
}
```

## Usage Examples

### Basic File Operations

```rust
use commy::manager::core::{ManagerConfig, SharedFileManager};
use commy::manager::{MessagePattern, MessagePatternConfig, SharedFileRequest, SharedFileOperation};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize manager
    let config = ManagerConfig::default();
    let manager = SharedFileManager::new(config).await?;

    // Write operation
    let write_request = SharedFileRequest {
        identifier: "write-demo".to_string(),
        pattern: MessagePattern::OneWay { delivery_confirmation: true },
        pattern_config: MessagePatternConfig {
            max_retries: Some(3),
            timeout_duration: Some(Duration::from_secs(10)),
            priority: Some(5),
        },
        file_path: Some(PathBuf::from("demo.txt")),
        operation: SharedFileOperation::Write {
            path: PathBuf::from("demo.txt"),
            offset: 0,
            data: b"Hello, Commy!".to_vec(),
        },
    };

    let response = manager.request_file(write_request).await?;
    println!("Write successful: {}", response.success);

    // Read operation
    let read_request = SharedFileRequest {
        identifier: "read-demo".to_string(),
        pattern: MessagePattern::RequestResponse {
            timeout: Duration::from_secs(5),
        },
        pattern_config: MessagePatternConfig::default(),
        file_path: Some(PathBuf::from("demo.txt")),
        operation: SharedFileOperation::Read {
            path: PathBuf::from("demo.txt"),
            offset: 0,
            length: 1024,
        },
    };

    let read_response = manager.request_file(read_request).await?;
    if let Some(data) = read_response.data {
        println!("Read data: {}", String::from_utf8_lossy(&data));
    }

    Ok(())
}
```

### Mesh Service Registration

```rust
use commy::mesh::{CommyMesh, ServiceInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mesh = CommyMesh::new("my-api-service").await?;

    // Register service
    let service_info = ServiceInfo {
        name: "user-api".to_string(),
        service_type: "api".to_string(),
        address: "127.0.0.1:8080".to_string(),
        metadata: HashMap::new(),
    };

    mesh.register_service(service_info).await?;
    mesh.start().await?;

    // Service is now discoverable by other mesh nodes

    Ok(())
}
```

### Advanced Configuration

```rust
use commy::manager::core::ManagerConfig;
use std::path::PathBuf;
use std::time::Duration;

let config = ManagerConfig {
    base_directory: PathBuf::from("/var/lib/commy"),
    max_file_size_mb: 2048,
    enable_mesh_capabilities: true,
    default_timeout_duration: Duration::from_secs(60),
    max_concurrent_operations: 200,
};

let manager = SharedFileManager::new(config).await?;
```

## Performance Considerations

### Memory Usage

- Use appropriate file size limits to control memory usage
- Consider streaming for large files
- Monitor concurrent operation limits

### Network Performance

- Configure appropriate timeouts for your network environment
- Use delivery confirmation only when necessary
- Consider message priority for time-sensitive operations

### Error Handling

- Always handle errors appropriately
- Use retries for transient failures
- Implement circuit breaker patterns for reliability

## Thread Safety

All Commy APIs are designed to be thread-safe and can be used across multiple async tasks:

```rust
use std::sync::Arc;

let manager = Arc::new(SharedFileManager::new(config).await?);
let manager_clone = manager.clone();

tokio::spawn(async move {
    // Use manager_clone in this task
    let response = manager_clone.request_file(request).await?;
    // Handle response
});
```

## Feature Flags

Commy supports several feature flags in `Cargo.toml`:

```toml
[dependencies]
commy = { version = "0.1.0", features = ["manager", "mesh", "security"] }
```

- **`manager`**: Enables SharedFileManager functionality (required)
- **`mesh`**: Enables distributed mesh capabilities
- **`security`**: Enables encryption and authentication features
- **`ffi`**: Enables FFI bindings for other languages

This documentation covers the core Rust API. For language-specific SDKs, see the respective SDK documentation.
