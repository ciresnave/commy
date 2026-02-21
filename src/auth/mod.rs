//! Multi-tenant authentication and authorization module
//!
//! This module provides per-tenant authentication with isolated contexts.
//! Each tenant gets its own authentication state with independent permissions.
//!
//! Uses auth-framework for core authentication functionality.

pub mod permissions;
pub mod tenant_context;

use thiserror::Error;

// Re-export auth-framework types
pub use auth_framework::{
    AuthConfig, AuthError as FrameworkAuthError, AuthFramework, AuthResult as FrameworkAuthResult,
    AuthToken, Credential,
    methods::{ApiKeyMethod, AuthMethod, AuthMethodEnum, JwtMethod},
};

pub use permissions::{Permission, PermissionSet};
pub use tenant_context::{AuthenticationMode, StorageBackend, TenantAuthConfig, TenantAuthContext};

/// Errors that can occur in the authentication system
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Tenant not found: {0}")]
    TenantNotFound(String),

    #[error("Client not found: {0}")]
    ClientNotFound(String),

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Auth framework error: {0}")]
    FrameworkError(String),
}

impl From<FrameworkAuthError> for AuthError {
    fn from(err: FrameworkAuthError) -> Self {
        AuthError::FrameworkError(format!("{:?}", err))
    }
}

pub type AuthResult<T> = Result<T, AuthError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_auth_framework_integration() {
        // Set development environment to allow memory storage
        unsafe {
            std::env::set_var("ENVIRONMENT", "development");
        }

        // Create auth config with required JWT secret
        let config = AuthConfig::new()
            .token_lifetime(Duration::from_secs(3600))
            .secret("test-secret-at-least-32-characters-long".to_string());

        // Create auth framework
        let mut auth = AuthFramework::new(config);

        // Register JWT method
        let jwt_method = JwtMethod::new()
            .secret_key("test-secret-at-least-32-characters-long")
            .issuer("commy-test");

        auth.register_method("jwt", AuthMethodEnum::Jwt(jwt_method));
        auth.initialize().await.unwrap();

        // Create token
        let token = auth
            .create_auth_token(
                "test_user",
                vec!["read".to_string(), "write".to_string()],
                "jwt",
                None,
            )
            .await
            .unwrap();

        // Validate token
        assert!(auth.validate_token(&token).await.unwrap());
    }
}
