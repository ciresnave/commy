//! Commy Server Clustering
//!
//! Enables multiple Commy servers to operate in a cluster with service state
//! replication and automatic client failover.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  Clustering Module                                      │
//! ├─────────────────────────────────────────────────────────┤
//! │  registry:       Peer discovery and health checks       │
//! │  peer:           Peer information and management        │
//! │  connection:     Inter-server connections              │
//! │  messages:       Server-to-server protocol             │
//! │  replication:    Service state transfer               │
//! │  consistency:    Conflict resolution and ordering     │
//! └─────────────────────────────────────────────────────────┘
//! ```

pub mod client_failover;
pub mod config;
pub mod conflict_resolution;
pub mod connection;
pub mod consistency;
pub mod failover_manager;
pub mod heartbeat;
pub mod messages;
pub mod peer;
pub mod protocol;
pub mod registry;
pub mod replication;
pub mod session_persistence;
pub mod snapshots;
pub mod vector_clocks;

// Re-export commonly used types
pub use client_failover::{
    ClientFailoverConfig, ClientFailoverDetector, ClientFailoverState, ServerCandidate,
    ServerConnection,
};
pub use config::{
    ClusteringConfig, ConflictResolutionStrategy, ConsistencyConfig, NetworkConfig,
    ReplicationConfig, ServerNode, StorageBackend, StorageConfig, TlsConfig,
};
pub use conflict_resolution::{
    ApplicationDefinedResolver, ConflictResolutionConfig, ConflictResolver, LastWriteWinsResolver,
    MergeResolver, ServerPriorityResolver,
};
pub use connection::ConnectionPool;
pub use consistency::{ConflictInfo, VariableVersion, VersionedServiceMetadata};
pub use failover_manager::{
    FailoverConfig, FailoverManager, FailureReason, MigrationStatus, ServiceMigration,
    TargetSelection,
};
pub use heartbeat::{HeartbeatConfig, HeartbeatService};
pub use messages::{PeerMessageEnvelope, ServerMessage, ServerStatus};
pub use peer::{PeerInfo, PeerStatus};
pub use protocol::{ProtocolConfig, ProtocolHandler};
pub use registry::{ClusterStatus, PeerConfig, PeerRegistry};
pub use replication::ReplicationCoordinator;
pub use session_persistence::{SessionData, SessionStore};
pub use snapshots::{ServiceSnapshot, SnapshotMetadata, SnapshotTransfer};
pub use vector_clocks::{VectorClock, VersionedValue, apply_remote_write};
