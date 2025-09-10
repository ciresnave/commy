//! Unified Configuration Module
//!
//! This module consolidates all configuration types into a single, coherent system
//! with proper validation, defaults, and builder patterns.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Unified configuration for all Commy components
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommyConfig {
    /// Manager configuration
    pub manager: ManagerConfig,

    /// Transport configuration
    pub transport: TransportConfig,

    /// Network configuration
    pub network: NetworkConfig,

    /// Shared memory configuration
    pub shared_memory: SharedMemoryConfig,

    /// Security configuration
    pub security: SecurityConfig,

    /// Performance configuration
    pub performance: PerformanceConfig,

    /// Mesh configuration
    pub mesh: MeshConfiguration,

    /// Service discovery configuration
    pub discovery: DiscoveryConfiguration,
}
/// Manager-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    /// Port to listen on
    pub listen_port: u16,

    /// Address to bind to
    pub bind_address: String,

    /// Maximum number of files
    pub max_files: u64,

    /// Maximum file size in bytes
    pub max_file_size: u64,

    /// Default TTL in seconds
    pub default_ttl_seconds: u64,

    /// Heartbeat timeout in seconds
    pub heartbeat_timeout_seconds: u64,

    /// Cleanup interval in seconds
    pub cleanup_interval_seconds: u64,

    /// Files directory
    pub files_directory: PathBuf,
}

/// Transport layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Default transport preference
    pub default_preference: TransportPreference,

    /// Performance thresholds
    pub performance_thresholds: PerformanceThresholds,

    /// Enable automatic optimization
    pub auto_optimization: bool,

    /// Fallback behavior when primary transport fails
    pub fallback_behavior: FallbackBehavior,
}

/// Network transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Host to connect to
    pub host: String,

    /// Port to connect to
    pub port: u16,

    /// TLS configuration
    pub tls_config: Option<TlsConfiguration>,

    /// Connection timeout
    pub connection_timeout: Duration,

    /// Read timeout
    pub read_timeout: Duration,

    /// Write timeout
    pub write_timeout: Duration,

    /// Enable keep-alive
    pub keep_alive: bool,

    /// TCP nodelay setting
    pub nodelay: bool,
}

/// Shared memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryConfig {
    /// Base directory for shared memory files
    pub base_path: String,

    /// Maximum file size in bytes
    pub max_file_size: usize,

    /// Enable automatic cleanup
    pub auto_cleanup: bool,

    /// File permissions (Unix)
    pub file_permissions: u32,

    /// Enable file locking
    pub enable_locking: bool,

    /// Sync strategy
    pub sync_strategy: SyncStrategy,

    /// Enable memory mapping optimizations
    pub enable_optimizations: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable authentication
    pub enable_auth: bool,

    /// JWT secret key
    pub jwt_secret: Option<String>,

    /// Token lifetime
    pub token_lifetime: Duration,

    /// Enable encryption
    pub enable_encryption: bool,

    /// TLS configuration
    pub tls: Option<TlsConfiguration>,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable high performance mode
    pub high_performance_mode: bool,

    /// Performance tier
    pub tier: PerformanceTier,

    /// Buffer sizes
    pub buffer_sizes: BufferSizes,

    /// Threading configuration
    pub threading: ThreadingConfig,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfiguration {
    /// Certificate file path
    pub cert_file: Option<PathBuf>,

    /// Private key file path
    pub key_file: Option<PathBuf>,

    /// CA certificate file path
    pub ca_file: Option<PathBuf>,

    /// TLS version to use
    pub version: TlsVersion,

    /// Enable certificate verification
    pub verify_certificates: bool,
}

/// Transport preferences
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransportPreference {
    /// Require local transport only
    RequireLocal,
    /// Require network transport only
    RequireNetwork,
    /// Prefer local but allow network fallback
    PreferLocal,
    /// Prefer network but allow local fallback
    PreferNetwork,
    /// Automatically optimize based on metrics
    AutoOptimize,
    /// Adaptive selection based on load
    Adaptive,
    /// Local transport only, no fallback
    LocalOnly,
    /// Network transport only, no fallback
    NetworkOnly,
}

/// Performance thresholds for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable latency in microseconds
    pub max_latency_us: u32,

    /// Minimum required throughput in MB/s
    pub min_throughput_mbps: u32,

    /// Maximum CPU usage percentage
    pub max_cpu_usage: f64,

    /// Maximum memory usage in MB
    pub max_memory_mb: u64,

    /// Minimum success rate (0.0 to 1.0)
    pub min_success_rate: f64,
}

/// Fallback behavior when transport fails
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FallbackBehavior {
    /// No fallback, fail immediately
    None,
    /// Try alternative transport once
    TryOnce,
    /// Retry with exponential backoff
    RetryWithBackoff {
        max_attempts: u32,
        base_delay_ms: u32,
    },
    /// Queue requests until transport recovers
    Queue { max_queue_size: usize },
}

/// File synchronization strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncStrategy {
    /// No explicit synchronization
    None,
    /// Sync on every write
    Immediate,
    /// Sync periodically
    Periodic { interval_ms: u32 },
    /// Sync on close/shutdown
    OnClose,
}

/// TLS protocol versions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TlsVersion {
    /// TLS 1.2
    Tls12,
    /// TLS 1.3
    Tls13,
}

/// Performance tiers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PerformanceTier {
    UltraLow,
    Low,
    Medium,
    Standard,
    High,
    Ultra,
}

/// Buffer size configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSizes {
    /// Read buffer size
    pub read_buffer: usize,

    /// Write buffer size
    pub write_buffer: usize,

    /// Network buffer size
    pub network_buffer: usize,

    /// Shared memory buffer size
    pub shared_memory_buffer: usize,
}

/// Threading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadingConfig {
    /// Number of worker threads
    pub worker_threads: Option<usize>,

    /// Enable work stealing
    pub work_stealing: bool,

    /// Thread stack size
    pub stack_size: Option<usize>,
}

/// Mesh coordination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfiguration {
    /// Maximum number of nodes in the mesh
    pub max_nodes: usize,

    /// Node heartbeat interval
    pub heartbeat_interval: Duration,

    /// Node timeout threshold
    pub node_timeout: Duration,

    /// Service registration timeout
    pub registration_timeout: Duration,

    /// Enable automatic service discovery
    pub auto_discovery: bool,

    /// Mesh network port range
    pub port_range: (u16, u16),
}

/// Service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfiguration {
    /// Discovery broadcast interval
    pub broadcast_interval: Duration,

    /// Service TTL (time to live)
    pub service_ttl: Duration,

    /// Maximum number of services per node
    pub max_services_per_node: usize,

    /// Enable service health checking
    pub enable_health_checks: bool,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Service announcement retries
    pub announcement_retries: u32,
}
/// Configuration builder with validation and defaults
#[derive(Default)]
pub struct ConfigBuilder {
    config: CommyConfig,
}

impl ConfigBuilder {
    /// Create a new configuration builder with defaults
    pub fn new() -> Self {
        Self {
            config: CommyConfig::default(),
        }
    }

    /// Set manager configuration
    pub fn manager(mut self, manager: ManagerConfig) -> Self {
        self.config.manager = manager;
        self
    }

    /// Set transport configuration
    pub fn transport(mut self, transport: TransportConfig) -> Self {
        self.config.transport = transport;
        self
    }

    /// Set network configuration
    pub fn network(mut self, network: NetworkConfig) -> Self {
        self.config.network = network;
        self
    }

    /// Set shared memory configuration
    pub fn shared_memory(mut self, shared_memory: SharedMemoryConfig) -> Self {
        self.config.shared_memory = shared_memory;
        self
    }

    /// Set security configuration
    pub fn security(mut self, security: SecurityConfig) -> Self {
        self.config.security = security;
        self
    }

    /// Set performance configuration
    pub fn performance(mut self, performance: PerformanceConfig) -> Self {
        self.config.performance = performance;
        self
    }

    /// Set mesh configuration
    pub fn mesh(mut self, mesh: MeshConfiguration) -> Self {
        self.config.mesh = mesh;
        self
    }

    /// Set discovery configuration
    pub fn discovery(mut self, discovery: DiscoveryConfiguration) -> Self {
        self.config.discovery = discovery;
        self
    }

    /// Build and validate the configuration
    pub fn build(self) -> Result<CommyConfig> {
        self.validate()?;
        Ok(self.config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        // Validate ports are in valid range
        if self.config.manager.listen_port == 0 {
            return Err(anyhow::anyhow!("Manager listen port cannot be 0"));
        }

        if self.config.network.port == 0 {
            return Err(anyhow::anyhow!("Network port cannot be 0"));
        }

        // Validate file size limits
        if self.config.manager.max_file_size == 0 {
            return Err(anyhow::anyhow!("Max file size cannot be 0"));
        }

        if self.config.shared_memory.max_file_size == 0 {
            return Err(anyhow::anyhow!("Shared memory max file size cannot be 0"));
        }

        // Validate performance thresholds
        let thresholds = &self.config.transport.performance_thresholds;
        if thresholds.min_success_rate < 0.0 || thresholds.min_success_rate > 1.0 {
            return Err(anyhow::anyhow!("Success rate must be between 0.0 and 1.0"));
        }

        if thresholds.max_cpu_usage < 0.0 || thresholds.max_cpu_usage > 100.0 {
            return Err(anyhow::anyhow!("CPU usage must be between 0.0 and 100.0"));
        }

        Ok(())
    }
}

// Default implementations
// CommyConfig and ConfigBuilder derive Default above where the derived
// implementation is equivalent to the manual implementations.

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            listen_port: 8080,
            bind_address: "127.0.0.1".to_string(),
            max_files: 1000,
            max_file_size: 1024 * 1024 * 1024, // 1GB
            default_ttl_seconds: 3600,         // 1 hour
            heartbeat_timeout_seconds: 30,
            cleanup_interval_seconds: 300, // 5 minutes
            files_directory: PathBuf::from("./commy_files"),
        }
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            default_preference: TransportPreference::PreferLocal,
            performance_thresholds: PerformanceThresholds::default(),
            auto_optimization: true,
            fallback_behavior: FallbackBehavior::TryOnce,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8081,
            tls_config: None,
            connection_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            keep_alive: true,
            nodelay: true,
        }
    }
}

impl Default for SharedMemoryConfig {
    fn default() -> Self {
        Self {
            base_path: "./commy_shared".to_string(),
            max_file_size: 1024 * 1024 * 100, // 100MB
            auto_cleanup: true,
            file_permissions: 0o644,
            enable_locking: true,
            sync_strategy: SyncStrategy::Periodic { interval_ms: 1000 },
            enable_optimizations: true,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: false,
            jwt_secret: None,
            token_lifetime: Duration::from_secs(3600),
            enable_encryption: false,
            tls: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            high_performance_mode: false,
            tier: PerformanceTier::Standard,
            buffer_sizes: BufferSizes::default(),
            threading: ThreadingConfig::default(),
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_latency_us: 10000, // 10ms
            min_throughput_mbps: 10,
            max_cpu_usage: 80.0,
            max_memory_mb: 1024,
            min_success_rate: 0.95,
        }
    }
}

impl Default for BufferSizes {
    fn default() -> Self {
        Self {
            read_buffer: 8192,
            write_buffer: 8192,
            network_buffer: 65536,
            shared_memory_buffer: 1024 * 1024,
        }
    }
}

impl Default for ThreadingConfig {
    fn default() -> Self {
        Self {
            worker_threads: None, // Use default from tokio
            work_stealing: true,
            stack_size: None,
        }
    }
}

impl Default for MeshConfiguration {
    fn default() -> Self {
        Self {
            max_nodes: 100,
            heartbeat_interval: Duration::from_secs(30),
            node_timeout: Duration::from_secs(120),
            registration_timeout: Duration::from_secs(10),
            auto_discovery: true,
            port_range: (9000, 9100),
        }
    }
}

impl Default for DiscoveryConfiguration {
    fn default() -> Self {
        Self {
            broadcast_interval: Duration::from_secs(60),
            service_ttl: Duration::from_secs(300),
            max_services_per_node: 50,
            enable_health_checks: true,
            health_check_interval: Duration::from_secs(30),
            announcement_retries: 3,
        }
    }
}

// ConfigBuilder derives Default above.
