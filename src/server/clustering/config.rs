/// Clustering configuration for multi-server Commy deployments
///
/// This module defines all configuration options for running Commy in a clustered mode,
/// including server nodes, replication settings, and consistency strategies.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

/// Top-level clustering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringConfig {
    /// Unique identifier for this server in the cluster
    pub server_id: String,

    /// List of all servers in the cluster
    pub nodes: Vec<ServerNode>,

    /// Replication settings
    pub replication: ReplicationConfig,

    /// Consistency strategy for conflict resolution
    pub consistency: ConsistencyConfig,

    /// Network and heartbeat settings
    pub network: NetworkConfig,

    /// Storage backend for cluster metadata
    pub storage: StorageConfig,
}

/// Configuration for a single server node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNode {
    /// Unique server ID (e.g., "server-1", "server-2")
    pub id: String,

    /// Address for inter-server communication
    pub address: SocketAddr,

    /// TLS configuration (optional)
    #[serde(default)]
    pub tls: Option<TlsConfig>,

    /// Maximum concurrent replication connections from this peer
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

/// TLS configuration for secure inter-server communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file (PEM format)
    pub cert_path: String,

    /// Path to private key file (PEM format)
    pub key_path: String,

    /// Path to CA certificate (PEM format, optional)
    pub ca_path: Option<String>,

    /// Verify server certificates
    #[serde(default = "default_true")]
    pub verify_peer: bool,
}

/// Replication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationConfig {
    /// How often to sync with peers (in milliseconds)
    #[serde(default = "default_sync_interval_ms")]
    pub sync_interval_ms: u64,

    /// Batch size for replication messages
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Maximum number of concurrent replication tasks
    #[serde(default = "default_max_replication_tasks")]
    pub max_concurrent_tasks: usize,

    /// Enable gossip-based eventual consistency
    #[serde(default = "default_true")]
    pub gossip_enabled: bool,

    /// Timeout for peer replication responses (in milliseconds)
    #[serde(default = "default_replication_timeout_ms")]
    pub replication_timeout_ms: u64,

    /// Enable checksumming for data integrity
    #[serde(default = "default_true")]
    pub enable_checksums: bool,

    /// Resume incomplete replication from checkpoint
    #[serde(default = "default_true")]
    pub enable_resume: bool,
}

/// Consistency strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyConfig {
    /// Conflict resolution strategy
    pub strategy: ConflictResolutionStrategy,

    /// Enable vector clocks for causality tracking
    #[serde(default = "default_true")]
    pub enable_vector_clocks: bool,

    /// Require quorum before writing (if supported by strategy)
    #[serde(default)]
    pub quorum_size: Option<usize>,

    /// Enable conflict detection and reporting
    #[serde(default = "default_true")]
    pub detect_conflicts: bool,

    /// Log all conflicts to a separate file for analysis
    #[serde(default = "default_conflict_log")]
    pub conflict_log_enabled: bool,

    /// Path to conflict log file (empty = no logging)
    #[serde(default)]
    pub conflict_log_path: String,
}

/// Conflict resolution strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolutionStrategy {
    /// Last-Write-Wins with server ID tie-breaking (deterministic)
    #[serde(rename = "last_write_wins")]
    LastWriteWins {
        /// Server ID to use for tie-breaking (if not specified, use alphabetical order)
        #[serde(default)]
        tie_breaker: Option<String>,
    },

    /// Server priority order (first in list wins)
    #[serde(rename = "server_priority")]
    ServerPriority {
        /// Ordered list of server IDs by priority
        priority_order: Vec<String>,
    },

    /// Application-defined resolution (requires app to implement handler)
    #[serde(rename = "application_defined")]
    ApplicationDefined {
        /// Optional hints for the application
        #[serde(default)]
        hints: HashMap<String, String>,
    },

    /// Merge-based resolution (for compatible types)
    #[serde(rename = "merge")]
    Merge {
        /// Strategy for merging (e.g., "union", "intersection", "custom")
        #[serde(default = "default_merge_strategy")]
        merge_type: String,
    },

    /// Custom strategy (application-specific)
    #[serde(rename = "custom")]
    Custom {
        /// Name of custom handler registered with the application
        handler_name: String,

        /// Configuration options for the handler
        #[serde(default)]
        config: HashMap<String, String>,
    },
}

/// Network and heartbeat configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Heartbeat interval (in milliseconds)
    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,

    /// Heartbeat timeout (in milliseconds)
    #[serde(default = "default_heartbeat_timeout_ms")]
    pub heartbeat_timeout_ms: u64,

    /// Maximum consecutive heartbeat failures before marking peer as dead
    #[serde(default = "default_max_heartbeat_failures")]
    pub max_heartbeat_failures: u32,

    /// Enable connection pooling for inter-server communication
    #[serde(default = "default_true")]
    pub connection_pooling: bool,

    /// Maximum messages in outbound queue before applying backpressure
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,

    /// Enable TCP keepalive
    #[serde(default = "default_true")]
    pub tcp_keepalive: bool,

    /// TCP keepalive interval (in milliseconds)
    #[serde(default = "default_tcp_keepalive_ms")]
    pub tcp_keepalive_ms: u64,
}

/// Storage configuration for cluster metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    pub backend: StorageBackend,

    /// Enable persistence of cluster state
    #[serde(default = "default_true")]
    pub persist_cluster_state: bool,

    /// Directory for local cluster metadata
    #[serde(default = "default_metadata_dir")]
    pub metadata_dir: String,

    /// Enable snapshots of cluster state
    #[serde(default = "default_true")]
    pub enable_snapshots: bool,

    /// Interval between snapshots (in milliseconds)
    #[serde(default = "default_snapshot_interval_ms")]
    pub snapshot_interval_ms: u64,
}

/// Supported storage backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    /// In-memory storage (development only)
    #[serde(rename = "memory")]
    Memory,

    /// SQLite for local metadata
    #[serde(rename = "sqlite")]
    Sqlite {
        /// Path to SQLite database file
        db_path: String,
    },

    /// PostgreSQL for distributed cluster state
    #[serde(rename = "postgresql")]
    PostgreSQL {
        /// Connection URL
        url: String,

        /// Maximum connections
        #[serde(default = "default_db_max_connections")]
        max_connections: u32,
    },

    /// MySQL for distributed cluster state
    #[serde(rename = "mysql")]
    MySQL {
        /// Connection URL
        url: String,

        /// Maximum connections
        #[serde(default = "default_db_max_connections")]
        max_connections: u32,
    },

    /// Redis for high-performance cluster state
    #[serde(rename = "redis")]
    Redis {
        /// Connection URL
        url: String,
    },
}

// Default value functions for serde
fn default_max_connections() -> usize {
    10
}

fn default_true() -> bool {
    true
}

fn default_sync_interval_ms() -> u64 {
    100
}

fn default_batch_size() -> usize {
    1000
}

fn default_max_replication_tasks() -> usize {
    4
}

fn default_replication_timeout_ms() -> u64 {
    5000
}

fn default_conflict_log() -> bool {
    false
}

fn default_merge_strategy() -> String {
    "union".to_string()
}

fn default_heartbeat_interval_ms() -> u64 {
    1000
}

fn default_heartbeat_timeout_ms() -> u64 {
    3000
}

fn default_max_heartbeat_failures() -> u32 {
    3
}

fn default_max_queue_size() -> usize {
    10000
}

fn default_true_val() -> bool {
    true
}

fn default_tcp_keepalive_ms() -> u64 {
    30000
}

fn default_metadata_dir() -> String {
    ".commy/cluster".to_string()
}

fn default_snapshot_interval_ms() -> u64 {
    60000
}

fn default_db_max_connections() -> u32 {
    10
}

// Builder pattern for convenient configuration
impl ClusteringConfig {
    /// Create a new clustering configuration with the given server ID
    pub fn new(server_id: String) -> Self {
        Self {
            server_id,
            nodes: Vec::new(),
            replication: ReplicationConfig::default(),
            consistency: ConsistencyConfig::default(),
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
        }
    }

    /// Add a server node to the cluster
    pub fn add_node(mut self, node: ServerNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// Set the replication configuration
    pub fn with_replication(mut self, replication: ReplicationConfig) -> Self {
        self.replication = replication;
        self
    }

    /// Set the consistency strategy
    pub fn with_consistency(mut self, consistency: ConsistencyConfig) -> Self {
        self.consistency = consistency;
        self
    }

    /// Set the network configuration
    pub fn with_network(mut self, network: NetworkConfig) -> Self {
        self.network = network;
        self
    }

    /// Set the storage configuration
    pub fn with_storage(mut self, storage: StorageConfig) -> Self {
        self.storage = storage;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Check that server_id exists in nodes
        if !self.nodes.iter().any(|n| n.id == self.server_id) {
            return Err(format!(
                "Server ID '{}' not found in nodes list",
                self.server_id
            ));
        }

        // Check that we have at least 1 node
        if self.nodes.is_empty() {
            return Err("Cluster must have at least 1 node".to_string());
        }

        // Check that all node IDs are unique
        let mut seen = std::collections::HashSet::new();
        for node in &self.nodes {
            if !seen.insert(&node.id) {
                return Err(format!("Duplicate node ID: '{}'", node.id));
            }
        }

        // Validate consistency config
        if let ConflictResolutionStrategy::ServerPriority { priority_order } =
            &self.consistency.strategy
        {
            for server_id in priority_order {
                if !self.nodes.iter().any(|n| &n.id == server_id) {
                    return Err(format!(
                        "Server '{}' in priority order not found in nodes",
                        server_id
                    ));
                }
            }
        }

        Ok(())
    }

    /// Create a configuration for a 3-node cluster
    pub fn three_node_cluster(server_id: String) -> Self {
        let mut config = Self::new(server_id);
        config.nodes = vec![
            ServerNode {
                id: "server-1".to_string(),
                address: "127.0.0.1:9001".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
            ServerNode {
                id: "server-2".to_string(),
                address: "127.0.0.1:9002".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
            ServerNode {
                id: "server-3".to_string(),
                address: "127.0.0.1:9003".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
        ];
        config
    }

    /// Create a configuration for a 5-node cluster
    pub fn five_node_cluster(server_id: String) -> Self {
        let mut config = Self::new(server_id);
        config.nodes = vec![
            ServerNode {
                id: "server-1".to_string(),
                address: "127.0.0.1:9001".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
            ServerNode {
                id: "server-2".to_string(),
                address: "127.0.0.1:9002".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
            ServerNode {
                id: "server-3".to_string(),
                address: "127.0.0.1:9003".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
            ServerNode {
                id: "server-4".to_string(),
                address: "127.0.0.1:9004".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
            ServerNode {
                id: "server-5".to_string(),
                address: "127.0.0.1:9005".parse().unwrap(),
                tls: None,
                max_connections: 10,
            },
        ];
        config
    }
}

// Default implementations
impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            sync_interval_ms: 100,
            batch_size: 1000,
            max_concurrent_tasks: 4,
            gossip_enabled: true,
            replication_timeout_ms: 5000,
            enable_checksums: true,
            enable_resume: true,
        }
    }
}

impl Default for ConsistencyConfig {
    fn default() -> Self {
        Self {
            strategy: ConflictResolutionStrategy::LastWriteWins { tie_breaker: None },
            enable_vector_clocks: true,
            quorum_size: None,
            detect_conflicts: true,
            conflict_log_enabled: false,
            conflict_log_path: String::new(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: 1000,
            heartbeat_timeout_ms: 3000,
            max_heartbeat_failures: 3,
            connection_pooling: true,
            max_queue_size: 10000,
            tcp_keepalive: true,
            tcp_keepalive_ms: 30000,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Memory,
            persist_cluster_state: true,
            metadata_dir: ".commy/cluster".to_string(),
            enable_snapshots: true,
            snapshot_interval_ms: 60000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = ClusteringConfig::new("server-1".to_string());
        assert_eq!(config.server_id, "server-1");
    }

    #[test]
    fn test_three_node_cluster() {
        let config = ClusteringConfig::three_node_cluster("server-1".to_string());
        assert_eq!(config.nodes.len(), 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_five_node_cluster() {
        let config = ClusteringConfig::five_node_cluster("server-2".to_string());
        assert_eq!(config.nodes.len(), 5);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_missing_server_id() {
        let config = ClusteringConfig::new("server-99".to_string()).add_node(ServerNode {
            id: "server-1".to_string(),
            address: "127.0.0.1:9001".parse().unwrap(),
            tls: None,
            max_connections: 10,
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_duplicate_ids() {
        let config = ClusteringConfig::new("server-1".to_string())
            .add_node(ServerNode {
                id: "server-1".to_string(),
                address: "127.0.0.1:9001".parse().unwrap(),
                tls: None,
                max_connections: 10,
            })
            .add_node(ServerNode {
                id: "server-1".to_string(),
                address: "127.0.0.1:9002".parse().unwrap(),
                tls: None,
                max_connections: 10,
            });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_builder_pattern() {
        let config = ClusteringConfig::new("server-1".to_string())
            .add_node(ServerNode {
                id: "server-1".to_string(),
                address: "127.0.0.1:9001".parse().unwrap(),
                tls: None,
                max_connections: 10,
            })
            .with_consistency(ConsistencyConfig {
                strategy: ConflictResolutionStrategy::LastWriteWins { tie_breaker: None },
                enable_vector_clocks: true,
                quorum_size: None,
                detect_conflicts: true,
                conflict_log_enabled: false,
                conflict_log_path: String::new(),
            });

        assert_eq!(config.nodes.len(), 1);
        assert!(config.consistency.enable_vector_clocks);
    }

    #[test]
    fn test_server_priority_validation() {
        let config = ClusteringConfig::new("server-1".to_string())
            .add_node(ServerNode {
                id: "server-1".to_string(),
                address: "127.0.0.1:9001".parse().unwrap(),
                tls: None,
                max_connections: 10,
            })
            .with_consistency(ConsistencyConfig {
                strategy: ConflictResolutionStrategy::ServerPriority {
                    priority_order: vec!["server-99".to_string()],
                },
                enable_vector_clocks: true,
                quorum_size: None,
                detect_conflicts: true,
                conflict_log_enabled: false,
                conflict_log_path: String::new(),
            });

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_serialization() {
        let config = ClusteringConfig::three_node_cluster("server-1".to_string());
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ClusteringConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.server_id, deserialized.server_id);
        assert_eq!(config.nodes.len(), deserialized.nodes.len());
    }
}
