//! Comprehensive Manager Module for Full Phase 1 Implementation
//! Supporting the complete distributed mesh vision with built-in communication patterns

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod auth_provider;
pub mod coordination;
pub mod core;
pub mod id_manager;
pub mod lifecycle;
pub mod memory_map;
pub mod network;
pub mod object_pool;
pub mod protocol;
pub mod shared_memory;
pub mod transport;
pub mod transport_impl;

/// Built-in communication patterns for optimal performance and consistent semantics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessagePattern {
    // 🔄 Message Exchange Patterns
    /// Request/Response with optional timeout
    RequestResponse {
        timeout_ms: Option<u32>,
        retry_count: Option<u8>,
    },
    /// Fire-and-forget with optional delivery confirmation
    OneWay { delivery_confirmation: bool },
    /// Publish/Subscribe with topic-based routing
    PublishSubscribe {
        topic: String,
        durable: bool,
        filter: Option<String>,
    },
    /// Event-driven with callback semantics
    EventDriven {
        event_types: Vec<String>,
        callback_mode: CallbackMode,
    },
    /// Message queue with work distribution
    MessageQueue {
        queue_name: String,
        worker_group: Option<String>,
        priority_levels: u8,
    },
    /// Fan-out to multiple workers, fan-in results
    FanOutFanIn {
        worker_count: u32,
        aggregation_strategy: AggregationStrategy,
    },
    /// Scatter/Gather with result aggregation
    ScatterGather {
        target_count: u32,
        min_responses: u32,
        timeout_ms: u32,
    },

    // 🧭 Coordination & Synchronization Patterns
    /// Leader/Follower coordination
    LeaderFollower {
        role: LeadershipRole,
        election_strategy: ElectionStrategy,
    },
    /// Observer pattern with state change notifications
    Observer {
        state_tracking: StateTrackingMode,
        notification_mode: NotificationMode,
    },
    /// Barrier synchronization point
    Barrier {
        participant_count: u32,
        timeout_ms: Option<u32>,
    },
    /// Two-phase commit for distributed transactions
    TwoPhaseCommit {
        coordinator_role: bool,
        participant_id: String,
    },
    /// Saga pattern for long-running transactions
    Saga {
        saga_id: String,
        step_number: u32,
        compensation_available: bool,
    },

    // 🗂 Data Sharing & Access Patterns
    /// Shared memory with synchronization
    SharedMemory {
        sync_mode: SynchronizationMode,
        access_pattern: AccessPattern,
    },
    /// Blackboard/shared state system
    Blackboard {
        knowledge_domain: String,
        conflict_resolution: ConflictResolution,
    },
    /// Pipeline processing with stages
    Pipeline {
        stage_number: u32,
        total_stages: u32,
        backpressure_strategy: BackpressureStrategy,
    },
    /// Batch processing with accumulation
    BatchProcessing {
        batch_size: u32,
        timeout_ms: u32,
        processing_strategy: BatchStrategy,
    },
    /// Stream processing with continuous flow
    StreamProcessing {
        window_size_ms: Option<u32>,
        watermark_strategy: WatermarkStrategy,
    },

    // 🌐 Distribution & Integration Patterns
    /// Point-to-point direct messaging
    PointToPoint {
        endpoint_id: String,
        delivery_semantics: DeliverySemantics,
    },
    /// Brokered messaging through intermediary
    BrokeredMessaging {
        broker_type: BrokerType,
        routing_key: Option<String>,
    },
    /// Service mesh communication
    ServiceMesh {
        service_name: String,
        mesh_routing: MeshRouting,
    },
    /// API Gateway aggregation
    ApiGateway {
        route_pattern: String,
        aggregation_rules: Vec<AggregationRule>,
    },
    /// CQRS command/query separation
    CQRS {
        operation_type: CQRSOperation,
        consistency_level: ConsistencyLevel,
    },
    /// Event sourcing with replay capability
    EventSourcing {
        stream_id: String,
        replay_from: Option<u64>,
    },
}

/// Callback execution modes for event-driven patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CallbackMode {
    Synchronous,
    Asynchronous,
    Deferred,
}

/// Result aggregation strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregationStrategy {
    FirstResponse,
    AllResponses,
    MajorityConsensus,
    CustomLogic(String),
}

/// Leadership roles in coordination patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LeadershipRole {
    Leader,
    Follower,
    Candidate,
}

/// Leader election strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ElectionStrategy {
    Raft,
    Bully,
    Ring,
    Custom(String),
}

/// State tracking modes for observer pattern
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateTrackingMode {
    FullState,
    StateDiff,
    EventLog,
}

/// Notification delivery modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationMode {
    Immediate,
    Batched,
    OnChange,
}

/// Memory access patterns for optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccessPattern {
    Sequential,
    Random,
    Producer,
    Consumer,
    ReadMostly,
    WriteMostly,
}

/// Conflict resolution strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictResolution {
    LastWriteWins,
    FirstWriteWins,
    Merge,
    CustomLogic(String),
}

/// Backpressure handling strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackpressureStrategy {
    Block,
    Drop,
    Buffer,
    Overflow,
}

/// Batch processing strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BatchStrategy {
    SizeTriggered,
    TimeTriggered,
    Adaptive,
}

/// Stream watermark strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WatermarkStrategy {
    ProcessingTime,
    EventTime,
    Custom(String),
}

/// Message delivery semantics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliverySemantics {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

/// Broker types for brokered messaging
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BrokerType {
    InMemory,
    Persistent,
    Kafka,
    RabbitMQ,
    Redis,
    Custom(String),
}

/// Service mesh routing strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MeshRouting {
    LoadBalanced,
    StickySession,
    Geolocation,
    Custom(String),
}

/// API Gateway aggregation rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregationRule {
    pub source_field: String,
    pub target_field: String,
    pub transformation: String,
}

/// CQRS operation types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CQRSOperation {
    Command,
    Query,
}

/// Enhanced synchronization modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SynchronizationMode {
    /// No synchronization (fastest, unsafe)
    None,
    /// Mutex-based synchronization
    Mutex,
    /// Read-write lock
    RwLock,
    /// Atomic operations
    Atomic,
    /// Lock-free algorithms
    LockFree,
    /// Pattern-specific optimized sync
    PatternOptimized,
}

/// Comprehensive file request supporting full mesh capabilities with built-in patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFileRequest {
    // Basic identification
    /// Unique identifier for the file
    pub identifier: String,
    /// Optional human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,

    // 🎯 NEW: Built-in Communication Pattern
    /// The communication pattern this request implements
    pub pattern: MessagePattern,
    /// Pattern-specific configuration
    pub pattern_config: std::collections::HashMap<String, String>,

    // Communication patterns (enhanced)
    /// How data flows (ReadOnly, WriteOnly, ReadWrite)
    pub directionality: Directionality,
    /// Connection topology (OneToOne, OneToMany, etc.)
    pub topology: Topology,
    /// Serialization format for this file
    pub serialization: SerializationFormat,
    /// Role in the communication pattern
    pub connection_side: ConnectionSide,

    // File lifecycle management
    /// How to handle file creation
    pub creation_policy: CreationPolicy,
    /// How to handle file existence
    pub existence_policy: ExistencePolicy,
    /// Optional custom file path
    pub file_path: Option<PathBuf>,
    /// Maximum size in bytes
    pub max_size_bytes: Option<u64>,
    /// Time to live in seconds
    pub ttl_seconds: Option<u64>,
    /// Maximum concurrent connections
    pub max_connections: Option<u32>,
    /// Required permissions
    pub required_permissions: Vec<Permission>,

    // Security and persistence
    /// Whether encryption is required
    pub encryption_required: bool,
    /// Whether to auto-cleanup when unused
    pub auto_cleanup: bool,
    /// Whether to persist after last disconnect
    pub persist_after_disconnect: bool,

    // Transport optimization
    /// Transport preference for this file
    pub transport_preference: TransportPreference,
    /// Performance requirements
    pub performance_requirements: PerformanceRequirements,

    // Operation specification
    /// The specific operation to perform
    pub operation: SharedFileOperation,
}

/// Data flow direction for communication optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Directionality {
    /// Read-only access (consumers only)
    ReadOnly,
    /// Write-only access (producers only)
    WriteOnly,
    /// Full read-write access
    ReadWrite,
}

/// Communication topology patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Topology {
    /// Single producer, single consumer
    OneToOne,
    /// Single producer, multiple consumers (broadcast)
    OneToMany,
    /// Multiple producers, single consumer (aggregation)
    ManyToOne,
    /// Multiple producers, multiple consumers (mesh)
    ManyToMany,
}

/// Serialization format selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SerializationFormat {
    /// JSON text format (human readable)
    Json,
    /// Binary format (compact, fast)
    Binary,
    /// MessagePack format (efficient, cross-language)
    MessagePack,
    /// CBOR format (RFC 7049)
    Cbor,
    /// Zero-copy format (maximum performance)
    ZeroCopy,
    /// Compact binary format
    Compact,
}

/// Connection role in communication pattern
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionSide {
    /// Data producer only
    Producer,
    /// Data consumer only
    Consumer,
    /// Both producer and consumer
    ProducerConsumer,
    /// Role-agnostic (determined at runtime)
    Agnostic,
}

/// File creation policies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CreationPolicy {
    /// Always create new file (fail if exists)
    Create,
    /// Never create (fail if doesn't exist)
    NeverCreate,
    /// Create only if it doesn't exist
    CreateIfNotExists,
    /// Create only if authorized
    CreateIfAuthorized,
}

/// Advanced existence policies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExistencePolicy {
    /// File must already exist
    MustExist,
    /// Create file if needed, connect if exists
    CreateOrConnect,
    /// Only create new files
    CreateOnly,
    /// Only connect to existing files
    ConnectOnly,
}

/// Transport preferences for optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransportPreference {
    /// Prefer local shared memory when possible
    PreferLocal,
    /// Prefer network transport (for distributed scenarios)
    PreferNetwork,
    /// Automatically optimize based on requirements
    AutoOptimize,
    /// Force local transport only
    LocalOnly,
    /// Force network transport only
    NetworkOnly,
    /// Require local transport
    RequireLocal,
    /// Require network transport
    RequireNetwork,
    /// Adaptive selection based on performance
    Adaptive,
}

/// Performance requirements specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceRequirements {
    /// Maximum acceptable latency in milliseconds
    pub max_latency_ms: Option<u32>,
    /// Minimum required throughput in MB/s
    pub min_throughput_mbps: Option<u32>,
    /// Data consistency requirements
    pub consistency_level: ConsistencyLevel,
    /// Whether durability is required
    pub durability_required: bool,
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            max_latency_ms: None,
            min_throughput_mbps: None,
            consistency_level: ConsistencyLevel::Eventual,
            durability_required: false,
        }
    }
}

/// Data consistency levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsistencyLevel {
    /// No consistency guarantees
    None,
    /// Eventually consistent
    Eventual,
    /// Strong consistency
    Strong,
    /// Linearizable consistency
    Linearizable,
}

/// File operations that can be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SharedFileOperation {
    /// Read data from file
    Read {
        path: PathBuf,
        offset: u64,
        length: Option<u64>,
    },
    /// Write data to file
    Write {
        path: PathBuf,
        offset: u64,
        data: Vec<u8>,
    },
    /// Append data to file
    Append { path: PathBuf, data: Vec<u8> },
    /// Create new file
    Create {
        path: PathBuf,
        size: u64,
        initial_data: Option<Vec<u8>>,
        permissions: Vec<Permission>,
    },
    /// Delete file
    Delete { path: PathBuf },
    /// Copy file
    Copy { from: PathBuf, to: PathBuf },
    /// Move file
    Move { from: PathBuf, to: PathBuf },
    /// List files in directory
    List { path: PathBuf },
    /// Get file information
    GetInfo { path: PathBuf },
    /// Set file permissions
    SetPermissions {
        path: PathBuf,
        permissions: Vec<Permission>,
    },
    /// Resize file
    Resize { path: PathBuf, new_size: u64 },
}

/// Enhanced permission types with hierarchical access
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    /// Basic read access
    Read,
    /// Basic write access
    Write,
    /// Administrative access
    Admin,
    /// Execute access (for executable files)
    Execute,
    /// Delete permission
    Delete,
    /// Permission to change permissions
    ChangePermissions,
    /// Full ownership
    Owner,
}

/// Operation response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SharedFileOperationResponse {
    /// Successful read operation
    ReadSuccess {
        data: Vec<u8>,
        timestamp: std::time::SystemTime,
    },
    /// Successful write operation
    WriteSuccess {
        bytes_written: u64,
        timestamp: std::time::SystemTime,
    },
    /// Successful create operation
    CreateSuccess {
        file_size: u64,
        timestamp: std::time::SystemTime,
    },
    /// Successful delete operation
    DeleteSuccess { timestamp: std::time::SystemTime },
    /// File information
    InfoSuccess {
        file_size: u64,
        created_at: Option<std::time::SystemTime>,
        modified_at: Option<std::time::SystemTime>,
        timestamp: std::time::SystemTime,
    },
    /// List of files
    ListSuccess {
        files: Vec<FileInfo>,
        timestamp: std::time::SystemTime,
    },
    /// Operation failed
    OperationFailed {
        error: String,
        error_code: u32,
        timestamp: std::time::SystemTime,
    },
}

/// File information for listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub permissions: Vec<Permission>,
    pub file_type: FileType,
}

/// File type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileType {
    /// Regular shared memory file
    SharedMemory,
    /// Directory
    Directory,
    /// Symbolic link
    SymbolicLink,
    /// Special device file
    Device,
}

/// Enhanced file status with more granular states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileStatus {
    /// File is active and available
    Active,
    /// File is inactive but persisted
    Inactive,
    /// File is being created
    Creating,
    /// File is being deleted
    Deleting,
    /// File is in maintenance mode
    Maintenance,
    /// File has encountered an error
    Error(String),
    /// File is being migrated to different storage
    Migrating,
}

/// Comprehensive response when a shared file operation is performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFileResponse {
    /// Unique identifier for this file allocation
    pub file_id: u64,
    /// Local path to the memory-mapped file
    pub file_path: PathBuf,
    /// Metadata about the allocated file
    pub metadata: FileMetadata,
    /// Information about the active transport
    pub transport: ActiveTransport,
    /// Performance characteristics of this connection
    pub performance_profile: PerformanceProfile,
    /// Security context for this connection
    pub security_context: SecurityContext,
    /// The specific operation result
    pub operation_result: SharedFileOperationResponse,
    /// Selected transport details
    pub selected_transport: SelectedTransport,
    /// Routing decision information
    pub routing_info: RoutingDecision,
}

/// Simple transport types for backwards compatibility
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransportType {
    /// Local shared memory
    SharedMemory,
    /// TCP network transport
    Network,
    /// WebSocket transport
    WebSocket,
    /// Named pipes (Windows/Unix)
    NamedPipe,
    /// Unix domain sockets
    UnixSocket,
}

/// Comprehensive metadata about an allocated shared file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Original request that created this file
    pub original_request: SharedFileRequest,
    /// When the file was created
    pub created_at: DateTime<Utc>,
    /// Last modified time
    pub last_modified: DateTime<Utc>,
    /// Last accessed time
    pub last_accessed: DateTime<Utc>,
    /// Current number of connected clients
    pub connection_count: u32,
    /// Maximum connections allowed
    pub max_connections: Option<u32>,
    /// Total size of the file in bytes
    pub size_bytes: u64,
    /// Current status of the file
    pub status: FileStatus,
    /// Performance statistics
    pub stats: FileStatistics,
    /// TTL expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// File version for conflict resolution
    pub version: u64,
    /// Checksum for integrity verification
    pub checksum: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// Comprehensive file statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileStatistics {
    /// Total number of read operations
    pub read_count: u64,
    /// Total number of write operations
    pub write_count: u64,
    /// Total bytes read
    pub bytes_read: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Average latency in microseconds
    pub avg_latency_us: f64,
    /// Peak concurrent connections
    pub peak_connections: u32,
    /// Last access time
    pub last_access: DateTime<Utc>,
    /// Error count
    pub error_count: u64,
    /// Cache hit ratio (0-1)
    pub cache_hit_ratio: f64,
    /// Compression ratio achieved
    pub compression_ratio: f64,
    /// Network bytes transferred
    pub network_bytes: u64,
    /// Local bytes transferred
    pub local_bytes: u64,
}

/// Enhanced information about the active transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActiveTransport {
    /// Local shared memory with detailed configuration
    SharedMemory {
        file_path: PathBuf,
        local_peers: Vec<u32>,
        memory_region: MemoryRegion,
        synchronization: SynchronizationMethod,
    },
    /// Network transport with connection details
    Network {
        endpoints: Vec<NetworkEndpoint>,
        protocol: NetworkProtocol,
        encryption: EncryptionDetails,
        compression: CompressionDetails,
    },
    /// Hybrid transport using both local and network
    Hybrid {
        primary: Box<ActiveTransport>,
        fallback: Box<ActiveTransport>,
        switching_criteria: SwitchingCriteria,
    },
}

/// Memory region details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRegion {
    /// Start address (for debugging)
    pub start_offset: u64,
    /// Size of the region
    pub size: u64,
    /// Memory page size
    pub page_size: usize,
    /// Whether huge pages are used
    pub uses_huge_pages: bool,
}

/// Synchronization method for shared memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SynchronizationMethod {
    /// No synchronization (fastest, unsafe)
    None,
    /// Mutex-based synchronization
    Mutex,
    /// Read-write lock
    RwLock,
    /// Atomic operations
    Atomic,
    /// Lock-free algorithms
    LockFree,
}

/// Network endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEndpoint {
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Protocol used
    pub protocol: String,
    /// Connection weight for load balancing
    pub weight: u8,
    /// Health status
    pub health: EndpointHealth,
}

/// Endpoint health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EndpointHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Network protocol details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkProtocol {
    Tcp,
    TcpTls,
    Udp,
    WebSocket,
    Http2,
    Quic,
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionDetails {
    /// Encryption algorithm
    pub algorithm: String,
    /// Key size in bits
    pub key_size: u16,
    /// Whether perfect forward secrecy is enabled
    pub perfect_forward_secrecy: bool,
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionDetails {
    /// Compression algorithm
    pub algorithm: String,
    /// Compression level (1-9)
    pub level: u8,
    /// Achieved compression ratio
    pub ratio: f64,
}

/// Criteria for transport switching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchingCriteria {
    /// Latency threshold for switching
    pub latency_threshold_ms: u32,
    /// Error rate threshold
    pub error_rate_threshold: f64,
    /// Bandwidth threshold
    pub bandwidth_threshold_mbps: u32,
}

/// Performance profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    /// Expected latency in microseconds
    pub expected_latency_us: u32,
    /// Expected throughput in MB/s
    pub expected_throughput_mbps: u32,
    /// High performance mode
    pub high_performance: bool,
    /// Performance tier
    pub tier: PerformanceTier,
}

/// Performance tiers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PerformanceTier {
    UltraLow,
    Low,
    Medium,
    Standard,
    High,
}

/// Security context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Auth token
    pub auth_token: String,
    /// Permissions granted
    pub permissions: Vec<Permission>,
    /// User identity
    pub identity: String,
    /// Session ID
    pub session_id: String,
    /// Security level
    pub security_level: SecurityLevel,
}

/// Security levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityLevel {
    None,
    Standard,
    High,
    Maximum,
}

/// Comprehensive manager events for monitoring and observability
#[derive(Debug, Clone)]
pub enum ManagerEvent {
    // File lifecycle events
    FileCreated {
        file_id: u64,
        identifier: String,
        file_path: PathBuf,
        size_bytes: u64,
        creation_policy: CreationPolicy,
        transport_type: TransportType,
    },
    FileConnected {
        file_id: u64,
        identifier: String,
        connection_count: usize,
        client_info: ClientInfo,
    },
    FileDisconnected {
        file_id: u64,
        identifier: String,
        remaining_connections: usize,
        disconnect_reason: DisconnectReason,
    },
    FileRemoved {
        file_id: u64,
        identifier: String,
        reason: String,
        cleanup_type: CleanupType,
    },
    FileExpired {
        file_id: u64,
        identifier: String,
        ttl_seconds: u64,
    },

    // Transport events
    TransportSelected {
        file_id: u64,
        transport_type: TransportType,
        selection_reason: String,
        performance_score: f64,
    },
    TransportSwitched {
        file_id: u64,
        from_transport: TransportType,
        to_transport: TransportType,
        switch_reason: String,
    },
    TransportFailed {
        file_id: u64,
        transport_type: TransportType,
        error_message: String,
        fallback_used: Option<TransportType>,
    },

    // Security events
    AuthenticationAttempt {
        client_info: ClientInfo,
        success: bool,
        method: String,
    },
    AuthorizationCheck {
        client_info: ClientInfo,
        resource: String,
        permissions: Vec<Permission>,
        granted: bool,
    },
    SecurityViolation {
        client_info: ClientInfo,
        violation_type: String,
        severity: SecuritySeverity,
    },

    // Performance events
    PerformanceAlert {
        metric_name: String,
        current_value: f64,
        threshold: f64,
        severity: AlertSeverity,
    },
    LatencyThresholdExceeded {
        file_id: u64,
        actual_latency_ms: f64,
        threshold_ms: f64,
    },

    // System events
    ClientConnected {
        client_info: ClientInfo,
        connection_time: DateTime<Utc>,
    },
    ClientDisconnected {
        client_id: String,
        session_id: Option<String>,
        disconnect_reason: String,
        disconnect_time: DateTime<Utc>,
        session_duration: std::time::Duration,
    },
    ManagerStarted {
        version: String,
        config_hash: String,
    },
    ManagerShutdown {
        reason: String,
        graceful: bool,
    },
    ConfigurationChanged {
        component: String,
        old_value: String,
        new_value: String,
    },
}

/// Client information for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_id: String,
    pub session_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub authentication_method: Option<String>,
    pub permissions: Vec<Permission>,
    pub connected_at: DateTime<Utc>,
}

/// Reason for client disconnection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisconnectReason {
    Normal,
    Timeout,
    Error(String),
    SecurityViolation,
    ResourceExhausted,
    Maintenance,
}

/// Type of cleanup performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupType {
    Automatic,
    Manual,
    Expired,
    Forced,
}

/// Security violation severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Shared file information for tracking
#[derive(Debug, Clone)]
pub struct SharedFileInfo {
    /// File metadata
    pub metadata: FileMetadata,
    /// Transport info
    pub transport: ActiveTransport,
    /// Performance profile
    pub performance_profile: PerformanceProfile,
}

/// Comprehensive error types for the manager
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum ManagerError {
    #[error("File already exists: {identifier}")]
    FileAlreadyExists { identifier: String },

    #[error("File not found: {identifier}")]
    FileNotFound { identifier: String },

    #[error("Invalid file identifier: {identifier} - {reason}")]
    InvalidIdentifier { identifier: String, reason: String },

    #[error("Permission denied for operation: {operation} on {resource}")]
    PermissionDenied { operation: String, resource: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Connection limit exceeded: {current}/{max}")]
    ConnectionLimitExceeded { current: usize, max: usize },

    #[error("Transport not available: {transport_type:?}")]
    TransportNotAvailable { transport_type: TransportType },

    #[error("Transport error: {transport_type:?} - {message}")]
    TransportError {
        transport_type: TransportType,
        message: String,
    },

    #[error("Configuration error: {component} - {message}")]
    ConfigurationError { component: String, message: String },

    #[error("Serialization error: {format:?} - {message}")]
    SerializationError {
        format: SerializationFormat,
        message: String,
    },

    #[error("Resource exhausted: {resource_type} - {details}")]
    ResourceExhausted {
        resource_type: String,
        details: String,
    },

    #[error("Operation timeout: {operation} after {timeout_ms}ms")]
    OperationTimeout { operation: String, timeout_ms: u64 },

    #[error("Invalid operation: {operation} for topology: {topology:?}")]
    InvalidOperation {
        operation: String,
        topology: Topology,
    },

    #[error("TTL expired for file: {identifier} after {ttl_seconds}s")]
    TtlExpired {
        identifier: String,
        ttl_seconds: u64,
    },

    #[error("IO error: {path} - {message}")]
    IoError { path: PathBuf, message: String },

    #[error("Memory mapping error: {path} - {message}")]
    MemoryMappingError { path: PathBuf, message: String },

    #[error("Network error: {endpoint} - {message}")]
    NetworkError { endpoint: String, message: String },

    #[error("Security violation: {violation_type} - {message}")]
    SecurityViolation {
        violation_type: String,
        message: String,
    },

    #[error("Allocation error: {resource} - requested {requested}, available {available}")]
    AllocationError {
        resource: String,
        requested: usize,
        available: usize,
    },

    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl From<Box<dyn std::error::Error + Send + Sync>> for ManagerError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        // Convert boxed error into the crate-local CommyError and delegate to the
        // centralized mapper with no path/format context.
        let com_err = crate::errors::CommyError::Other(err.to_string());
        map_commy_error_to_manager_error(com_err, None, None)
    }
}

/// Central helper to map a crate-level CommyError into a ManagerError while
/// accepting optional context such as the file path or serialization format.
pub fn map_commy_error_to_manager_error(
    com_err: crate::errors::CommyError,
    path: Option<std::path::PathBuf>,
    format: Option<SerializationFormat>,
) -> ManagerError {
    match com_err {
        crate::errors::CommyError::Io {
            source: e,
            path: err_path,
        } => ManagerError::IoError {
            path: path.or(err_path).unwrap_or_default(),
            message: e.to_string(),
        },
        crate::errors::CommyError::JsonSerialization(e) => ManagerError::SerializationError {
            format: format.unwrap_or(SerializationFormat::Json),
            message: format!("{}", e),
        },
        crate::errors::CommyError::BinarySerialization(s) => ManagerError::SerializationError {
            format: format.unwrap_or(SerializationFormat::Binary),
            message: s,
        },
        crate::errors::CommyError::MessagePackSerialization(s) => {
            ManagerError::SerializationError {
                format: format.unwrap_or(SerializationFormat::MessagePack),
                message: s,
            }
        }
        crate::errors::CommyError::CborSerialization(s) => ManagerError::SerializationError {
            format: format.unwrap_or(SerializationFormat::Cbor),
            message: s,
        },
        // No legacy Serialize variant exists anymore; any unhandled serialization
        // variants are already mapped explicitly above. Fall through to the
        // generic handler below which converts unknown variants to InternalError.
        crate::errors::CommyError::BufferTooSmall => ManagerError::ResourceExhausted {
            resource_type: "buffer".to_string(),
            details: "buffer too small".to_string(),
        },
        crate::errors::CommyError::PluginLoad(s) => ManagerError::ConfigurationError {
            component: "plugin".to_string(),
            message: s,
        },
        crate::errors::CommyError::InvalidArgument(s) => ManagerError::InternalError { message: s },
        crate::errors::CommyError::Other(s) => ManagerError::InternalError { message: s },
        // Any unhandled/richer variants map conservatively to an internal error
        // preserving the Display representation. This avoids an exhaustive
        // match update during the migration.
        other => ManagerError::InternalError {
            message: format!("{}", other),
        },
    }
}

/// Extended connection information returned to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub connection_id: String,
    pub connection_type: ConnectionType,
    pub endpoint: String,
    pub protocol_version: String,
    pub max_connections: usize,
    pub current_connections: usize,
    pub ttl_remaining: Option<u64>,
    pub permissions: Vec<Permission>,
}

/// Extended security information for connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityInfo {
    pub encryption_enabled: bool,
    pub authentication_method: Option<String>,
    pub required_permissions: Vec<Permission>,
    pub granted_permissions: Vec<Permission>,
    pub security_level: SecurityLevel,
    pub audit_id: Option<String>,
}

/// Extended performance metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub allocation_time_ms: f64,
    pub connection_time_ms: f64,
    pub throughput_mbps: Option<f64>,
    pub latency_percentiles: LatencyPercentiles,
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: f64,
}

/// Latency percentile measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyPercentiles {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub p99_9_ms: f64,
}

/// Connection type classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    Producer,
    Consumer,
    Bidirectional,
    Observer,
}

/// Extended response for file allocation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedSharedFileResponse {
    pub success: bool,
    pub file_id: Option<u64>,
    pub identifier: String,
    pub file_path: Option<PathBuf>,
    pub size_bytes: Option<u64>,
    pub allocated_transport: Option<TransportType>,
    pub connection_info: Option<ConnectionInfo>,
    pub security_info: Option<SecurityInfo>,
    pub performance_metrics: Option<PerformanceMetrics>,
    pub error: Option<ManagerError>,
    pub warnings: Vec<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

// Re-export the main manager
pub use core::SharedFileManager;

// Re-export transport types for easier access
pub use transport::{
    FallbackBehavior, NetworkConfig, PerformanceThresholds, RoutingDecision, RoutingReason,
    SelectedTransport, SharedMemoryConfig, SyncStrategy, TlsConfig, TlsVersion, TransportConfig,
    TransportManager,
};
pub use transport_impl::TransportError;
