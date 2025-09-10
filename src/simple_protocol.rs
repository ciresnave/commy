//! Simple Protocol Handler for Phase 1 Implementation
//!
//! This module provides a basic working protocol handler that demonstrates
//! the foundation layer capabilities without complex architectural issues.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simple message for basic file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleMessage {
    pub id: String,
    pub operation: String,
    pub data: Option<Vec<u8>>,
    pub metadata: HashMap<String, String>,
}

/// Simple response for file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleResponse {
    pub id: String,
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<u8>>,
}

/// Basic protocol handler for demonstrating the foundation
#[derive(Debug)]
pub struct SimpleProtocolHandler {
    /// In-memory storage for demonstration
    storage: HashMap<String, Vec<u8>>,
}

impl SimpleProtocolHandler {
    /// Create a new simple protocol handler
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    /// Handle a simple message and return a response
    pub fn handle_message(&mut self, message: SimpleMessage) -> SimpleResponse {
        match message.operation.as_str() {
            "create" => {
                if let Some(data) = message.data {
                    self.storage.insert(message.id.clone(), data);
                    SimpleResponse {
                        id: message.id,
                        success: true,
                        message: "File created successfully".to_string(),
                        data: None,
                    }
                } else {
                    SimpleResponse {
                        id: message.id,
                        success: false,
                        message: "No data provided for create operation".to_string(),
                        data: None,
                    }
                }
            }
            "read" => {
                if let Some(data) = self.storage.get(&message.id) {
                    SimpleResponse {
                        id: message.id,
                        success: true,
                        message: "File read successfully".to_string(),
                        data: Some(data.clone()),
                    }
                } else {
                    SimpleResponse {
                        id: message.id,
                        success: false,
                        message: "File not found".to_string(),
                        data: None,
                    }
                }
            }
            "delete" => {
                if self.storage.remove(&message.id).is_some() {
                    SimpleResponse {
                        id: message.id,
                        success: true,
                        message: "File deleted successfully".to_string(),
                        data: None,
                    }
                } else {
                    SimpleResponse {
                        id: message.id,
                        success: false,
                        message: "File not found".to_string(),
                        data: None,
                    }
                }
            }
            "list" => {
                let files: Vec<String> = self.storage.keys().cloned().collect();
                // Map serde_json errors into CommyError at the boundary, but
                // preserve the original behavior of returning default bytes on error.
                let files_json = serde_json::to_vec(&files)
                    .map_err(crate::errors::CommyError::JsonSerialization)
                    .unwrap_or_default();
                SimpleResponse {
                    id: message.id,
                    success: true,
                    message: format!("Found {} files", files.len()),
                    data: Some(files_json),
                }
            }
            _ => SimpleResponse {
                id: message.id,
                success: false,
                message: format!("Unknown operation: {}", message.operation),
                data: None,
            },
        }
    }

    /// Get storage statistics
    pub fn get_stats(&self) -> (usize, usize) {
        let total_files = self.storage.len();
        let total_size: usize = self.storage.values().map(|v| v.len()).sum();
        (total_files, total_size)
    }
}

impl Default for SimpleProtocolHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_protocol_handler() {
        let mut handler = SimpleProtocolHandler::new();

        // Test create operation
        let create_msg = SimpleMessage {
            id: "test_file".to_string(),
            operation: "create".to_string(),
            data: Some(b"Hello, World!".to_vec()),
            metadata: HashMap::new(),
        };

        let response = handler.handle_message(create_msg);
        assert!(response.success);
        assert_eq!(response.message, "File created successfully");

        // Test read operation
        let read_msg = SimpleMessage {
            id: "test_file".to_string(),
            operation: "read".to_string(),
            data: None,
            metadata: HashMap::new(),
        };

        let response = handler.handle_message(read_msg);
        assert!(response.success);
        assert_eq!(response.data, Some(b"Hello, World!".to_vec()));

        // Test stats
        let (file_count, total_size) = handler.get_stats();
        assert_eq!(file_count, 1);
        assert_eq!(total_size, 13); // "Hello, World!" is 13 bytes
    }
}
