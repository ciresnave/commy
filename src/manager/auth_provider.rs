use async_trait::async_trait;
use chrono::Duration;
use std::sync::Arc;
use tokio::sync::RwLock;

// Focused change: small marker comment to create an isolated auth-provider commit
// This file contains the `AuthProvider` trait and test-friendly providers.
use crate::manager::ManagerError;

/// Trait for pluggable auth providers so tests can inject a mock.
#[async_trait]
pub trait AuthProvider: Send + Sync + 'static {
    async fn validate(&self, token: &str) -> Result<bool, String>;
}

/// Real provider that wraps the existing AuthFramework implementation
pub struct RealAuthProvider {
    inner: Arc<RwLock<auth_framework::AuthFramework>>,
}

impl RealAuthProvider {
    pub fn new(inner: Arc<RwLock<auth_framework::AuthFramework>>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl AuthProvider for RealAuthProvider {
    async fn validate(&self, token: &str) -> Result<bool, String> {
        let guard = self.inner.read().await;
        // Build AuthToken using documented constructor
        let auth_token = auth_framework::tokens::AuthToken::new(
            "".to_string(),
            token.to_string(),
            std::time::Duration::from_secs(3600),
            "bearer".to_string(),
        );
        match guard.validate_token(&auth_token).await {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("auth framework error: {}", e)),
        }
    }
}

/// Mock provider used in tests to deterministically accept/reject tokens.
pub struct MockAuthProvider {
    pub accept_tokens: bool,
}

impl MockAuthProvider {
    pub fn new(accept_tokens: bool) -> Self {
        Self { accept_tokens }
    }
}

#[async_trait]
impl AuthProvider for MockAuthProvider {
    async fn validate(&self, _token: &str) -> Result<bool, String> {
        Ok(self.accept_tokens)
    }
}
