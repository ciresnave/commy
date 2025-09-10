//! Protocol definitions for Commy communications
//!
//! This module defines the message formats and protocol handlers used
//! for communication between nodes in the mesh.

use super::*;

/// Protocol handler for message processing
#[derive(Debug)]
pub struct ProtocolHandler;

impl ProtocolHandler {
    /// Create a new protocol handler
    pub fn new() -> Self {
        ProtocolHandler
    }

    /// Serialize a protocol message to bytes
    pub fn serialize_message(
        &self,
        message: &ProtocolMessage,
    ) -> Result<Vec<u8>, crate::errors::CommyError> {
        // For now, use JSON serialization - in production would use binary format
        // Map serde errors into CommyError and preserve format context by
        // mapping into a TransportError for downstream consumers when needed.
        serde_json::to_vec(message).map_err(crate::errors::CommyError::JsonSerialization)
    }

    /// Deserialize bytes to a protocol message
    pub fn deserialize_message(
        &self,
        data: &[u8],
    ) -> Result<ProtocolMessage, crate::errors::CommyError> {
        // For now, use JSON deserialization - in production would use binary format
        serde_json::from_slice(data).map_err(crate::errors::CommyError::JsonSerialization)
    }
}

impl Default for ProtocolHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Protocol message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    /// Unique message identifier
    pub message_id: String,
    /// Message type
    pub message_type: MessageType,
    /// Message payload
    pub payload: MessagePayload,
}

/// Message types for protocol communication
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageType {
    /// File operation message
    FileOperation,
    /// Response message
    Response,
    /// Error message
    Error,
    /// Heartbeat message
    Heartbeat,
}

/// Message payload variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// File request payload
    FileRequest { request: Box<SharedFileRequest> },
    /// File response payload
    FileResponse { file_id: u64, data: Vec<u8> },
    /// Error message payload
    Error {
        error_message: String,
        error_code: u32,
    },
    /// Heartbeat payload
    Heartbeat {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}
