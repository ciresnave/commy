//! Transport Layer Implementation
//!
//! This module provides the intelligent transport selection and routing
//! between shared memory and network communication based on performance
//! requirements and locality.

use super::*;
use crate::manager::protocol::ProtocolHandler;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use tokio::net::TcpStream;
#[cfg(feature = "network")]
use tokio_rustls::{client::TlsStream, TlsConnector};

/// Transport abstraction that can handle multiple communication methods
#[derive(Debug)]
pub struct TransportManager {
    /// Local shared memory transport
    pub local_transport: SharedMemoryTransport,

    /// Network transport for remote communication
    pub network_transport: NetworkTransport,

    /// Performance monitor for making routing decisions
    pub performance_monitor: PerformanceMonitor,

    /// Current transport configuration (shared)
    pub config: Arc<TransportConfig>,
}

/// Configuration for transport behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Default transport preference
    pub default_preference: TransportPreference,

    /// Performance thresholds for automatic routing decisions
    pub performance_thresholds: PerformanceThresholds,

    /// Network configuration
    pub network_config: NetworkConfig,

    /// Shared memory configuration
    pub shared_memory_config: SharedMemoryConfig,

    /// Enable automatic optimization based on metrics
    pub auto_optimization: bool,

    /// Fallback behavior when preferred transport fails
    pub fallback_behavior: FallbackBehavior,
}

impl TransportConfig {
    /// Create a new builder for TransportConfig
    pub fn builder() -> TransportConfigBuilder {
        TransportConfigBuilder::default()
    }
}

/// Builder for TransportConfig with sensible defaults
#[derive(Debug, Default)]
pub struct TransportConfigBuilder {
    default_preference: Option<TransportPreference>,
    performance_thresholds: Option<PerformanceThresholds>,
    network_config: Option<NetworkConfig>,
    shared_memory_config: Option<SharedMemoryConfig>,
    auto_optimization: Option<bool>,
    fallback_behavior: Option<FallbackBehavior>,
}

impl TransportConfigBuilder {
    /// Set the default transport preference
    pub fn default_preference(mut self, preference: TransportPreference) -> Self {
        self.default_preference = Some(preference);
        self
    }

    /// Set performance thresholds
    pub fn performance_thresholds(mut self, thresholds: PerformanceThresholds) -> Self {
        self.performance_thresholds = Some(thresholds);
        self
    }

    /// Set network configuration
    pub fn network_config(mut self, config: NetworkConfig) -> Self {
        self.network_config = Some(config);
        self
    }

    /// Set shared memory configuration
    pub fn shared_memory_config(mut self, config: SharedMemoryConfig) -> Self {
        self.shared_memory_config = Some(config);
        self
    }

    /// Enable or disable auto optimization
    pub fn auto_optimization(mut self, enabled: bool) -> Self {
        self.auto_optimization = Some(enabled);
        self
    }

    /// Set fallback behavior
    pub fn fallback_behavior(mut self, behavior: FallbackBehavior) -> Self {
        self.fallback_behavior = Some(behavior);
        self
    }

    /// Build the TransportConfig with validation
    pub fn build(self) -> Result<TransportConfig, TransportError> {
        Ok(TransportConfig {
            default_preference: self
                .default_preference
                .unwrap_or(TransportPreference::AutoOptimize),
            performance_thresholds: self.performance_thresholds.unwrap_or_default(),
            network_config: self.network_config.unwrap_or_default(),
            shared_memory_config: self.shared_memory_config.unwrap_or_default(),
            auto_optimization: self.auto_optimization.unwrap_or(true),
            fallback_behavior: self
                .fallback_behavior
                .unwrap_or(FallbackBehavior::FallbackOnce),
        })
    }
}

/// Performance thresholds for routing decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Latency threshold in microseconds - prefer local if below this
    pub latency_local_threshold_us: u32,

    /// Latency threshold in microseconds - switch to network if above this
    pub latency_network_threshold_us: u32,

    /// Throughput threshold in MB/s - prefer network if above this
    pub throughput_network_threshold_mbps: u32,

    /// Message size threshold - use network for large messages
    pub large_message_threshold_bytes: u64,

    /// Connection count threshold - use network for many connections
    pub high_connection_threshold: u32,
}

/// Network transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Default port for network connections
    pub default_port: u16,

    /// Network endpoints to connect to
    pub endpoints: Vec<String>,

    /// Connection timeout in seconds
    pub connection_timeout_seconds: u32,

    /// Read timeout in seconds
    pub read_timeout_seconds: u32,

    /// Write timeout in seconds
    pub write_timeout_seconds: u32,

    /// Enable TCP keepalive
    pub tcp_keepalive: bool,

    /// TCP keepalive interval in seconds
    pub keepalive_interval_seconds: u32,

    /// Maximum concurrent connections
    pub max_connections: u32,

    /// Connection pool size
    pub connection_pool_size: u32,

    /// Enable Nagle's algorithm
    pub tcp_nodelay: bool,

    /// TLS configuration
    pub tls_config: TlsConfig,
}

/// TLS configuration for secure network communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS encryption
    pub enabled: bool,

    /// Require TLS for all connections
    pub required: bool,

    /// Path to CA certificate file
    pub ca_cert_path: Option<PathBuf>,

    /// Path to client certificate file
    pub client_cert_path: Option<PathBuf>,

    /// Path to client private key file
    pub client_key_path: Option<PathBuf>,

    /// Server name for SNI
    pub server_name: Option<String>,

    /// Minimum TLS version
    pub min_version: TlsVersion,

    /// Allowed cipher suites
    pub cipher_suites: Vec<String>,

    /// Enable certificate verification
    pub verify_certificates: bool,
}

/// TLS protocol versions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TlsVersion {
    /// TLS 1.2
    Tls12,
    /// TLS 1.3
    Tls13,
}

/// Shared memory transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryConfig {
    /// Directory for shared memory files
    pub files_directory: PathBuf,

    /// Default file size in bytes
    pub default_file_size: u64,

    /// Maximum file size in bytes
    pub max_file_size: u64,

    /// File creation permissions (Unix)
    pub file_permissions: u32,

    /// Enable file locking
    pub enable_locking: bool,

    /// Sync strategy for file writes
    pub sync_strategy: SyncStrategy,

    /// Enable memory mapping optimizations
    pub enable_optimizations: bool,
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

/// Fallback behavior when transport fails
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FallbackBehavior {
    /// Fail immediately if preferred transport not available
    Fail,
    /// Try alternative transport once
    FallbackOnce,
    /// Keep trying alternative transports
    KeepTrying,
    /// Use best available transport
    BestAvailable,
}

// Re-export SharedMemoryTransport from shared_memory module
pub use super::shared_memory::SharedMemoryTransport;

/// Information about an active shared memory mapping
#[derive(Debug)]
pub struct SharedMemoryMapping {
    /// File path
    pub path: PathBuf,

    /// File size
    pub size: u64,

    /// Memory map handle
    pub map: memmap2::MmapMut,

    /// Access permissions
    pub permissions: AccessPermissions,

    /// Lock for synchronization
    pub lock: Arc<RwLock<()>>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last access timestamp
    pub last_access: Arc<RwLock<DateTime<Utc>>>,

    /// Usage statistics
    pub stats: Arc<RwLock<MappingStats>>,
}

/// Access permissions for shared memory
#[derive(Debug, Clone, PartialEq)]
pub enum AccessPermissions {
    /// Read-only access
    ReadOnly,
    /// Write-only access
    WriteOnly,
    /// Read and write access
    ReadWrite,
}

/// Statistics for a shared memory mapping
#[derive(Debug, Clone, Default)]
pub struct MappingStats {
    /// Number of read operations
    pub read_count: u64,

    /// Number of write operations
    pub write_count: u64,

    /// Total bytes read
    pub bytes_read: u64,

    /// Total bytes written
    pub bytes_written: u64,

    /// Average read latency in nanoseconds
    pub avg_read_latency_ns: f64,

    /// Average write latency in nanoseconds
    pub avg_write_latency_ns: f64,
}

/// Network transport implementation
pub struct NetworkTransport {
    /// Configuration
    pub config: NetworkConfig,

    /// Active connections
    pub active_connections: Arc<DashMap<String, Arc<NetworkConnection>>>,

    /// Connection pool
    pub connection_pool: Arc<RwLock<Vec<PooledConnection>>>,

    /// Performance metrics
    pub metrics: Arc<RwLock<TransportMetrics>>,

    /// TLS connector for secure connections
    #[cfg(feature = "network")]
    pub tls_connector: Option<TlsConnector>,
}

impl std::fmt::Debug for NetworkTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkTransport")
            .field("config", &self.config)
            .field("active_connections", &self.active_connections)
            .field("connection_pool", &self.connection_pool)
            .field("metrics", &self.metrics)
            .field("tls_connector", &"TlsConnector { ... }")
            .finish()
    }
}

/// Information about an active network connection
#[derive(Debug)]
pub struct NetworkConnection {
    /// Remote endpoint
    pub endpoint: NetworkEndpoint,

    /// Connection stream
    pub stream: NetworkStream,

    /// Connection state
    pub state: ConnectionState,

    /// Protocol handler
    pub protocol: ProtocolHandler,

    /// Connection statistics
    pub stats: Arc<RwLock<ConnectionStats>>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_activity: Arc<RwLock<DateTime<Utc>>>,
}

/// Network stream variants
#[derive(Debug)]
pub enum NetworkStream {
    /// Plain TCP stream
    Tcp(TcpStream),

    /// TLS-encrypted stream
    #[cfg(feature = "network")]
    Tls(Box<TlsStream<TcpStream>>),
}

/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,

    /// Connection is active and healthy
    Connected,

    /// Connection is degraded but functional
    Degraded,

    /// Connection is in error state
    Error,

    /// Connection is being closed
    Closing,

    /// Connection is closed
    Closed,
}

/// Statistics for a network connection
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Number of messages sent
    pub messages_sent: u64,

    /// Number of messages received
    pub messages_received: u64,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Average round-trip latency in microseconds
    pub avg_rtt_us: f64,

    /// Connection uptime in seconds
    pub uptime_seconds: u64,

    /// Number of reconnections
    pub reconnect_count: u32,
}

/// Pooled connection for reuse
#[derive(Debug)]
pub struct PooledConnection {
    /// Connection ID
    pub id: String,

    /// Network connection
    pub connection: NetworkConnection,

    /// Whether this connection is currently in use
    pub in_use: bool,

    /// Last used timestamp
    pub last_used: DateTime<Utc>,
}

/// Performance monitor for routing decisions
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// Historical performance data
    pub history: Arc<RwLock<PerformanceHistory>>,

    /// Current performance metrics
    pub current_metrics: Arc<RwLock<PerformanceSnapshot>>,

    /// Performance thresholds
    pub thresholds: PerformanceThresholds,
}

/// Historical performance data
#[derive(Debug)]
pub struct PerformanceHistory {
    /// Shared memory performance samples
    pub local_samples: Vec<PerformanceSample>,

    /// Network performance samples
    pub network_samples: Vec<PerformanceSample>,

    /// Maximum number of samples to keep
    pub max_samples: usize,
}

/// Performance sample data point
#[derive(Debug, Clone)]
pub struct PerformanceSample {
    /// Timestamp of the sample
    pub timestamp: DateTime<Utc>,

    /// Latency in microseconds
    pub latency_us: f64,

    /// Throughput in MB/s
    pub throughput_mbps: f64,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,

    /// Connection count at time of sample
    pub connection_count: u32,

    /// Message size in bytes
    pub message_size: u64,
}

/// Current performance snapshot
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// Local transport metrics
    pub local: TransportMetrics,

    /// Network transport metrics
    pub network: TransportMetrics,

    /// Timestamp of this snapshot
    pub timestamp: DateTime<Utc>,
}

/// Transport performance metrics
#[derive(Debug, Clone, Default)]
pub struct TransportMetrics {
    /// Average latency in microseconds
    pub avg_latency_us: f64,

    /// 95th percentile latency in microseconds
    pub p95_latency_us: f64,

    /// 99th percentile latency in microseconds
    pub p99_latency_us: f64,

    /// Average throughput in MB/s
    pub avg_throughput_mbps: f64,

    /// Peak throughput in MB/s
    pub peak_throughput_mbps: f64,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,

    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,

    /// Total operations
    pub total_operations: u64,

    /// Active connections
    pub active_connections: u32,
}

/// Transport routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected transport type
    pub transport: SelectedTransport,

    /// Reason for the decision
    pub reason: RoutingReason,

    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,

    /// Expected performance profile
    pub expected_performance: PerformanceProfile,
}

/// Selected transport type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectedTransport {
    /// Use shared memory transport
    SharedMemory,

    /// Use network transport
    Network,

    /// Use hybrid approach
    Hybrid {
        primary: Box<SelectedTransport>,
        fallback: Box<SelectedTransport>,
    },
}

/// Reason for routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingReason {
    /// User specified preference
    UserPreference,

    /// Performance optimization
    PerformanceOptimization { metric: String },

    /// Locality constraint
    LocalityConstraint,

    /// Resource availability
    ResourceAvailability,

    /// Fallback due to failure
    Fallback { original_failure: String },

    /// Load balancing
    LoadBalancing,

    /// Default behavior
    Default,
}

// Default implementations
impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            default_preference: TransportPreference::PreferLocal,
            performance_thresholds: PerformanceThresholds::default(),
            network_config: NetworkConfig::default(),
            shared_memory_config: SharedMemoryConfig::default(),
            auto_optimization: true,
            fallback_behavior: FallbackBehavior::FallbackOnce,
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            latency_local_threshold_us: 100,            // 100 microseconds
            latency_network_threshold_us: 10_000,       // 10 milliseconds
            throughput_network_threshold_mbps: 100,     // 100 MB/s
            large_message_threshold_bytes: 1024 * 1024, // 1MB
            high_connection_threshold: 50,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            default_port: 8080,
            endpoints: vec!["localhost:8080".to_string()],
            connection_timeout_seconds: 30,
            read_timeout_seconds: 30,
            write_timeout_seconds: 30,
            tcp_keepalive: true,
            keepalive_interval_seconds: 30,
            max_connections: 1000,
            connection_pool_size: 10,
            tcp_nodelay: true,
            tls_config: TlsConfig::default(),
        }
    }
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            required: false,
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
            server_name: None,
            min_version: TlsVersion::Tls12,
            cipher_suites: vec![],
            verify_certificates: true,
        }
    }
}

impl Default for SharedMemoryConfig {
    fn default() -> Self {
        Self {
            files_directory: PathBuf::from("./commy_files"),
            default_file_size: 1024 * 1024,    // 1MB
            max_file_size: 1024 * 1024 * 1024, // 1GB
            file_permissions: 0o600,
            enable_locking: true,
            sync_strategy: SyncStrategy::Periodic { interval_ms: 1000 },
            enable_optimizations: true,
        }
    }
}

// default derived

// default derived

// default derived

impl NetworkTransport {
    /// Get reference to the configuration
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    /// Get reference to active connections
    pub fn active_connections(&self) -> &Arc<DashMap<String, Arc<NetworkConnection>>> {
        &self.active_connections
    }

    /// Get reference to metrics
    pub fn metrics(&self) -> &Arc<RwLock<TransportMetrics>> {
        &self.metrics
    }

    /// Get reference to TLS connector
    #[cfg(feature = "network")]
    pub fn tls_connector(&self) -> &Option<TlsConnector> {
        &self.tls_connector
    }
}

impl PerformanceMonitor {
    /// Get reference to current metrics
    pub fn current_metrics(&self) -> &Arc<RwLock<PerformanceSnapshot>> {
        &self.current_metrics
    }

    /// Get reference to history
    pub fn history(&self) -> &Arc<RwLock<PerformanceHistory>> {
        &self.history
    }
}

impl PerformanceHistory {
    /// Get reference to local samples
    pub fn local_samples(&self) -> &Vec<PerformanceSample> {
        &self.local_samples
    }

    /// Get mutable reference to local samples
    pub fn local_samples_mut(&mut self) -> &mut Vec<PerformanceSample> {
        &mut self.local_samples
    }

    /// Get reference to network samples
    pub fn network_samples(&self) -> &Vec<PerformanceSample> {
        &self.network_samples
    }

    /// Get mutable reference to network samples
    pub fn network_samples_mut(&mut self) -> &mut Vec<PerformanceSample> {
        &mut self.network_samples
    }

    /// Get max samples limit
    pub fn max_samples(&self) -> usize {
        self.max_samples
    }
}

impl TransportManager {
    /// Get reference to the performance monitor
    pub fn performance_monitor(&self) -> &PerformanceMonitor {
        &self.performance_monitor
    }

    /// Get reference to the configuration
    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    /// Get reference to the local transport
    pub fn local_transport(&self) -> &SharedMemoryTransport {
        &self.local_transport
    }

    /// Get reference to the network transport
    pub fn network_transport(&self) -> &NetworkTransport {
        &self.network_transport
    }
}
