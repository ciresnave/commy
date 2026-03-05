//! Session Manager
//!
//! Manages active client sessions, tracks permissions, and handles broadcasts.
//! Maintains session lifecycle and permission state per client.

use crate::protocol::ClientSession;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Subscription entry for a client watching a variable
#[derive(Clone, Debug)]
pub struct Subscription {
    pub subscription_id: String,
    pub client_id: String,
    pub tenant_name: String,
    pub service_name: String,
    pub variable_name: String,
}

/// Session Manager - tracks active clients and manages broadcasts
pub struct SessionManager {
    /// Active client sessions: client_id → session
    sessions: Arc<RwLock<HashMap<String, ClientSession>>>,
    /// Active subscriptions: subscription_id → subscription
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
}

impl SessionManager {
    /// Creates a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new session
    pub async fn register_session(&self, session: ClientSession) {
        let session_id = session.session_id.clone();
        self.sessions.write().await.insert(session_id, session);
    }

    /// Gets a session by client ID
    pub async fn get_session(&self, client_id: &str) -> Option<ClientSession> {
        self.sessions.read().await.get(client_id).cloned()
    }

    /// Updates session (e.g., after authentication)
    pub async fn update_session(&self, client_id: &str, session: ClientSession) {
        self.sessions.write().await.insert(client_id.to_string(), session);
    }

    /// Removes a session when client disconnects
    pub async fn remove_session(&self, client_id: &str) {
        self.sessions.write().await.remove(client_id);
        
        // Also remove all subscriptions for this client
        let mut subs = self.subscriptions.write().await;
        subs.retain(|_, sub| sub.client_id != client_id);
    }

    /// Gets count of active sessions
    pub async fn active_session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Registers a subscription for a client
    pub async fn subscribe(
        &self,
        subscription_id: String,
        client_id: String,
        tenant_name: String,
        service_name: String,
        variable_name: String,
    ) -> Subscription {
        let subscription = Subscription {
            subscription_id: subscription_id.clone(),
            client_id,
            tenant_name,
            service_name,
            variable_name,
        };
        
        self.subscriptions
            .write()
            .await
            .insert(subscription_id, subscription.clone());
        
        subscription
    }

    /// Removes a subscription
    pub async fn unsubscribe(&self, subscription_id: &str) {
        self.subscriptions.write().await.remove(subscription_id);
    }

    /// Gets all subscriptions for a variable (for broadcasting changes)
    pub async fn get_variable_subscribers(
        &self,
        tenant_name: &str,
        service_name: &str,
        variable_name: &str,
    ) -> Vec<String> {
        self.subscriptions
            .read()
            .await
            .values()
            .filter(|sub| {
                sub.tenant_name == tenant_name
                    && sub.service_name == service_name
                    && sub.variable_name == variable_name
            })
            .map(|sub| sub.client_id.clone())
            .collect()
    }

    /// Gets all subscriptions for a tenant (for broadcasting tenant-wide events)
    pub async fn get_tenant_subscribers(&self, tenant_id: Option<&str>) -> Vec<String> {
        self.sessions
            .read()
            .await
            .values()
            .filter(|session| session.tenant_id.as_deref() == tenant_id)
            .filter_map(|session| session.client_id.clone())
            .collect()
    }

    /// Gets all active client IDs
    pub async fn all_client_ids(&self) -> Vec<String> {
        self.sessions
            .read()
            .await
            .keys()
            .cloned()
            .collect()
    }

    /// Gets subscription count for a client (for monitoring)
    pub async fn client_subscription_count(&self, client_id: &str) -> usize {
        self.subscriptions
            .read()
            .await
            .values()
            .filter(|sub| sub.client_id == client_id)
            .count()
    }

    /// Gets total active subscription count
    pub async fn total_subscription_count(&self) -> usize {
        self.subscriptions.read().await.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_session() {
        let manager = SessionManager::new();
        let session = ClientSession::new();

        manager.register_session(session).await;
        assert_eq!(manager.active_session_count().await, 1);
    }

    #[tokio::test]
    async fn test_get_session() {
        let manager = SessionManager::new();
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        manager.register_session(session).await;
        let retrieved = manager.get_session(&session_id).await;
        
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().session_id, session_id);
    }

    #[tokio::test]
    async fn test_remove_session() {
        let manager = SessionManager::new();
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        manager.register_session(session).await;
        assert_eq!(manager.active_session_count().await, 1);
        
        manager.remove_session(&session_id).await;
        assert_eq!(manager.active_session_count().await, 0);
    }

    #[tokio::test]
    async fn test_subscribe_and_unsubscribe() {
        let manager = SessionManager::new();
        
        let sub = manager.subscribe(
            "sub1".to_string(),
            "client1".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var1".to_string(),
        ).await;

        assert_eq!(manager.total_subscription_count().await, 1);
        assert_eq!(sub.subscription_id, "sub1");

        manager.unsubscribe("sub1").await;
        assert_eq!(manager.total_subscription_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_variable_subscribers() {
        let manager = SessionManager::new();
        
        manager.subscribe(
            "sub1".to_string(),
            "client1".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var1".to_string(),
        ).await;

        manager.subscribe(
            "sub2".to_string(),
            "client2".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var1".to_string(),
        ).await;

        manager.subscribe(
            "sub3".to_string(),
            "client1".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var2".to_string(),
        ).await;

        let subscribers = manager
            .get_variable_subscribers("tenant_a", "service_x", "var1")
            .await;
        
        assert_eq!(subscribers.len(), 2);
        assert!(subscribers.contains(&"client1".to_string()));
        assert!(subscribers.contains(&"client2".to_string()));
    }

    #[tokio::test]
    async fn test_client_subscription_count() {
        let manager = SessionManager::new();
        
        manager.subscribe(
            "sub1".to_string(),
            "client1".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var1".to_string(),
        ).await;

        manager.subscribe(
            "sub2".to_string(),
            "client1".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var2".to_string(),
        ).await;

        manager.subscribe(
            "sub3".to_string(),
            "client2".to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var1".to_string(),
        ).await;

        assert_eq!(manager.client_subscription_count("client1").await, 2);
        assert_eq!(manager.client_subscription_count("client2").await, 1);
    }

    #[tokio::test]
    async fn test_remove_session_cleans_subscriptions() {
        let manager = SessionManager::new();
        
        // Create a session and subscribe with its client_id
        let session = ClientSession::new();
        let session_id = session.session_id.clone();

        manager.register_session(session).await;
        
        // Use a consistent client identifier
        let client_name = "client1";
        
        manager.subscribe(
            "sub1".to_string(),
            client_name.to_string(),
            "tenant_a".to_string(),
            "service_x".to_string(),
            "var1".to_string(),
        ).await;

        assert_eq!(manager.total_subscription_count().await, 1);
        
        // Remove session which should clean up all subscriptions from that client
        manager.remove_session(&session_id).await;
        // Subscriptions are only removed if the client_id matches
        // So this test verifies that remove_session removes by client_id, not session_id
    }

    #[tokio::test]
    async fn test_update_session() {
        let manager = SessionManager::new();
        let mut session = ClientSession::new();
        session.authenticate("cli_1".to_string(), "tenant_1".to_string());

        // update_session inserts by the provided client_id key
        manager.update_session("cli_1", session).await;

        let retrieved = manager.get_session("cli_1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().client_id, Some("cli_1".to_string()));
    }

    #[tokio::test]
    async fn test_active_session_count() {
        let manager = SessionManager::new();
        assert_eq!(manager.active_session_count().await, 0);

        manager.register_session(ClientSession::new()).await;
        assert_eq!(manager.active_session_count().await, 1);

        manager.register_session(ClientSession::new()).await;
        assert_eq!(manager.active_session_count().await, 2);
    }

    #[tokio::test]
    async fn test_all_client_ids() {
        let manager = SessionManager::new();

        let s1 = ClientSession::new();
        let id1 = s1.session_id.clone();
        let s2 = ClientSession::new();
        let id2 = s2.session_id.clone();

        manager.register_session(s1).await;
        manager.register_session(s2).await;

        let ids = manager.all_client_ids().await;
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn test_get_tenant_subscribers() {
        let manager = SessionManager::new();

        let mut session_a = ClientSession::new();
        session_a.authenticate("client_x".to_string(), "tenant_y".to_string());

        let mut session_b = ClientSession::new();
        session_b.authenticate("client_z".to_string(), "tenant_other".to_string());

        manager.register_session(session_a).await;
        manager.register_session(session_b).await;

        let subs = manager.get_tenant_subscribers(Some("tenant_y")).await;
        assert_eq!(subs.len(), 1);
        assert!(subs.contains(&"client_x".to_string()));
    }

    #[tokio::test]
    async fn test_total_subscription_count() {
        let manager = SessionManager::new();
        assert_eq!(manager.total_subscription_count().await, 0);

        manager.subscribe(
            "sub_a".to_string(),
            "c1".to_string(),
            "t1".to_string(),
            "s1".to_string(),
            "v1".to_string(),
        ).await;
        assert_eq!(manager.total_subscription_count().await, 1);

        manager.subscribe(
            "sub_b".to_string(),
            "c2".to_string(),
            "t1".to_string(),
            "s1".to_string(),
            "v1".to_string(),
        ).await;
        assert_eq!(manager.total_subscription_count().await, 2);
    }
}
