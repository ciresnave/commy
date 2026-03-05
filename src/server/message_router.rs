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
}
