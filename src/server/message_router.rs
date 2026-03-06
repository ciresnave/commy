//! Message Router
//!
//! Routes incoming WssMessages to appropriate handlers based on message type.
//! Coordinates between protocol layer, tenant/service layer, and session management.

use crate::protocol::WssMessage;

/// Routing decision for a message
#[derive(Debug, Clone)]
pub enum RoutingDecision {
    /// Route to authentication handler
    AuthenticationHandler,
    /// Route to service operation
    ServiceOperation(String),
    /// Route to subscription manager
    SubscriptionManager,
    /// Route to heartbeat/health handler
    HealthCheck,
    /// Message does not require routing
    Terminal,
}

/// Message Router - determines where each message should be dispatched
pub struct MessageRouter;

impl MessageRouter {
    /// Creates a new message router
    pub fn new() -> Self {
        Self
    }

    /// Routes a message and returns routing decision
    pub fn route(&self, message: &WssMessage) -> RoutingDecision {
        use WssMessage::*;

        match message {
            // Authentication messages
            Authenticate { .. } => RoutingDecision::AuthenticationHandler,

            // Service operations
            GetVariables { service_name, .. } => {
                RoutingDecision::ServiceOperation(format!("GetVariables({})", service_name))
            }
            SetVariables { service_name, .. } => {
                RoutingDecision::ServiceOperation(format!("SetVariables({})", service_name))
            }
            VariablesUpdated { .. } => RoutingDecision::Terminal, // Response only
            VariablesData { .. } => RoutingDecision::Terminal,    // Response only

            // Subscription messages
            Subscribe { .. } => RoutingDecision::SubscriptionManager,
            SubscriptionAck { .. } => RoutingDecision::Terminal, // Response only
            VariableChanged { .. } => RoutingDecision::Terminal, // Broadcast only

            // Health/heartbeat messages
            Heartbeat { .. } => RoutingDecision::HealthCheck,
            HeartbeatAck { .. } => RoutingDecision::Terminal, // Response only

            // Permission messages
            PermissionRevoked { .. } => RoutingDecision::Terminal, // Notification only
            CheckPermission { .. } => RoutingDecision::Terminal,
            PermissionResponse { .. } => RoutingDecision::Terminal, // Response only

            // File migration
            FileMigration { .. } => RoutingDecision::Terminal, // Notification
            MigrationAck { .. } => RoutingDecision::Terminal,  // Response only

            // Clustering
            TokenSync { .. } => RoutingDecision::Terminal, // Sync only
            ClusterPing { .. } => RoutingDecision::HealthCheck,
            ClusterPingResponse { .. } => RoutingDecision::Terminal, // Response only

            // Error/ack
            Error { .. } => RoutingDecision::Terminal, // Response only
            AuthenticationResponse { .. } => RoutingDecision::Terminal, // Response only
            Ack { .. } => RoutingDecision::Terminal,   // Response only

            // Logout/token lifecycle
            Logout { .. } => RoutingDecision::AuthenticationHandler,
            LogoutResponse { .. } => RoutingDecision::Terminal, // Response only
            RefreshToken { .. } => RoutingDecision::AuthenticationHandler,
            TokenRefreshResponse { .. } => RoutingDecision::Terminal, // Response only
        }
    }

    /// Get human-readable description of routing
    pub fn describe(&self, decision: &RoutingDecision) -> String {
        match decision {
            RoutingDecision::AuthenticationHandler => "Authentication handler".to_string(),
            RoutingDecision::ServiceOperation(desc) => format!("Service operation: {}", desc),
            RoutingDecision::SubscriptionManager => "Subscription manager".to_string(),
            RoutingDecision::HealthCheck => "Health check".to_string(),
            RoutingDecision::Terminal => "Terminal (no routing)".to_string(),
        }
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_authenticate() {
        let router = MessageRouter::new();
        let msg = WssMessage::Authenticate {
            tenant_id: "tenant_a".to_string(),
            client_id: "client1".to_string(),
            client_version: "0.1.0".to_string(),
            auth_method: "api_key".to_string(),
            credentials: "key123".to_string(),
        };

        let decision = router.route(&msg);
        match decision {
            RoutingDecision::AuthenticationHandler => (),
            _ => panic!("Expected AuthenticationHandler"),
        }
    }

    #[test]
    fn test_route_get_variables() {
        let router = MessageRouter::new();
        let msg = WssMessage::GetVariables {
            session_id: "sess1".to_string(),
            tenant_id: "tenant_a".to_string(),
            service_name: "service_x".to_string(),
            variable_names: vec!["var1".to_string()],
        };

        let decision = router.route(&msg);
        match decision {
            RoutingDecision::ServiceOperation(desc) => {
                assert!(desc.contains("GetVariables"));
                assert!(desc.contains("service_x"));
            }
            _ => panic!("Expected ServiceOperation"),
        }
    }

    #[test]
    fn test_route_heartbeat() {
        let router = MessageRouter::new();
        let msg = WssMessage::Heartbeat {
            session_id: "sess1".to_string(),
        };

        let decision = router.route(&msg);
        match decision {
            RoutingDecision::HealthCheck => (),
            _ => panic!("Expected HealthCheck"),
        }
    }

    #[test]
    fn test_routing_description() {
        let router = MessageRouter::new();
        let decision = RoutingDecision::ServiceOperation("SetVariables(config)".to_string());

        let desc = router.describe(&decision);
        assert!(desc.contains("Service operation"));
        assert!(desc.contains("config"));
    }

    #[test]
    fn test_route_subscribe() {
        let router = MessageRouter::new();
        let msg = WssMessage::Subscribe {
            session_id: "sess1".to_string(),
            tenant_id: "tenant_a".to_string(),
            service_name: "service_x".to_string(),
            variable_names: vec!["var1".to_string()],
        };

        let decision = router.route(&msg);
        match decision {
            RoutingDecision::SubscriptionManager => (),
            _ => panic!("Expected SubscriptionManager"),
        }
    }

    #[test]
    fn test_routing_description_all_variants() {
        let router = MessageRouter::new();

        let desc = router.describe(&RoutingDecision::AuthenticationHandler);
        assert!(desc.contains("Authentication handler"), "got: {}", desc);

        let desc = router.describe(&RoutingDecision::SubscriptionManager);
        assert!(desc.contains("Subscription manager"), "got: {}", desc);

        let desc = router.describe(&RoutingDecision::HealthCheck);
        assert!(desc.contains("Health check"), "got: {}", desc);

        let desc = router.describe(&RoutingDecision::Terminal);
        assert!(desc.contains("Terminal"), "got: {}", desc);

        let desc = router.describe(&RoutingDecision::ServiceOperation("WriteVariables(cfg)".to_string()));
        assert!(desc.contains("Service operation"), "got: {}", desc);
        assert!(desc.contains("cfg"), "got: {}", desc);
    }

    #[test]
    fn test_message_router_default() {
        let router = MessageRouter::default();
        // default() must produce the same routing behaviour as new()
        let msg = WssMessage::Heartbeat { session_id: "s1".to_string() };
        match router.route(&msg) {
            RoutingDecision::HealthCheck => (),
            _ => panic!("Expected HealthCheck from default router"),
        }

        let msg = WssMessage::Logout {
            session_id: "s1".to_string(),
            token: "tok".to_string(),
        };
        match router.route(&msg) {
            RoutingDecision::AuthenticationHandler => (),
            _ => panic!("Expected AuthenticationHandler for Logout"),
        }
    }

    #[test]
    fn test_route_set_variables() {
        let router = MessageRouter::new();
        let msg = WssMessage::SetVariables {
            session_id: "sess1".to_string(),
            tenant_id: "tenant_a".to_string(),
            service_name: "config_svc".to_string(),
            variables: std::collections::HashMap::new(),
        };
        let decision = router.route(&msg);
        match decision {
            RoutingDecision::ServiceOperation(desc) => {
                assert!(
                    desc.contains("SetVariables"),
                    "desc should contain SetVariables, got: {}",
                    desc
                );
                assert!(
                    desc.contains("config_svc"),
                    "desc should contain service name, got: {}",
                    desc
                );
            }
            _ => panic!("Expected ServiceOperation for SetVariables"),
        }
    }

    #[test]
    fn test_route_refresh_token() {
        let router = MessageRouter::new();
        let msg = WssMessage::RefreshToken {
            session_id: "sess1".to_string(),
            current_token: "tok123".to_string(),
        };
        let decision = router.route(&msg);
        match decision {
            RoutingDecision::AuthenticationHandler => (),
            _ => panic!("Expected AuthenticationHandler for RefreshToken"),
        }
    }

    #[test]
    fn test_route_cluster_ping() {
        let router = MessageRouter::new();
        let msg = WssMessage::ClusterPing {
            node_id: "node_1".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        let decision = router.route(&msg);
        match decision {
            RoutingDecision::HealthCheck => (),
            _ => panic!("Expected HealthCheck for ClusterPing"),
        }
    }

    #[test]
    fn test_route_response_only_messages_are_terminal() {
        let router = MessageRouter::new();
        let terminal_msgs: Vec<WssMessage> = vec![
            WssMessage::VariablesUpdated {
                success: true,
                message: "ok".to_string(),
                service_name: "svc".to_string(),
            },
            WssMessage::HeartbeatAck {
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            },
            WssMessage::Error {
                code: "ERR_001".to_string(),
                message: "some error".to_string(),
                details: None,
            },
            WssMessage::PermissionRevoked {
                reason: "revoked".to_string(),
                detail: "details here".to_string(),
            },
        ];
        for msg in &terminal_msgs {
            match router.route(msg) {
                RoutingDecision::Terminal => (),
                other => panic!("Message should route to Terminal, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_route_remaining_terminal_variants() {
        let router = MessageRouter::new();
        let terminal_msgs: Vec<WssMessage> = vec![
            WssMessage::VariablesData {
                tenant_id: "t".to_string(),
                service_name: "s".to_string(),
                variables: std::collections::HashMap::new(),
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            },
            WssMessage::SubscriptionAck {
                success: true,
                message: "ok".to_string(),
                service_name: "s".to_string(),
            },
            WssMessage::VariableChanged {
                tenant_id: "t".to_string(),
                service_name: "s".to_string(),
                variable_name: "v".to_string(),
                new_value: vec![],
                changed_by_client: None,
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            },
            WssMessage::FileMigration {
                old_service_path: "old".to_string(),
                new_service_path: "new".to_string(),
                service_name: "s".to_string(),
                reason: "revocation".to_string(),
            },
            WssMessage::MigrationAck {
                success: true,
                service_name: "s".to_string(),
            },
            WssMessage::CheckPermission {
                session_id: "sess1".to_string(),
                tenant_id: "t".to_string(),
                permission: "ServiceRead".to_string(),
            },
            WssMessage::PermissionResponse { has_permission: true },
            WssMessage::TokenSync {
                client_id: "c".to_string(),
                token: "tok".to_string(),
                tenant_id: "t".to_string(),
                permissions: "[]".to_string(),
                expires_at: "2026-01-01T00:00:00Z".to_string(),
            },
            WssMessage::ClusterPingResponse {
                node_id: "n1".to_string(),
                timestamp: "2025-01-01T00:00:00Z".to_string(),
            },
            WssMessage::Ack { message_id: "msg_1".to_string() },
            WssMessage::LogoutResponse {
                success: true,
                message: "logged out".to_string(),
            },
            WssMessage::TokenRefreshResponse {
                success: false,
                message: "expired".to_string(),
                token: None,
                expires_in_seconds: None,
            },
        ];
        for msg in &terminal_msgs {
            match router.route(msg) {
                RoutingDecision::Terminal => (),
                other => panic!(
                    "Expected Terminal for {:?}, got {:?}",
                    msg.message_type(),
                    other
                ),
            }
        }
    }
}
