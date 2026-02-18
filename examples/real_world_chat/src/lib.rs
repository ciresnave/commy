// Commy Chat System - Shared library
//
// This module defines the core types and protocols used throughout
// the chat system for serialization and communication.

use serde::{Deserialize, Serialize};

/// A message sent by a user in a chat room
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub room: String,
    pub user: String,
    pub text: String,
    pub timestamp: u128,
}

/// Presence information for a user in a room
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: String,
    pub username: String,
    pub room: String,
    pub joined_at: u128,
    pub last_active: u128,
    pub status: String,
}

/// Indicates when a user is typing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypingIndicator {
    pub room: String,
    pub user: String,
    pub is_typing: bool,
    pub timestamp: u128,
}

/// Response from an operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl Response {
    pub fn ok(message: impl Into<String>) -> Self {
        Response {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Response {
            success: false,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            id: "uuid-1".to_string(),
            room: "lobby".to_string(),
            user: "alice".to_string(),
            text: "Hello!".to_string(),
            timestamp: 1000,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.text, deserialized.text);
    }

    #[test]
    fn test_response_creation() {
        let resp = Response::ok("Success").with_data(serde_json::json!({"status": "ok"}));
        assert!(resp.success);
        assert!(resp.data.is_some());
    }
}
