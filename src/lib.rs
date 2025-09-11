#![doc(html_logo_url = "../../../TwoRobotsExchangingCodeOverCoffee.jpeg")]
//! Commy - High Performance Distributed Communication Mesh
//!
//! Commy is a high-performance Rust distributed communication mesh designed to be THE premier
//! distributed communication mesh for microservices in any programming language. It provides
//! exceptional performance, comprehensive security, cross-platform compatibility, and
//! multi-language SDK support.
//!
//! # Features
//!
//! - **Ultra-low Latency**: Memory-mapped files for local IPC with sub-microsecond access
//! - **Multi-format Serialization**: Support for JSON, Binary, MessagePack, CBOR, and zero-copy
//! - **Intelligent Transport**: Automatic selection between shared memory and network
//! - **Enterprise Security**: OAuth, JWT, RBAC, TLS encryption, audit logging
//! - **Distributed Configuration**: Hierarchical config with real-time updates
//! - **Cross-platform**: Windows, Linux, macOS support
//! - **Multi-language SDKs**: Python, JavaScript/TypeScript, Go, C/C++ bindings
//!
//! # Architecture
//!
//! Commy transforms from a simple IPC library into a complete distributed service mesh:
//!
//! 1. **Foundation Layer**: Shared file manager with auth and config integration
//! 2. **Mesh Capabilities**: Service discovery, load balancing, failover
//! 3. **Multi-language SDKs**: Native bindings for popular languages
//! 4. **Enterprise Features**: Multi-region federation, advanced observability
//!
//! # Quick Start
//!
//! ```rust
//! use commy::SharedFileManager;
//! use commy::manager::core::ManagerConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Construct a default manager configuration and create the manager.
//!     // The manager constructor now requires a `ManagerConfig`.
//!     let config = ManagerConfig::default();
//!     let _manager = SharedFileManager::new(config).await?;
//!
//!     // Manager is initialized; further operations (requesting files, starting
//!     // the manager, etc.) are available via the public API on `SharedFileManager`.
//!     println!("SharedFileManager initialized");
//!     Ok(())
//! }
//! ```

pub mod serialization;

pub mod simple_protocol;

pub mod utils;

#[cfg(feature = "plugins")]
pub mod plugins;

#[cfg(feature = "plugins")]
pub mod types;

pub mod errors;

#[cfg(feature = "manager")]
pub mod manager;

#[cfg(feature = "manager")]
pub mod config;

#[cfg(feature = "manager")]
pub mod error;

#[cfg(feature = "mesh")]
pub mod mesh;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "manager")]
pub use manager::core::SharedFileManager;

#[cfg(feature = "manager")]
pub use config::{CommyConfig, ConfigBuilder};

#[cfg(feature = "manager")]
pub use error::{CommyError, CommyResult, ErrorContext};

#[cfg(feature = "mesh")]
pub use mesh::{HealthMonitor, LoadBalancer, MeshCoordinator, NodeRegistry, ServiceDiscovery};

#[cfg(feature = "ffi")]
pub use ffi::*;

// Re-export serialization types for convenience
pub use serialization::*;
