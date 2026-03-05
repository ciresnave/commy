//! WSS Server Module
//!
//! Provides the WebSocket Secure (WSS) server implementation for COMMY.
//! Implements RFC 6455 (WebSocket Protocol) with RFC 5246 (TLS 1.2+) support.
//! Accepts remote client connections, manages protocol message routing,
//! maintains client sessions, and broadcasts variable changes.
//!
//! # Architecture
//!
//! ```text
//! Client → WSS Connection → WssServer (with TLS) → SessionManager → MessageRouter → Tenant/Service
//! ```
//!
//! - **WssServer**: Listens on a WSS port, accepts TLS connections, performs WebSocket handshake
//! - **TlsConfiguration**: Loads and manages TLS certificates
//! - **SessionManager**: Tracks active clients, manages permissions, broadcasts updates
//! - **MessageRouter**: Routes incoming WssMessages to appropriate handlers
//! - **WsHandler**: Per-connection task managing individual client communication

pub mod clustering;
pub mod message_router;
pub mod session_manager;
pub mod tls;
pub mod ws_handler;

use crate::Server;
use crate::protocol::{ClientSession, WssMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tls::TlsConfiguration;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_rustls::TlsAcceptor;

/// Configuration for the WSS server
#[derive(Clone, Debug)]
pub struct WssServerConfig {
    /// Address to bind to (e.g., "127.0.0.1")
    pub bind_addr: String,
    /// Port to listen on
    pub port: u16,
    /// Path to TLS certificate file for WSS (PEM format)
    pub cert_path: Option<String>,
    /// Path to TLS key file for WSS (PEM format)
    pub key_path: Option<String>,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Message buffer size per client
    pub buffer_size: usize,
}

impl Default for WssServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 8443,
            cert_path: None,
            key_path: None,
            max_connections: 1000,
            buffer_size: 65536,
        }
    }
}

impl WssServerConfig {
    /// Validate that TLS cert and key files are specified
    pub fn validate_tls(&self) -> Result<(), Box<dyn std::error::Error>> {
        match (&self.cert_path, &self.key_path) {
            (Some(cert), Some(key)) => {
                TlsConfiguration::from_files(cert, key)?;
                Ok(())
            }
            _ => Err("TLS certificate and key paths must be specified".into()),
        }
    }
}

/// WSS Server - Main entry point for remote client connections
///
/// Manages:
/// - TLS/WSS protocol handling (RFC 5246 + RFC 6455)
/// - Accepting WebSocket connections
/// - Maintaining client sessions
/// - Routing protocol messages
/// - Broadcasting variable changes
pub struct WssServer {
    config: WssServerConfig,
    server: Arc<RwLock<Server>>,
    sessions: Arc<RwLock<HashMap<String, ClientSession>>>,
    tls_acceptor: Option<TlsAcceptor>,
}

impl WssServer {
    /// Creates a new WSS server instance
    pub fn new(config: WssServerConfig, server: Arc<RwLock<Server>>) -> Self {
        Self {
            config,
            server,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            tls_acceptor: None,
        }
    }

    /// Initialize TLS from certificate and key files
    pub fn initialize_tls(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(cert_path), Some(key_path)) = (&self.config.cert_path, &self.config.key_path) {
            let tls_config = TlsConfiguration::from_files(cert_path, key_path)?;
            self.tls_acceptor = Some(TlsAcceptor::from(tls_config.config));
            println!("TLS initialized: cert={}, key={}", cert_path, key_path);
            Ok(())
        } else {
            Err("TLS certificate and key paths must be configured".into())
        }
    }

    /// Starts a background task to periodically clean up expired tokens
    ///
    /// NOTE: With auth-framework integration, token cleanup is handled internally
    /// by auth-framework's storage layer. This method is kept for API compatibility
    /// but doesn't perform any actions.
    ///
    /// # Arguments
    /// * `interval_seconds` - How often to run cleanup (ignored with auth-framework)
    pub fn start_token_cleanup_task(&self, interval_seconds: u64) {
        println!("Token cleanup task skipped - auth-framework handles cleanup internally (interval: {}s)", interval_seconds);
    }

    /// Starts the WSS server and listens for connections (RFC 5246 + RFC 6455)
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.bind_addr, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        println!(
            "WebSocket Secure (WSS) server listening on {} (RFC 6455 + TLS)",
            addr
        );

        let tls_acceptor = self
            .tls_acceptor
            .clone()
            .ok_or("TLS not initialized. Call initialize_tls() first")?;

        loop {
            let (socket, peer_addr) = listener.accept().await?;

            // Upgrade to TLS
            let tls_stream = match tls_acceptor.accept(socket).await {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("TLS handshake failed for {}: {}", peer_addr, e);
                    continue;
                }
            };

            println!("TLS handshake successful: {}", peer_addr);

            let server_clone = Arc::clone(&self.server);
            let sessions_clone = Arc::clone(&self.sessions);
            let config = self.config.clone();

            // Spawn a task for each client connection
            tokio::spawn(async move {
                if let Err(e) = ws_handler::handle_connection(
                    tls_stream,
                    peer_addr,
                    server_clone,
                    sessions_clone,
                    config,
                )
                .await
                {
                    eprintln!("Client handler error for {}: {}", peer_addr, e);
                }
            });
        }
    }

    /// Get active client sessions
    pub async fn active_sessions(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get a client session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<ClientSession> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Remove a client session
    pub async fn remove_session(&self, session_id: &str) {
        self.sessions.write().await.remove(session_id);
    }

    /// Update client session activity
    #[allow(dead_code)]
    pub async fn update_session_state(&self, session_id: &str) {
        if let Some(session) = self.sessions.write().await.get_mut(session_id) {
            session.last_activity = chrono::Utc::now();
        }
    }

    /// Broadcast a message to all connected clients of a tenant
    pub async fn broadcast_to_tenant(&self, tenant_id: &str, _message: WssMessage) {
        let sessions = self.sessions.read().await;
        for session in sessions.values() {
            if session
                .tenant_id
                .as_ref()
                .map_or(false, |id| id == tenant_id)
            {
                // In production, send message to client through their outbound queue
                println!(
                    "Broadcasting to client {} in tenant {}",
                    session.session_id, tenant_id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wss_config_default() {
        let config = WssServerConfig::default();
        assert_eq!(config.bind_addr, "127.0.0.1");
        assert_eq!(config.port, 8443);
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.buffer_size, 65536);
    }

    #[test]
    fn test_wss_config_custom() {
        let config = WssServerConfig {
            bind_addr: "0.0.0.0".to_string(),
            port: 9443,
            cert_path: Some("cert.pem".to_string()),
            key_path: Some("key.pem".to_string()),
            max_connections: 5000,
            buffer_size: 131072,
        };
        assert_eq!(config.port, 9443);
        assert_eq!(config.max_connections, 5000);
        assert!(config.cert_path.is_some());
    }

    #[tokio::test]
    async fn test_server_creation() {
        let config = WssServerConfig::default();
        let server = Arc::new(RwLock::new(Server::new()));
        let wss_server = WssServer::new(config, server);

        let sessions = wss_server.active_sessions().await;
        assert_eq!(sessions, 0);
    }

    // ─── WssServerConfig::validate_tls ────────────────────────────────────────

    #[test]
    fn test_validate_tls_no_paths_returns_err() {
        let config = WssServerConfig::default(); // cert_path and key_path are None
        let result = config.validate_tls();
        assert!(result.is_err(), "expected error when no TLS paths are set");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("TLS"), "error should mention TLS: {}", msg);
    }

    #[test]
    fn test_validate_tls_nonexistent_files_returns_err() {
        let config = WssServerConfig {
            cert_path: Some("/nonexistent/cert.pem".to_string()),
            key_path: Some("/nonexistent/key.pem".to_string()),
            ..WssServerConfig::default()
        };
        let result = config.validate_tls();
        assert!(result.is_err(), "expected error when files do not exist");
    }

    // ─── WssServer::initialize_tls ────────────────────────────────────────────

    #[test]
    fn test_initialize_tls_no_paths_returns_err() {
        let server = Arc::new(RwLock::new(Server::new()));
        let mut wss = WssServer::new(WssServerConfig::default(), server);
        let result = wss.initialize_tls();
        assert!(result.is_err(), "expected error when cert/key paths are not configured");
    }

    #[test]
    fn test_initialize_tls_nonexistent_files_returns_err() {
        let server = Arc::new(RwLock::new(Server::new()));
        let config = WssServerConfig {
            cert_path: Some("/nonexistent/cert.pem".to_string()),
            key_path: Some("/nonexistent/key.pem".to_string()),
            ..WssServerConfig::default()
        };
        let mut wss = WssServer::new(config, server);
        let result = wss.initialize_tls();
        assert!(result.is_err(), "expected error for nonexistent cert files");
    }

    // ─── WssServer::get_session / remove_session / update_session_state ───────

    #[tokio::test]
    async fn test_get_session_returns_none_for_unknown() {
        let server = Arc::new(RwLock::new(Server::new()));
        let wss = WssServer::new(WssServerConfig::default(), server);
        assert!(wss.get_session("unknown_id").await.is_none());
    }

    #[tokio::test]
    async fn test_get_and_remove_session() {
        let server = Arc::new(RwLock::new(Server::new()));
        let wss = WssServer::new(WssServerConfig::default(), server);

        // Insert a session directly (tests sit in the same module so private fields are accessible)
        let session = ClientSession::new();
        let id = session.session_id.clone();
        wss.sessions.write().await.insert(id.clone(), session);

        // get_session returns Some
        let found = wss.get_session(&id).await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().session_id, id);

        // remove_session drops it
        wss.remove_session(&id).await;
        assert!(wss.get_session(&id).await.is_none());
        assert_eq!(wss.active_sessions().await, 0);
    }

    #[tokio::test]
    async fn test_update_session_state_existing() {
        let server = Arc::new(RwLock::new(Server::new()));
        let wss = WssServer::new(WssServerConfig::default(), server);

        let session = ClientSession::new();
        let id = session.session_id.clone();
        wss.sessions.write().await.insert(id.clone(), session);

        // Should not panic and session should still exist
        wss.update_session_state(&id).await;
        assert!(wss.get_session(&id).await.is_some());
    }

    #[tokio::test]
    async fn test_update_session_state_unknown_is_noop() {
        let server = Arc::new(RwLock::new(Server::new()));
        let wss = WssServer::new(WssServerConfig::default(), server);
        // Should not panic for a session that does not exist
        wss.update_session_state("ghost_id").await;
        assert_eq!(wss.active_sessions().await, 0);
    }

    // ─── WssServer::broadcast_to_tenant ───────────────────────────────────────

    #[tokio::test]
    async fn test_broadcast_to_tenant_completes_without_panic() {
        use crate::protocol::ClientSession;
        let server = Arc::new(RwLock::new(Server::new()));
        let wss = WssServer::new(WssServerConfig::default(), server);

        // Insert two sessions: one matching tenant, one not
        let mut session_a = ClientSession::new();
        session_a.tenant_id = Some("tenant_a".to_string());
        let mut session_b = ClientSession::new();
        session_b.tenant_id = Some("tenant_b".to_string());

        {
            let mut s = wss.sessions.write().await;
            s.insert(session_a.session_id.clone(), session_a);
            s.insert(session_b.session_id.clone(), session_b);
        }

        let msg = WssMessage::Heartbeat { session_id: "test".to_string() };
        wss.broadcast_to_tenant("tenant_a", msg).await;
        // No assertion needed — just verify it completes without panic
        assert_eq!(wss.active_sessions().await, 2);
    }

    // ─── WssServer::start_token_cleanup_task ──────────────────────────────────

    #[test]
    fn test_start_token_cleanup_task_does_not_panic() {
        let server = Arc::new(RwLock::new(Server::new()));
        let wss = WssServer::new(WssServerConfig::default(), server);
        // Should simply print a message and return
        wss.start_token_cleanup_task(60);
        wss.start_token_cleanup_task(0);
    }
}
