//! Server-to-server communication protocol handler
//!
//! Manages TCP connections between servers and handles message serialization/deserialization.

use super::messages::{PeerMessageEnvelope, ServerMessage};
use super::connection::ConnectionPool;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use std::time::Duration;

/// Maximum message size (100 MB)
const MAX_MESSAGE_SIZE: usize = 100 * 1024 * 1024;

/// Message timeout (30 seconds)
const MESSAGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Protocol handler for inter-server communication
pub struct ProtocolHandler {
    /// Connection pool to peer servers
    connection_pool: Arc<ConnectionPool>,

    /// Server ID of this server
    server_id: String,

    /// Configuration for protocol behavior
    config: Arc<RwLock<ProtocolConfig>>,
}

/// Protocol configuration
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    /// Read timeout for TCP operations
    pub read_timeout: Duration,

    /// Write timeout for TCP operations
    pub write_timeout: Duration,

    /// Maximum message size in bytes
    pub max_message_size: usize,

    /// Enable message compression
    pub enable_compression: bool,

    /// Buffer size for TCP reads
    pub buffer_size: usize,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            read_timeout: MESSAGE_TIMEOUT,
            write_timeout: MESSAGE_TIMEOUT,
            max_message_size: MAX_MESSAGE_SIZE,
            enable_compression: false,
            buffer_size: 8192,
        }
    }
}

impl ProtocolHandler {
    /// Create a new protocol handler
    pub fn new(server_id: String, connection_pool: Arc<ConnectionPool>) -> Self {
        Self {
            connection_pool,
            server_id,
            config: Arc::new(RwLock::new(ProtocolConfig::default())),
        }
    }

    /// Send a message to a peer server
    pub async fn send_message(
        &self,
        target_server: &str,
        target_address: &str,
        message: ServerMessage,
    ) -> Result<String, String> {
        // Create message envelope
        let envelope = PeerMessageEnvelope::new(
            self.server_id.clone(),
            target_server.to_string(),
            message,
        );

        let message_id = envelope.message_id().to_string();

        // Serialize message
        let payload = serde_json::to_vec(&envelope).map_err(|e| {
            format!("Failed to serialize message: {}", e)
        })?;

        // Check size
        let config = self.config.read().await;
        if payload.len() > config.max_message_size {
            return Err("Message too large".to_string());
        }

        // Get or create connection
        let conn = self
            .connection_pool
            .get_connection(target_server, target_address)
            .await
            .map_err(|e| e)?;

        // Send with timeout
        let send_result = tokio::time::timeout(
            config.write_timeout,
            Self::send_to_peer(&conn.remote_address, &payload),
        )
        .await;

        match send_result {
            Ok(Ok(())) => {
                // Mark connection as active
                let _ = self
                    .connection_pool
                    .mark_active(target_server)
                    .await;
                Ok(message_id)
            }
            Ok(Err(e)) => {
                // Mark connection as inactive
                let _ = self
                    .connection_pool
                    .mark_inactive(target_server)
                    .await;
                Err(format!("Send failed: {}", e))
            }
            Err(_) => {
                // Timeout
                let _ = self
                    .connection_pool
                    .mark_inactive(target_server)
                    .await;
                Err("Send timeout".to_string())
            }
        }
    }

    /// Receive a message from a peer
    pub async fn receive_message(
        &self,
        stream: &mut TcpStream,
    ) -> Result<PeerMessageEnvelope, String> {
        let config = self.config.read().await;

        // Read message length (4 bytes)
        let mut len_buf = [0u8; 4];
        let read_result = tokio::time::timeout(
            config.read_timeout,
            stream.read_exact(&mut len_buf),
        )
        .await;

        let bytes_read = match read_result {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(format!("Read error: {}", e)),
            Err(_) => return Err("Read timeout".to_string()),
        };

        if bytes_read == 0 {
            return Err("Connection closed".to_string());
        }

        let message_len = u32::from_be_bytes(len_buf) as usize;

        // Validate size
        if message_len > config.max_message_size {
            return Err("Message size exceeds limit".to_string());
        }

        // Read message body
        let mut buffer = vec![0u8; message_len];
        let read_result = tokio::time::timeout(
            config.read_timeout,
            stream.read_exact(&mut buffer),
        )
        .await;

        match read_result {
            Ok(Ok(_)) => {
                // Deserialize
                serde_json::from_slice(&buffer).map_err(|e| {
                    format!("Deserialization error: {}", e)
                })
            }
            Ok(Err(e)) => Err(format!("Read error: {}", e)),
            Err(_) => Err("Read timeout".to_string()),
        }
    }

    /// Send bytes to a peer (internal helper)
    async fn send_to_peer(address: &str, payload: &[u8]) -> Result<(), String> {
        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;

        // Send message length
        let len = payload.len() as u32;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| format!("Write error: {}", e))?;

        // Send payload
        stream
            .write_all(payload)
            .await
            .map_err(|e| format!("Write error: {}", e))?;

        stream
            .flush()
            .await
            .map_err(|e| format!("Flush error: {}", e))?;

        Ok(())
    }

    /// Update protocol configuration
    pub async fn update_config(&self, config: ProtocolConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
    }

    /// Get current protocol configuration
    pub async fn get_config(&self) -> ProtocolConfig {
        self.config.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_config_default() {
        let config = ProtocolConfig::default();
        assert_eq!(config.read_timeout, MESSAGE_TIMEOUT);
        assert_eq!(config.write_timeout, MESSAGE_TIMEOUT);
        assert_eq!(config.max_message_size, MAX_MESSAGE_SIZE);
        assert!(!config.enable_compression);
    }

    #[tokio::test]
    async fn test_protocol_handler_creation() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);

        let config = handler.get_config().await;
        assert_eq!(config.read_timeout, MESSAGE_TIMEOUT);
    }

    #[tokio::test]
    async fn test_protocol_handler_config_update() {
        let pool = Arc::new(ConnectionPool::new());
        let handler = ProtocolHandler::new("server_1".to_string(), pool);

        let mut config = handler.get_config().await;
        config.read_timeout = Duration::from_secs(60);

        handler.update_config(config.clone()).await;

        let updated = handler.get_config().await;
        assert_eq!(updated.read_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_message_envelope_serialization() {
        let msg = ServerMessage::HeartbeatPing {
            server_id: "server_1".to_string(),
            timestamp: 1000000,
            sequence: 42,
        };

        let envelope =
            PeerMessageEnvelope::new("server_1".to_string(), "server_2".to_string(), msg);

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: PeerMessageEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.from_server, "server_1");
        assert_eq!(deserialized.to_server, "server_2");
    }

    #[test]
    fn test_protocol_message_types() {
        let messages = vec![
            ServerMessage::HeartbeatPing {
                server_id: "s1".to_string(),
                timestamp: 1000,
                sequence: 1,
            },
            ServerMessage::Error {
                request_id: None,
                error_code: "TEST_ERROR".to_string(),
                message: "Test".to_string(),
            },
        ];

        for msg in messages {
            let json = serde_json::to_string(&msg).unwrap();
            let _: ServerMessage = serde_json::from_str(&json).unwrap();
        }
    }
}
