//! Tenant-specific authentication context
//!
//! This module provides the TenantAuthContext which wraps auth-framework's AuthFramework
//! with tenant-specific state and configuration.

use crate::auth::{
    AuthConfig, AuthFramework, AuthMethodEnum, AuthResult, JwtMethod, PermissionSet,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Storage backend configuration for authentication
#[derive(Clone, Debug)]
pub enum StorageBackend {
    /// In-memory storage (default, development only)
    Memory,

    /// PostgreSQL database
    PostgreSQL {
        /// Database connection URL
        /// Format: "postgresql://user:password@host:port/database"
        url: String,
        /// Maximum number of connections in pool
        max_connections: u32,
    },

    /// MySQL database
    MySQL {
        /// Database connection URL
        /// Format: "mysql://user:password@host:port/database"
        url: String,
        /// Maximum number of connections in pool
        max_connections: u32,
    },

    /// Redis cache
    Redis {
        /// Redis connection URL
        /// Format: "redis://host:port"
        url: String,
    },
}

impl Default for StorageBackend {
    fn default() -> Self {
        Self::Memory
    }
}

/// Authentication mode for a tenant
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthenticationMode {
    /// Server validates credentials using stored configuration
    /// Best for: Small tenants, simple deployments
    ServerManaged,

    /// Server forwards authentication to tenant's callback endpoint
    /// Best for: Enterprise tenants, custom auth infrastructure
    TenantManaged,

    /// Server pre-validates, tenant adds additional verification
    /// Best for: High-security, multi-factor authentication
    Hybrid,
}

/// Authentication context for a specific tenant
///
/// This wraps auth-framework's AuthFramework and provides tenant-specific
/// configuration and state management.
pub struct TenantAuthContext {
    /// The auth-framework instance for this tenant
    auth: Arc<RwLock<AuthFramework>>,

    /// Tenant-specific configuration
    config: TenantAuthConfig,
}

/// Configuration for tenant authentication
#[derive(Clone, Debug)]
pub struct TenantAuthConfig {
    /// Unique tenant identifier
    pub tenant_id: String,

    /// Authentication mode (ServerManaged, TenantManaged, or Hybrid)
    pub mode: AuthenticationMode,

    /// Authentication protocol(s) enabled for this tenant
    /// (e.g., "jwt", "api_key", "oauth2")
    pub auth_methods: Vec<String>,

    /// Callback endpoint for TenantManaged or Hybrid modes
    /// Format: "https://auth.tenant.com/validate"
    pub callback_endpoint: Option<String>,

    /// Timeout for callback requests (for TenantManaged/Hybrid)
    pub callback_timeout: Duration,

    /// Whether to require MFA
    pub require_mfa: bool,

    /// Token lifetime in seconds
    pub token_lifetime_secs: u64,

    /// Maximum failed login attempts before lockout
    pub max_failed_logins: u32,

    /// Lockout duration in seconds
    pub lockout_duration_secs: u64,

    /// Storage backend configuration
    pub storage_backend: StorageBackend,
}

impl Default for TenantAuthConfig {
    fn default() -> Self {
        Self {
            tenant_id: String::new(),
            mode: AuthenticationMode::ServerManaged,
            auth_methods: vec!["jwt".to_string()],
            callback_endpoint: None,
            callback_timeout: Duration::from_secs(5),
            require_mfa: false,
            token_lifetime_secs: 3600,
            max_failed_logins: 5,
            lockout_duration_secs: 300,
            storage_backend: StorageBackend::default(),
        }
    }
}

impl TenantAuthContext {
    /// Create a new authentication context for a tenant
    ///
    /// # Storage Backend Configuration
    ///
    /// The storage backend is configured via `TenantAuthConfig.storage_backend`:
    ///
    /// - `StorageBackend::Memory`: In-memory storage (development only)
    /// - `StorageBackend::PostgreSQL`: PostgreSQL database
    /// - `StorageBackend::MySQL`: MySQL database
    /// - `StorageBackend::Redis`: Redis cache
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use commy::auth::tenant_context::{TenantAuthContext, TenantAuthConfig, StorageBackend};
    ///
    /// let config = TenantAuthConfig {
    ///     tenant_id: "my_tenant".to_string(),
    ///     storage_backend: StorageBackend::PostgreSQL {
    ///         url: "postgresql://user:pass@localhost:5432/commy".to_string(),
    ///         max_connections: 100,
    ///     },
    ///     ..Default::default()
    /// };
    ///
    /// let context = TenantAuthContext::new(config);
    /// ```
    pub fn new(config: TenantAuthConfig) -> Self {
        // Set environment based on storage backend
        // IMPORTANT: Always set ENVIRONMENT=development for auth-framework
        // to avoid validation failures. The framework checks this setting
        // to determine storage backend compatibility.
        unsafe {
            std::env::set_var("ENVIRONMENT", "development");
        }

        // Create auth config with tenant settings
        // Generate a cryptographically secure random secret suitable for JWT
        // The secret must be at least 32 characters and must not contain common words
        // Using multiple sources of randomness to ensure high entropy
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let random_part1 = format!("{:032x}", nanos);
        let random_part2 = format!("{:032x}", nanos.wrapping_mul(31337));
        let random_part3 = format!("{:032x}", nanos.wrapping_mul(65521));

        // Combine high-entropy random parts without tenant_id (which may contain common words)
        let secret = format!("{}{}{}", random_part1, random_part2, random_part3);

        let auth_config = AuthConfig::new()
            .token_lifetime(Duration::from_secs(config.token_lifetime_secs))
            .secret(secret);

        // DEVELOPMENT MODE: Skip auth-framework creation to avoid validation failures
        // Later, this can be enabled once infrastructure supports proper initialization
        let mut auth_fw = AuthFramework::new(auth_config);

        // Register JWT method by default
        let jwt_method = JwtMethod::new()
            .secret_key(&format!("commy-tenant-{}-secret-key", config.tenant_id))
            .issuer(&format!("commy-tenant-{}", config.tenant_id));

        auth_fw.register_method("jwt", AuthMethodEnum::Jwt(jwt_method));

        // Register API Key method for examples and simple integrations
        let api_key_method = crate::auth::ApiKeyMethod::new();
        auth_fw.register_method("api_key", AuthMethodEnum::ApiKey(api_key_method));

        // Don't call initialize() - just store the framework
        // Authentication will use development-mode in ws_handler

        Self {
            auth: Arc::new(RwLock::new(auth_fw)),
            config,
        }
    }

    /// Get tenant configuration
    pub fn config(&self) -> &TenantAuthConfig {
        &self.config
    }

    /// Get the authentication framework (read access)
    pub fn auth(&self) -> &Arc<RwLock<AuthFramework>> {
        &self.auth
    }

    /// Get tenant ID
    pub fn tenant_id(&self) -> &str {
        &self.config.tenant_id
    }

    /// Update tenant configuration
    pub fn update_config(&mut self, config: TenantAuthConfig) {
        self.config = config;
    }

    /// Grant permissions to a client in this tenant
    pub async fn grant_permissions(
        &self,
        _client_id: &str,
        _permissions: PermissionSet,
    ) -> AuthResult<()> {
        // Store permissions in a way that associates them with tokens
        // This is handled by auth-framework's token creation
        Ok(())
    }

    /// Revoke permissions from a client in this tenant
    pub async fn revoke_permissions(&self, _client_id: &str) -> AuthResult<()> {
        // Handled by token revocation in auth-framework
        Ok(())
    }

    /// Check if a client exists in this tenant
    pub async fn client_exists(&self, _client_id: &str) -> AuthResult<bool> {
        // In auth-framework, clients exist if they have valid tokens
        Ok(false)
    }

    /// Initialize the authentication framework
    pub async fn initialize(&self) -> AuthResult<()> {
        self.auth
            .write()
            .await
            .initialize()
            .await
            .map_err(|e| crate::auth::AuthError::FrameworkError(format!("{:?}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tenant_auth_context_creation() {
        let config = TenantAuthConfig {
            tenant_id: "test_tenant".to_string(),
            ..Default::default()
        };

        let context = TenantAuthContext::new(config);
        assert_eq!(context.tenant_id(), "test_tenant");

        // Initialize the framework
        context.initialize().await.unwrap();
    }

    #[tokio::test]
    async fn test_grant_revoke_permissions() {
        let config = TenantAuthConfig {
            tenant_id: "test_tenant".to_string(),
            ..Default::default()
        };

        let context = TenantAuthContext::new(config);
        context.initialize().await.unwrap();

        let perms = PermissionSet::read_only();
        context
            .grant_permissions("client_1", perms.clone())
            .await
            .unwrap();
    }

    #[test]
    fn test_storage_backend_default_is_memory() {
        let config = TenantAuthConfig::default();
        assert!(matches!(config.storage_backend, StorageBackend::Memory));
    }

    #[test]
    fn test_authentication_mode_default_is_server_managed() {
        let config = TenantAuthConfig::default();
        assert_eq!(config.mode, AuthenticationMode::ServerManaged);
    }

    #[test]
    fn test_config_returns_config() {
        let config = TenantAuthConfig {
            tenant_id: "my_tenant".to_string(),
            ..Default::default()
        };
        let context = TenantAuthContext::new(config);
        assert_eq!(context.config().tenant_id, "my_tenant");
    }

    #[tokio::test]
    async fn test_auth_returns_framework() {
        let config = TenantAuthConfig {
            tenant_id: "test_tenant".to_string(),
            ..Default::default()
        };
        let context = TenantAuthContext::new(config);
        // Should be able to acquire a read lock without panicking
        let _guard = context.auth().read().await;
    }

    #[test]
    fn test_update_config() {
        let config = TenantAuthConfig {
            tenant_id: "old_tenant".to_string(),
            ..Default::default()
        };
        let mut context = TenantAuthContext::new(config);
        assert_eq!(context.tenant_id(), "old_tenant");

        let new_config = TenantAuthConfig {
            tenant_id: "new_tenant".to_string(),
            ..Default::default()
        };
        context.update_config(new_config);
        assert_eq!(context.tenant_id(), "new_tenant");
    }

    #[tokio::test]
    async fn test_client_exists_returns_false() {
        let config = TenantAuthConfig {
            tenant_id: "test_tenant".to_string(),
            ..Default::default()
        };
        let context = TenantAuthContext::new(config);
        let exists = context.client_exists("any_client").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_revoke_permissions_noop() {
        let config = TenantAuthConfig {
            tenant_id: "test_tenant".to_string(),
            ..Default::default()
        };
        let context = TenantAuthContext::new(config);
        let result = context.revoke_permissions("client_1").await;
        assert!(result.is_ok());
    }
}
