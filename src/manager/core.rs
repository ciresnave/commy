//! Core Shared File Manager Implementation
//!
//! This module provides the main SharedFileManager that orchestrates all
//! file operations, authentication, and configuration management with
//! comprehensive API support.

use super::memory_map::{MappedFile, MemoryMapManager};
use super::*;
use crate::manager::auth_provider::{AuthProvider, MockAuthProvider, RealAuthProvider};
use auth_framework::AuthFramework;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use distributed_config::ConfigManager;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::{broadcast, RwLock};
// Provide a TlsAcceptor alias so the type exists even when the "network"
// feature is disabled. When the feature is enabled, import the real type;
// otherwise create a dummy alias so the code can compile but will return a
// configuration error if TLS is actually requested at runtime.
#[cfg(feature = "network")]
use tokio_rustls::TlsAcceptor;
#[cfg(not(feature = "network"))]
type TlsAcceptor = ();

use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Minimal client connection type to satisfy references across manager code.
// Kept intentionally small; expand as needed by higher-level logic.
#[cfg(feature = "manager")]
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub id: Uuid,
    pub last_seen: DateTime<Utc>,
}

#[cfg(feature = "manager")]
pub struct SharedFileManager {
    pub auth: std::sync::Arc<dyn AuthProvider>,
    pub config: Arc<RwLock<ConfigManager>>,
    pub memory_map_manager: Arc<MemoryMapManager>,
    pub active_files: Arc<DashMap<u64, SharedFileInfo>>,
    pub next_file_id: AtomicU64,
    pub reusable_ids: Arc<RwLock<Vec<u64>>>,
    pub event_broadcaster: broadcast::Sender<ManagerEvent>,
    /// Manager configuration
    pub manager_config: ManagerConfig,

    /// Network listener for incoming connections
    pub listener: Option<TcpListener>,

    /// TLS acceptor for secure connections (only available with "network" feature)
    pub tls_acceptor: Option<TlsAcceptor>,

    /// Shutdown signal sender
    pub shutdown_tx: Option<mpsc::Sender<()>>,
}

#[cfg(feature = "manager")]
/// Internal shared file information
#[derive(Debug)]
pub struct SharedFileInfo {
    /// Unique file ID
    pub id: u64,

    /// File metadata
    pub metadata: FileMetadata,

    /// Actual path to the memory-mapped file
    pub file_path: PathBuf,

    /// The actual memory-mapped file
    pub mapped_file: Option<Arc<RwLock<MappedFile>>>,

    /// Connected clients
    pub clients: Vec<ClientConnection>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last access timestamp
    pub last_access: DateTime<Utc>,

    /// File lock for coordination
    pub lock: Arc<RwLock<()>>,
}

#[cfg(feature = "manager")]
/// Performance metrics for a client connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMetrics {
    /// Total messages sent by this client
    pub messages_sent: u64,

    /// Total messages received by this client
    pub messages_received: u64,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Average latency in microseconds
    pub avg_latency_us: f64,

    /// Connection uptime
    pub uptime_seconds: u64,
}

#[cfg(feature = "manager")]
/// Manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    /// Port to listen on for incoming connections
    pub listen_port: u16,

    /// Bind address
    pub bind_address: String,

    /// Maximum number of concurrent files
    pub max_files: u32,

    /// Maximum file size in bytes
    pub max_file_size: u64,

    /// Default TTL for files in seconds
    pub default_ttl_seconds: u64,

    /// Client heartbeat timeout in seconds
    pub heartbeat_timeout_seconds: u64,

    /// Cleanup interval in seconds
    pub cleanup_interval_seconds: u64,

    /// Database file path
    pub database_path: PathBuf,

    /// Shared files directory
    pub files_directory: PathBuf,

    /// TLS certificate path
    pub tls_cert_path: Option<PathBuf>,

    /// TLS private key path
    pub tls_key_path: Option<PathBuf>,

    /// Whether to require TLS for all connections
    pub require_tls: bool,

    /// Performance monitoring configuration
    pub performance_config: PerformanceConfig,

    /// Security configuration
    pub security_config: SecurityConfig,

    /// Enable mesh capabilities (service discovery, load balancing) - Phase 2
    pub enable_mesh_capabilities: bool,
}

#[cfg(feature = "manager")]
/// Performance monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable performance monitoring
    pub enabled: bool,

    /// Metrics collection interval in seconds
    pub collection_interval_seconds: u32,

    /// Performance history retention in days
    pub history_retention_days: u32,

    /// Enable detailed latency tracking
    pub detailed_latency_tracking: bool,
}

#[cfg(feature = "manager")]
/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Require authentication for all operations
    pub require_auth: bool,

    /// Maximum failed authentication attempts
    pub max_auth_failures: u32,

    /// Authentication failure lockout time in seconds
    pub auth_lockout_seconds: u64,

    /// Enable audit logging
    pub audit_logging: bool,

    /// Audit log file path
    pub audit_log_path: PathBuf,

    /// Enable threat detection
    pub threat_detection: bool,
}

#[cfg(feature = "manager")]
impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            listen_port: 8080,
            bind_address: "127.0.0.1".to_string(),
            max_files: 1000,
            max_file_size: 1024 * 1024 * 1024, // 1GB
            default_ttl_seconds: 3600,         // 1 hour
            heartbeat_timeout_seconds: 30,
            cleanup_interval_seconds: 60,
            database_path: PathBuf::from("commy_manager.db"),
            files_directory: PathBuf::from("./commy_files"),
            tls_cert_path: None,
            tls_key_path: None,
            require_tls: false,
            performance_config: PerformanceConfig::default(),
            security_config: SecurityConfig::default(),
            enable_mesh_capabilities: true, // Enable Phase 2 features by default
        }
    }
}

#[cfg(feature = "manager")]
impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_seconds: 10,
            history_retention_days: 30,
            detailed_latency_tracking: true,
        }
    }
}

#[cfg(feature = "manager")]
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            // Default to not requiring auth for local tests and examples so
            // that TestEnvironment::default() does not force external
            // AuthFramework initialization.
            require_auth: false,
            max_auth_failures: 5,
            auth_lockout_seconds: 300, // 5 minutes
            audit_logging: true,
            audit_log_path: PathBuf::from("commy_audit.log"),
            threat_detection: true,
        }
    }
}

#[cfg(feature = "manager")]
impl Default for ClientMetrics {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            avg_latency_us: 0.0,
            uptime_seconds: 0,
        }
    }
}

#[cfg(feature = "manager")]
impl SharedFileManager {
    /// Create a new shared file manager instance
    pub async fn new(config: ManagerConfig) -> Result<Self, ManagerError> {
        // Initialize tracing only if not already initialized
        let _ = tracing_subscriber::fmt::try_init();

        info!("Initializing Commy Shared File Manager with Phase 2 Memory Mapping");

        // Create files directory if it doesn't exist
        tokio::fs::create_dir_all(&config.files_directory)
            .await
            .map_err(|e| ManagerError::ConfigurationError {
                component: "Filesystem".to_string(),
                message: format!(
                    "Failed to create files directory {}: {}",
                    config.files_directory.display(),
                    e
                ),
            })?;

        // Initialize memory mapping manager
        let memory_map_manager = Arc::new(MemoryMapManager::new(&config.files_directory)?);
        info!(
            "Memory mapping manager initialized at {:?}",
            config.files_directory
        );

        // Initialize auth provider. If the manager configuration disables
        // authentication, use a MockAuthProvider that accepts tokens to keep
        // local tests and non-secure deployments simple. Otherwise initialize
        // the real AuthFramework and wrap it in a RealAuthProvider.
        let token_lifetime_secs: u64 = 3600; // 1 hour

        let auth_arc: std::sync::Arc<dyn AuthProvider> = if config.security_config.require_auth {
            let auth_config = auth_framework::AuthConfig::new()
                .token_lifetime(Duration::from_secs(token_lifetime_secs))
                .refresh_token_lifetime(Duration::from_secs(86400 * 7));

            let mut auth_fw = AuthFramework::new(auth_config);
            auth_fw
                .initialize()
                .await
                .map_err(|e| ManagerError::ConfigurationError {
                    component: "Auth".to_string(),
                    message: format!("Auth initialization failed: {}", e),
                })?;

            std::sync::Arc::new(RealAuthProvider::new(
                Arc::new(RwLock::new(auth_fw)),
                token_lifetime_secs,
            ))
        } else {
            // No-op mock provider that accepts any non-empty token
            std::sync::Arc::new(MockAuthProvider::new(true))
        };

        // Initialize distributed config
        let dist_config = ConfigManager::new();
        dist_config
            .initialize()
            .await
            .map_err(|e| ManagerError::ConfigurationError {
                component: "DistributedConfig".to_string(),
                message: format!("Distributed config initialization failed: {}", e),
            })?;

        // Create event broadcaster
        let (event_tx, _) = broadcast::channel(1000);

        // Note: Mesh coordinator will be initialized separately to avoid circular dependencies
        // This allows SharedFileManager to focus on core functionality while mesh capabilities
        // are orchestrated at a higher level

        // Wrap the real auth framework in the RealAuthProvider and expose it as
        // an `Arc<dyn AuthProvider>` so the manager depends on the abstraction.
        // Pass the configured token lifetime so the provider constructs tokens
        // that align with the framework's expectations. Create the Arc directly
        // and coerce to the trait object type.
        Ok(Self {
            auth: auth_arc,
            config: Arc::new(RwLock::new(dist_config)),
            memory_map_manager,
            active_files: Arc::new(DashMap::new()),
            next_file_id: AtomicU64::new(1),
            reusable_ids: Arc::new(RwLock::new(Vec::new())),
            event_broadcaster: event_tx,
            manager_config: config,
            listener: None,
            tls_acceptor: None,
            shutdown_tx: None,
        })
    }

    /// Create a manager with an injected auth provider (useful for tests)
    pub async fn new_with_provider(
        config: ManagerConfig,
        provider: std::sync::Arc<dyn AuthProvider>,
    ) -> Result<Self, ManagerError> {
        // Initialize tracing only if not already initialized
        let _ = tracing_subscriber::fmt::try_init();

        info!("Initializing Commy Shared File Manager (test) with Phase 2 Memory Mapping");

        // Create files directory if it doesn't exist
        tokio::fs::create_dir_all(&config.files_directory)
            .await
            .map_err(|e| ManagerError::ConfigurationError {
                component: "Filesystem".to_string(),
                message: format!(
                    "Failed to create files directory {}: {}",
                    config.files_directory.display(),
                    e
                ),
            })?;

        // Initialize memory mapping manager
        let memory_map_manager = Arc::new(MemoryMapManager::new(&config.files_directory)?);

        // Initialize distributed config
        let dist_config = ConfigManager::new();
        dist_config
            .initialize()
            .await
            .map_err(|e| ManagerError::ConfigurationError {
                component: "DistributedConfig".to_string(),
                message: format!("Distributed config initialization failed: {}", e),
            })?;

        // Create event broadcaster
        let (event_tx, _) = broadcast::channel(1000);

        Ok(Self {
            auth: provider,
            config: Arc::new(RwLock::new(dist_config)),
            memory_map_manager,
            active_files: Arc::new(DashMap::new()),
            next_file_id: AtomicU64::new(1),
            reusable_ids: Arc::new(RwLock::new(Vec::new())),
            event_broadcaster: event_tx,
            manager_config: config,
            listener: None,
            tls_acceptor: None,
            shutdown_tx: None,
        })
    }

    /// Initialize the SQLite database with required tables
    /// Start the shared file manager server
    pub async fn start(&mut self) -> Result<(), ManagerError> {
        info!(
            "Starting Commy Shared File Manager on {}:{}",
            self.manager_config.bind_address, self.manager_config.listen_port
        );

        // Bind to the configured address and port
        let addr = format!(
            "{}:{}",
            self.manager_config.bind_address, self.manager_config.listen_port
        );

        let listener =
            TcpListener::bind(&addr)
                .await
                .map_err(|e| ManagerError::ConfigurationError {
                    component: "Network".to_string(),
                    message: format!("Failed to bind listener to {}: {}", addr, e),
                })?;
        info!("Manager listening on {}", addr);

        // Initialize TLS if configured
        if let (Some(cert_path), Some(key_path)) = (
            &self.manager_config.tls_cert_path,
            &self.manager_config.tls_key_path,
        ) {
            self.tls_acceptor = Some(Self::create_tls_acceptor(cert_path, key_path).await?);
            info!("TLS enabled for secure connections");
        }

        self.listener = Some(listener);

        // Start background tasks
        self.start_background_tasks().await?;

        // Main server loop
        self.run_server_loop().await
    }

    /// Create TLS acceptor from certificate and key files
    #[cfg(feature = "network")]
    async fn create_tls_acceptor(
        cert_path: &PathBuf,
        key_path: &PathBuf,
    ) -> Result<TlsAcceptor, ManagerError> {
        use std::io::BufReader;
        use std::sync::Arc as StdArc;

        // Read certificate file
        let cert_bytes =
            tokio::fs::read(cert_path)
                .await
                .map_err(|e| ManagerError::ConfigurationError {
                    component: "TLS".to_string(),
                    message: format!("Failed to read cert file {}: {}", cert_path.display(), e),
                })?;

        // Parse PEM certificates into rustls-pki-types::CertificateDer and convert to owned ('static) DER
        let mut cert_reader = BufReader::new(&cert_bytes[..]);
        let certs_raw = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ManagerError::ConfigurationError {
                component: "TLS".to_string(),
                message: format!(
                    "Failed to parse certificate PEM {}: {}",
                    cert_path.display(),
                    e
                ),
            })?;

        // Convert parsed certificate items into owned CertificateDer<'static>
        let cert_chain: Vec<rustls::pki_types::CertificateDer<'static>> =
            certs_raw.into_iter().map(|c| c.into_owned()).collect();

        if cert_chain.is_empty() {
            return Err(ManagerError::ConfigurationError {
                component: "TLS".to_string(),
                message: format!("No certificates found in {}", cert_path.display()),
            });
        }

        // Read key file
        let key_bytes =
            tokio::fs::read(key_path)
                .await
                .map_err(|e| ManagerError::ConfigurationError {
                    component: "TLS".to_string(),
                    message: format!("Failed to read key file {}: {}", key_path.display(), e),
                })?;

        // Try parsing PKCS8 keys first, then RSA keys as fallback
        let mut key_reader = BufReader::new(&key_bytes[..]);
        let pkcs8_raw = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ManagerError::ConfigurationError {
                component: "TLS".to_string(),
                message: format!(
                    "Failed to parse PKCS8 private key {}: {}",
                    key_path.display(),
                    e
                ),
            })?;

        // Determine owned PrivateKeyDer<'static> from parsed key variants
        let key_der_static: rustls::pki_types::PrivateKeyDer<'static> = if !pkcs8_raw.is_empty() {
            // Convert the parsed PKCS8 key into the generic PrivateKeyDer and clone to 'static
            let first = pkcs8_raw.into_iter().next().unwrap();
            rustls::pki_types::PrivateKeyDer::from(first).clone_key()
        } else {
            // rewind reader and attempt rsa
            let mut key_reader = BufReader::new(&key_bytes[..]);
            let rsa_raw = rustls_pemfile::rsa_private_keys(&mut key_reader)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ManagerError::ConfigurationError {
                    component: "TLS".to_string(),
                    message: format!(
                        "Failed to parse RSA private key {}: {}",
                        key_path.display(),
                        e
                    ),
                })?;

            if rsa_raw.is_empty() {
                return Err(ManagerError::ConfigurationError {
                    component: "TLS".to_string(),
                    message: format!("No private keys found in {}", key_path.display()),
                });
            }

            let first = rsa_raw.into_iter().next().unwrap();
            rustls::pki_types::PrivateKeyDer::from(first).clone_key()
        };
        // Build server config using rustls builder API (ServerConfig::builder -> with_no_client_auth -> with_single_cert)
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key_der_static)
            .map_err(|e| ManagerError::ConfigurationError {
                component: "TLS".to_string(),
                message: format!("Failed to construct rustls ServerConfig: {}", e),
            })?;

        Ok(TlsAcceptor::from(StdArc::new(server_config)))
    }

    #[cfg(not(feature = "network"))]
    async fn create_tls_acceptor(
        _cert_path: &PathBuf,
        _key_path: &PathBuf,
    ) -> Result<TlsAcceptor, ManagerError> {
        Err(ManagerError::ConfigurationError {
            component: "TLS".to_string(),
            message: "TLS support is not enabled in this build (network feature missing)"
                .to_string(),
        })
    }

    /// Start background tasks for cleanup, monitoring, etc.
    async fn start_background_tasks(&mut self) -> Result<(), ManagerError> {
        let (shutdown_tx, _shutdown_rx) = broadcast::channel(10);

        // Store shutdown sender for later use
        let shutdown_sender = shutdown_tx.clone();

        // Cleanup task
        let cleanup_interval = Duration::from_secs(self.manager_config.cleanup_interval_seconds);
        let active_files_cleanup = Arc::clone(&self.active_files);
        let event_broadcaster_cleanup = self.event_broadcaster.clone();
        let mut shutdown_rx_cleanup = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::cleanup_expired_files(&active_files_cleanup, &event_broadcaster_cleanup).await;
                    }
                    _ = shutdown_rx_cleanup.recv() => {
                        info!("Cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        // Performance monitoring task
        if self.manager_config.performance_config.enabled {
            let monitoring_interval = Duration::from_secs(
                self.manager_config
                    .performance_config
                    .collection_interval_seconds as u64,
            );
            let active_files_monitor = Arc::clone(&self.active_files);
            let event_broadcaster_monitor = self.event_broadcaster.clone();
            let mut shutdown_rx_monitor = shutdown_tx.subscribe();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(monitoring_interval);
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            Self::collect_performance_metrics(&active_files_monitor, &event_broadcaster_monitor).await;
                        }
                        _ = shutdown_rx_monitor.recv() => {
                            info!("Performance monitoring task shutting down");
                            break;
                        }
                    }
                }
            });
        }

        // Convert shutdown_sender to mpsc::Sender for compatibility
        let (mpsc_tx, mut mpsc_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(mpsc_tx);

        // Bridge broadcast to mpsc
        tokio::spawn(async move {
            if let Some(()) = mpsc_rx.recv().await {
                let _ = shutdown_sender.send(());
            }
        });

        Ok(())
    }

    /// Main server loop to handle incoming connections
    async fn run_server_loop(&self) -> Result<(), ManagerError> {
        let listener = self.listener.as_ref().unwrap();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);

                    let auth = Arc::clone(&self.auth);
                    let config = Arc::clone(&self.config);
                    let active_files = Arc::clone(&self.active_files);
                    let event_broadcaster = self.event_broadcaster.clone();
                    let manager_config = self.manager_config.clone();
                    // `Option<TlsAcceptor>` is Copy when the network feature is
                    // disabled (TlsAcceptor = ()), so calling `clone()` triggers
                    // clippy::clone_on_copy. Copy the field instead which is
                    // a no-op for Copy types and avoids an extra clone call.
                    let tls_acceptor = self.tls_acceptor;

                    // Handle connection in a separate task
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(
                            stream,
                            addr.to_string(),
                            auth,
                            config,
                            active_files,
                            event_broadcaster,
                            manager_config,
                            tls_acceptor,
                        )
                        .await
                        {
                            error!("Error handling connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    /// Handle an individual client connection
    #[allow(clippy::too_many_arguments)]
    async fn handle_connection(
        _stream: TcpStream,
        addr: String,
        _auth: std::sync::Arc<dyn AuthProvider>,
        _config: Arc<RwLock<ConfigManager>>,
        _active_files: Arc<DashMap<u64, SharedFileInfo>>,
        _event_broadcaster: broadcast::Sender<ManagerEvent>,
        _manager_config: ManagerConfig,
        _tls_acceptor: Option<TlsAcceptor>,
    ) -> Result<(), ManagerError> {
        info!("Basic connection handling for {}", addr);

        // TODO: Implement proper protocol handling
        // For now, just log the connection and close it
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        info!("Connection from {} completed", addr);
        Ok(())
    }

    /// Clean up expired files and disconnected clients
    async fn cleanup_expired_files(
        active_files: &DashMap<u64, SharedFileInfo>,
        event_broadcaster: &broadcast::Sender<ManagerEvent>,
    ) {
        debug!("Running cleanup task");

        let mut files_to_remove = Vec::new();
        let cleanup_threshold = std::time::Duration::from_secs(300); // 5 minutes

        // Check each active file for cleanup criteria
        for entry in active_files.iter() {
            let file_id = *entry.key();
            let file_info = entry.value();

            // Check if file has been inactive for too long
            let elapsed = Utc::now().signed_duration_since(file_info.metadata.created_at);
            let should_cleanup = match file_info.metadata.status {
                FileStatus::Inactive => {
                    // Mark inactive files older than threshold for cleanup
                    elapsed.num_seconds() > cleanup_threshold.as_secs() as i64
                }
                FileStatus::Error(_) => {
                    // Clean up error files immediately
                    true
                }
                FileStatus::Active => {
                    // Keep active files but check connection count
                    file_info.metadata.connection_count == 0
                        && elapsed.num_seconds() > (cleanup_threshold.as_secs() * 2) as i64
                }
                FileStatus::Creating
                | FileStatus::Deleting
                | FileStatus::Maintenance
                | FileStatus::Migrating => {
                    // Don't cleanup files in transitional states
                    false
                }
            };

            if should_cleanup {
                files_to_remove.push(file_id);
                info!(
                    "Marking file {} for cleanup (status: {:?}, age: {}s)",
                    file_id,
                    file_info.metadata.status,
                    elapsed.num_seconds()
                );
            }
        }

        // Remove files that need cleanup
        for file_id in files_to_remove {
            if let Some((_, file_info)) = active_files.remove(&file_id) {
                info!(
                    "Cleaned up file {}: {}",
                    file_id, file_info.metadata.original_request.identifier
                );

                // Broadcast cleanup event
                let _ = event_broadcaster.send(ManagerEvent::FileRemoved {
                    file_id,
                    identifier: file_info.metadata.original_request.identifier,
                    reason: "Automatic cleanup due to inactivity".to_string(),
                    cleanup_type: CleanupType::Automatic,
                });
            }
        }
    }

    /// Collect performance metrics from active files
    async fn collect_performance_metrics(
        _active_files: &DashMap<u64, SharedFileInfo>,
        _event_broadcaster: &broadcast::Sender<ManagerEvent>,
    ) {
        // TODO: Implement performance metrics collection
        debug!("Collecting performance metrics");
    }

    /// Gracefully shutdown the manager
    pub async fn shutdown(&mut self) -> Result<(), ManagerError> {
        info!("Shutting down Commy Shared File Manager");

        // Send shutdown signal to background tasks
        if let Some(shutdown_tx) = &self.shutdown_tx {
            let _ = shutdown_tx.send(()).await;
        }

        // Broadcast shutdown event
        let _ = self.event_broadcaster.send(ManagerEvent::ManagerShutdown {
            reason: "Graceful shutdown".to_string(),
            graceful: true,
        });

        // Clean up all active files
        self.active_files.clear();

        info!("Commy Shared File Manager shutdown complete");
        Ok(())
    }

    /// Request a shared file allocation
    pub async fn request_file(
        &self,
        request: SharedFileRequest,
        auth_token: &str,
    ) -> Result<SharedFileResponse, ManagerError> {
        info!("Processing file request: {:?}", request.identifier);

        // 1. Authentication and authorization
        // Validate token using the configured AuthFramework. If the auth framework
        // provides a validation API, use it; otherwise fall back to the simple
        // non-empty check previously present. This keeps behavior stable while
        // enabling a straightforward upgrade path to full authentication.
        self.validate_auth_token(auth_token).await?;

        // 2. Validate request parameters
        if request.identifier.is_empty() {
            return Err(ManagerError::InvalidIdentifier {
                identifier: request.identifier.clone(),
                reason: "File identifier cannot be empty".to_string(),
            });
        }

        let max_size = request.max_size_bytes.unwrap_or(1024 * 1024);
        if max_size == 0 {
            return Err(ManagerError::InvalidOperation {
                operation: "create_file".to_string(),
                topology: request.topology,
            });
        }

        // 3. Check if file already exists based on policy
        let existing_file = self
            .active_files
            .iter()
            .find(|entry| entry.metadata.original_request.identifier == request.identifier);

        if let Some(existing) = existing_file {
            match request.existence_policy {
                ExistencePolicy::CreateOrConnect | ExistencePolicy::MustExist => {
                    // Connect to existing file
                    let file_id = *existing.key();

                    // Get the current values before updating
                    let metadata = existing.value().metadata.clone();
                    let actual_file_path = existing.value().file_path.clone();

                    // Drop the reference and get a mutable one
                    drop(existing);

                    // Update connection count
                    if let Some(mut entry) = self.active_files.get_mut(&file_id) {
                        entry.value_mut().metadata.connection_count += 1;
                    }

                    let connection_count = metadata.connection_count + 1;

                    // Create updated metadata with the incremented connection count
                    let updated_metadata = FileMetadata {
                        connection_count,
                        ..metadata
                    };

                    info!(
                        "Connected to existing file {} with ID {}",
                        request.identifier, file_id
                    );

                    // Broadcast connection event
                    let _ = self.event_broadcaster.send(ManagerEvent::FileConnected {
                        file_id,
                        identifier: request.identifier.clone(),
                        connection_count: connection_count as usize,
                        client_info: Self::create_default_client_info(),
                    });

                    return Ok(Self::create_enhanced_response(
                        file_id,
                        actual_file_path,
                        updated_metadata,
                        auth_token,
                        request.required_permissions,
                        request.max_size_bytes.unwrap_or(1024 * 1024), // Default 1MB
                    ));
                }
                ExistencePolicy::CreateOnly => {
                    return Err(ManagerError::FileAlreadyExists {
                        identifier: request.identifier.clone(),
                    });
                }
                ExistencePolicy::ConnectOnly => {
                    // Connect to existing file (same logic as CreateOrConnect)
                    let file_id = *existing.key();

                    // Get the current values before updating
                    let metadata = existing.value().metadata.clone();
                    let actual_file_path = existing.value().file_path.clone();

                    // Drop the reference and get a mutable one
                    drop(existing);

                    // Update connection count
                    if let Some(mut entry) = self.active_files.get_mut(&file_id) {
                        entry.value_mut().metadata.connection_count += 1;
                    }

                    let connection_count = metadata.connection_count + 1;

                    // Create updated metadata with new connection count
                    let mut updated_metadata = metadata.clone();
                    updated_metadata.connection_count = connection_count;

                    info!(
                        "Connected to existing file {} with ID {} (ConnectOnly)",
                        request.identifier, file_id
                    );

                    // Broadcast connection event
                    let _ = self.event_broadcaster.send(ManagerEvent::FileConnected {
                        file_id,
                        identifier: request.identifier.clone(),
                        connection_count: connection_count as usize,
                        client_info: Self::create_default_client_info(),
                    });

                    return Ok(Self::create_enhanced_response(
                        file_id,
                        actual_file_path,
                        updated_metadata,
                        auth_token,
                        request.required_permissions,
                        request.max_size_bytes.unwrap_or(1024 * 1024), // Default 1MB
                    ));
                }
            }
        } else if request.existence_policy == ExistencePolicy::ConnectOnly {
            return Err(ManagerError::FileNotFound {
                identifier: request.identifier.clone(),
            });
        }

        // 4. Allocate or connect to file
        let file_id = self.allocate_file_id().await;

        // 5. Create the actual memory-mapped file. If the requester provided a filename,
        // use create_file_with_name so the created file matches the requested path. Otherwise
        // fall back to the default naming.
        let mapped_file = if let Some(requested_path) = request.file_path.clone() {
            match self.memory_map_manager.create_file_with_name(
                file_id,
                requested_path.clone(),
                max_size,
            ) {
                Ok(mmap_file) => {
                    info!(
                        "Successfully created memory-mapped file for ID {} at {:?} with size {} bytes",
                        file_id, mmap_file.path, max_size
                    );
                    Some(Arc::new(RwLock::new(mmap_file)))
                }
                Err(e) => {
                    error!(
                        "Failed to create memory-mapped file for ID {} at requested path {:?}: {}",
                        file_id, requested_path, e
                    );
                    return Err(e);
                }
            }
        } else {
            match self.memory_map_manager.create_file(file_id, max_size) {
                Ok(mmap_file) => {
                    info!(
                        "Successfully created memory-mapped file for ID {} with size {} bytes",
                        file_id, max_size
                    );
                    Some(Arc::new(RwLock::new(mmap_file)))
                }
                Err(e) => {
                    error!(
                        "Failed to create memory-mapped file for ID {}: {}",
                        file_id, e
                    );
                    return Err(e);
                }
            }
        };

        // Determine final file_path from the created mapped file (preferred) or fall back to
        // a deterministic name based on file_id. This ensures response.file_path points to an
        // actual existing file.
        let final_file_path = if let Some(mf) = mapped_file.as_ref() {
            mf.read().await.path.clone()
        } else {
            PathBuf::from(format!("commy_file_{}.mmap", file_id))
        };

        // 6. Create file metadata and tracking structures
        let mut metadata = Self::create_enhanced_metadata(request.clone(), max_size);

        // The creating client is considered connected to the new file
        metadata.connection_count = 1;

        let _transport = Self::create_enhanced_transport(final_file_path.clone(), max_size);

        let _performance_profile = PerformanceProfile {
            expected_latency_us: 10,
            expected_throughput_mbps: 1000,
            high_performance: true,
            tier: PerformanceTier::UltraLow,
        };

        let shared_file_info = SharedFileInfo {
            id: file_id,
            metadata: metadata.clone(),
            file_path: final_file_path.clone(),
            mapped_file,
            clients: vec![],
            created_at: Utc::now(),
            last_access: Utc::now(),
            lock: Arc::new(RwLock::new(())),
        };

        // 6. Add to tracking structures
        self.active_files.insert(file_id, shared_file_info);

        info!(
            "Created new file '{}' with ID {} at {:?}",
            request.identifier, file_id, final_file_path
        );

        // Broadcast creation event
        let _ = self.event_broadcaster.send(Self::create_file_created_event(
            file_id,
            request.identifier.clone(),
            final_file_path.clone(),
            max_size,
            request.creation_policy,
        ));

        // 7. Return response
        Ok(Self::create_enhanced_response(
            file_id,
            final_file_path,
            metadata,
            auth_token,
            request.required_permissions,
            max_size,
        ))
    }

    /// Validate an auth token using the AuthFramework instance.
    /// Returns Ok(()) when the token is accepted, otherwise a ManagerError.
    async fn validate_auth_token(&self, token: &str) -> Result<(), ManagerError> {
        // Quick reject empty tokens
        if token.is_empty() {
            return Err(ManagerError::PermissionDenied {
                operation: "request_file".to_string(),
                resource: "<unknown>".to_string(),
            });
        }

        // Use injected provider to validate strictly. No fallback behavior
        // here: if the provider returns Err or Ok(false), we deny the request.
        match self.auth.validate(token).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(ManagerError::PermissionDenied {
                operation: "request_file".to_string(),
                resource: "<unknown>".to_string(),
            }),
            Err(e) => Err(ManagerError::AuthenticationFailed {
                reason: format!("Auth validation failed: {}", e),
            }),
        }
    }

    /// Handle client disconnection from a file
    pub async fn disconnect_file(&self, file_id: u64) -> Result<(), ManagerError> {
        if let Some(mut entry) = self.active_files.get_mut(&file_id) {
            let file_info = entry.value_mut();

            if file_info.metadata.connection_count > 0 {
                file_info.metadata.connection_count -= 1;

                info!(
                    "Client disconnected from file {}, remaining connections: {}",
                    file_id, file_info.metadata.connection_count
                );

                // If no more connections, mark as inactive and clean up
                if file_info.metadata.connection_count == 0 {
                    file_info.metadata.status = FileStatus::Inactive;

                    // Delete the memory-mapped file. Prefer deleting the actual path we
                    // stored in SharedFileInfo to support user-provided filenames. If that
                    // fails, fall back to the manager's delete_file by file_id.
                    let path_to_delete = file_info.file_path.clone();
                    match std::fs::remove_file(&path_to_delete) {
                        Ok(_) => info!("Deleted memory-mapped file: {:?}", path_to_delete),
                        Err(e) => {
                            warn!(
                                "Failed to delete memory-mapped file at {:?}: {}. Falling back to delete_file(file_id)",
                                path_to_delete, e
                            );
                            if let Err(e) = self.memory_map_manager.delete_file(file_id) {
                                warn!("Fallback delete_file failed: {}", e);
                            }
                        }
                    }

                    // Broadcast disconnection event
                    let _ = self
                        .event_broadcaster
                        .send(Self::create_file_disconnected_event(
                            file_id,
                            file_info.metadata.original_request.identifier.clone(),
                            0,
                        ));
                }

                Ok(())
            } else {
                Err(ManagerError::InternalError {
                    message: format!("File {} has no active connections", file_id),
                })
            }
        } else {
            Err(ManagerError::FileNotFound {
                identifier: file_id.to_string(),
            })
        }
    }

    /// Allocate a new file ID, reusing IDs from deleted files if available
    async fn allocate_file_id(&self) -> u64 {
        // Try to reuse an ID first
        if let Some(id) = self.reusable_ids.write().await.pop() {
            return id;
        }

        // Otherwise, get the next sequential ID
        self.next_file_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Get a list of all active files
    pub async fn list_active_files(&self) -> Vec<(u64, String, FileStatus, usize)> {
        // TODO: Fix DashMap API usage
        vec![]
    }

    /// Subscribe to manager events
    pub fn subscribe_events(&self) -> broadcast::Receiver<ManagerEvent> {
        self.event_broadcaster.subscribe()
    }
}

// Helper functions for creating default values
impl SharedFileManager {
    /// Create default memory region for shared memory transport
    fn create_default_memory_region(size: u64) -> MemoryRegion {
        MemoryRegion {
            start_offset: 0,
            size,
            page_size: 4096,
            uses_huge_pages: false,
        }
    }

    /// Create default synchronization method
    fn create_default_synchronization() -> SynchronizationMethod {
        SynchronizationMethod::Mutex
    }

    /// Create default operation result for successful connection
    fn create_default_operation_result() -> SharedFileOperationResponse {
        SharedFileOperationResponse::InfoSuccess {
            file_size: 0,
            created_at: Some(std::time::SystemTime::now()),
            modified_at: Some(std::time::SystemTime::now()),
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Create default routing info for API responses
    fn create_default_routing_info() -> RoutingDecision {
        RoutingDecision {
            transport: SelectedTransport::SharedMemory,
            reason: RoutingReason::Default,
            confidence: 0.95,
            expected_performance: PerformanceProfile {
                expected_latency_us: 10,
                expected_throughput_mbps: 1000,
                high_performance: true,
                tier: PerformanceTier::High,
            },
        }
    }

    /// Create default selected transport for API responses
    fn create_default_selected_transport() -> SelectedTransport {
        SelectedTransport::SharedMemory
    }

    /// Create default client info for events
    fn create_default_client_info() -> ClientInfo {
        ClientInfo {
            client_id: Uuid::new_v4().to_string(),
            session_id: Uuid::new_v4().to_string(),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: None,
            authentication_method: Some("internal".to_string()),
            permissions: vec![Permission::Read, Permission::Write],
            connected_at: Utc::now(),
        }
    }

    /// Create enhanced file metadata with defaults for new fields
    fn create_enhanced_metadata(
        original_request: SharedFileRequest,
        size_bytes: u64,
    ) -> FileMetadata {
        let now = Utc::now();
        FileMetadata {
            original_request,
            created_at: now,
            last_modified: now,
            last_accessed: now,
            connection_count: 0,
            max_connections: Some(100), // Default limit
            size_bytes,
            status: FileStatus::Active,
            stats: FileStatistics {
                last_access: now,
                ..Default::default()
            },
            expires_at: None, // No expiration by default
            version: 1,
            checksum: None, // Calculate later if needed
            tags: vec![],
        }
    }

    /// Create enhanced active transport with defaults
    fn create_enhanced_transport(file_path: std::path::PathBuf, size: u64) -> ActiveTransport {
        ActiveTransport::SharedMemory {
            file_path,
            local_peers: vec![],
            memory_region: Self::create_default_memory_region(size),
            synchronization: Self::create_default_synchronization(),
        }
    }

    /// Create enhanced shared file response with all required fields
    fn create_enhanced_response(
        file_id: u64,
        file_path: std::path::PathBuf,
        metadata: FileMetadata,
        auth_token: &str,
        permissions: Vec<Permission>,
        size: u64,
    ) -> SharedFileResponse {
        SharedFileResponse {
            file_id,
            file_path: file_path.clone(),
            metadata,
            transport: Self::create_enhanced_transport(file_path, size),
            performance_profile: PerformanceProfile {
                expected_latency_us: 10,
                expected_throughput_mbps: 1000,
                high_performance: true,
                tier: PerformanceTier::UltraLow,
            },
            security_context: SecurityContext {
                auth_token: auth_token.to_string(),
                permissions,
                identity: "authenticated_user".to_string(),
                session_id: Uuid::new_v4().to_string(),
                security_level: SecurityLevel::Standard,
            },
            operation_result: Self::create_default_operation_result(),
            selected_transport: Self::create_default_selected_transport(),
            routing_info: Self::create_default_routing_info(),
        }
    }

    /// Create file created event with required fields
    fn create_file_created_event(
        file_id: u64,
        identifier: String,
        file_path: std::path::PathBuf,
        size_bytes: u64,
        creation_policy: CreationPolicy,
    ) -> ManagerEvent {
        ManagerEvent::FileCreated {
            file_id,
            identifier,
            file_path,
            size_bytes,
            creation_policy,
            transport_type: TransportType::SharedMemory,
        }
    }

    /// Create file disconnected event with reason
    fn create_file_disconnected_event(
        file_id: u64,
        identifier: String,
        remaining_connections: usize,
    ) -> ManagerEvent {
        ManagerEvent::FileDisconnected {
            file_id,
            identifier,
            remaining_connections,
            disconnect_reason: DisconnectReason::Normal,
        }
    }
}
