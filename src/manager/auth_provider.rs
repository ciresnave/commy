use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

// This file contains the `AuthProvider` trait and test-friendly providers.

/// Trait for pluggable auth providers so tests can inject a mock.
#[async_trait]
pub trait AuthProvider: Send + Sync + 'static {
    async fn validate(&self, token: &str) -> Result<bool, String>;
}

/// Real provider that wraps the existing AuthFramework implementation
pub struct RealAuthProvider {
    inner: Arc<RwLock<auth_framework::AuthFramework>>,
    default_lifetime_secs: u64,
}

impl RealAuthProvider {
    /// Create a new RealAuthProvider.
    ///
    /// `default_lifetime_secs` is used when the incoming token does not include
    /// an explicit lifetime. This value should be taken from the AuthFramework
    /// configuration so constructed AuthToken instances match the framework's
    /// expectations.
    pub fn new(
        inner: Arc<RwLock<auth_framework::AuthFramework>>,
        default_lifetime_secs: u64,
    ) -> Self {
        Self {
            inner,
            default_lifetime_secs,
        }
    }
}

#[async_trait]
impl AuthProvider for RealAuthProvider {
    async fn validate(&self, token: &str) -> Result<bool, String> {
        let guard = self.inner.read().await;
        // Accept tokens of the form "Bearer <token>" or raw token strings.
        // Strip the optional bearer prefix so the framework receives the raw token.
        let token_trimmed = token.trim();
        let token_value = if token_trimmed.to_lowercase().starts_with("bearer ") {
            token_trimmed[7..].trim().to_string()
        } else {
            token_trimmed.to_string()
        };

        // Build AuthToken using the configured default lifetime (in seconds).
        let auth_token = auth_framework::tokens::AuthToken::new(
            "".to_string(),
            token_value,
            std::time::Duration::from_secs(self.default_lifetime_secs),
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
