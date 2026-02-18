// Chat system data models and helpers

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the chat system
#[derive(Clone, Debug)]
pub struct ChatConfig {
    pub commy_url: String,
    pub commy_tenant_prefix: String,
    pub max_message_length: usize,
    pub max_room_name_length: usize,
    pub presence_timeout_seconds: u64,
    pub max_message_history: usize,
}

impl Default for ChatConfig {
    fn default() -> Self {
        ChatConfig {
            commy_url: "wss://localhost:8443".to_string(),
            commy_tenant_prefix: "chat_".to_string(),
            max_message_length: 10000,
            max_room_name_length: 100,
            presence_timeout_seconds: 300,
            max_message_history: 10000,
        }
    }
}

/// Room information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomInfo {
    pub name: String,
    pub description: String,
    pub created_at: i64,
    pub member_count: usize,
    pub last_message_at: i64,
}

/// Statistics about a room
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomStats {
    pub total_messages: u64,
    pub active_users: usize,
    pub typing_users: usize,
    pub creation_time: i64,
    pub last_activity: i64,
}

/// Server configuration that tracks rooms and state
#[derive(Clone, Debug)]
pub struct ServerState {
    pub rooms: HashMap<String, RoomInfo>,
    pub connected_users: HashMap<String, String>, // user_id -> room_name
}

impl ServerState {
    pub fn new() -> Self {
        ServerState {
            rooms: HashMap::new(),
            connected_users: HashMap::new(),
        }
    }

    pub fn add_room(&mut self, name: String, description: String) {
        self.rooms.insert(
            name.clone(),
            RoomInfo {
                name,
                description,
                created_at: chrono::Local::now().timestamp(),
                member_count: 0,
                last_message_at: 0,
            },
        );
    }

    pub fn get_room(&self, name: &str) -> Option<&RoomInfo> {
        self.rooms.get(name)
    }

    pub fn add_user_to_room(&mut self, user_id: String, room_name: String) {
        self.connected_users.insert(user_id, room_name.clone());
        if let Some(room) = self.rooms.get_mut(&room_name) {
            room.member_count += 1;
        }
    }

    pub fn remove_user(&mut self, user_id: &str) {
        if let Some(room_name) = self.connected_users.remove(user_id) {
            if let Some(room) = self.rooms.get_mut(&room_name) {
                room.member_count = room.member_count.saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_state() {
        let mut state = ServerState::new();
        state.add_room("lobby".to_string(), "Main chat room".to_string());
        
        state.add_user_to_room("user1".to_string(), "lobby".to_string());
        
        let room = state.get_room("lobby").unwrap();
        assert_eq!(room.member_count, 1);
    }
}
