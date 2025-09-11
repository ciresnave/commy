use async_trait::async_trait;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;
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

    /// Try to extract the `exp` (expiration) claim in seconds from a JWT string.
    /// Returns Some(duration_seconds) when present and parsable, otherwise None.
    fn jwt_exp_seconds(token: &str) -> Option<u64> {
        // JWTs are in the form header.payload.signature (all base64url encoded)
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() < 2 {
            return None;
        }
        let payload_b64 = parts[1];

        // Try decoding the base64url payload. Use the base64 Engine API (URL_SAFE)
        // and fall back to the NO_PAD variant if needed.
        let decoded = match URL_SAFE.decode(payload_b64) {
            Ok(b) => b,
            Err(_) => match URL_SAFE_NO_PAD.decode(payload_b64) {
                Ok(b) => b,
                Err(_) => return None,
            },
        };

        let payload_str = match std::str::from_utf8(&decoded) {
            Ok(v) => v,
            Err(_) => return None,
        };

        // Parse the JSON payload using serde_json for robustness.
        match serde_json::from_str::<serde_json::Value>(payload_str) {
            Ok(val) => {
                if let Some(exp_val) = val.get("exp") {
                    // Accept number or string numbers
                    if exp_val.is_u64() {
                        return exp_val.as_u64();
                    }
                    if exp_val.is_number() {
                        // as_u64 may be None if it's float; try as_f64 then cast
                        if let Some(f) = exp_val.as_f64() {
                            if f >= 0.0 {
                                return Some(f as u64);
                            }
                        }
                    }
                    if exp_val.is_string() {
                        if let Some(s) = exp_val.as_str() {
                            if let Ok(n) = s.parse::<u64>() {
                                return Some(n);
                            }
                        }
                    }
                }
                None
            }
            Err(_) => None,
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

        // Determine lifetime. If token looks like a JWT and encodes an `exp` claim,
        // use that to compute a remaining lifetime. Otherwise, fall back to the
        // configured default lifetime.
        let lifetime_secs = if let Some(exp) = Self::jwt_exp_seconds(&token_value) {
            // compute remaining seconds from now (exp is unix epoch seconds)
            let now = chrono::Utc::now().timestamp() as u64;
            // Use saturating_sub to avoid manual checks and potential underflow
            exp.saturating_sub(now)
        } else {
            self.default_lifetime_secs
        };

        let auth_token = auth_framework::tokens::AuthToken::new(
            "".to_string(),
            token_value,
            std::time::Duration::from_secs(lifetime_secs),
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
