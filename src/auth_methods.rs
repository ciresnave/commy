//! Authentication method registration and management
//! 
//! Allows per-tenant configuration of authentication protocols:
//! - API Key authentication
//! - JWT (JSON Web Token)
//! - mTLS (mutual TLS)
//! - Custom authentication methods
//!
//! ## Hybrid Serialization Strategy
//! While the outer protocol messages use MessagePack for efficiency,
//! custom authentication configurations remain JSON (serde_json::Value).
//! This allows tenants to define custom authentication methods without
//! requiring schema coordination across the cluster.

use crate::auth::{AuthError, AuthResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Authentication method type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMethodType {
    /// API Key authentication (bearer token)
    ApiKey,
    /// JWT (JSON Web Token) authentication
    Jwt,
    /// mTLS (mutual TLS certificate) authentication
    Mtls,
    /// Custom authentication method
    Custom(String),
}

impl AuthMethodType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::ApiKey => "api_key",
            Self::Jwt => "jwt",
            Self::Mtls => "mtls",
            Self::Custom(name) => name,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "api_key" => Self::ApiKey,
            "jwt" => Self::Jwt,
            "mtls" => Self::Mtls,
            custom => Self::Custom(custom.to_string()),
        }
    }
}

/// Configuration for API Key authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    /// Header name (e.g., "X-API-Key")
    pub header_name: String,
    /// API key value (in production, would be hashed)
    pub api_key: String,
}

/// Configuration for JWT authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JwtConfig {
    /// Signing secret (in production, would be a key)
    pub signing_secret: String,
    /// Algorithm (HS256, RS256, etc.)
    pub algorithm: String,
    /// Token lifetime in seconds
    pub token_lifetime_secs: u64,
    /// Issuer claim
    pub issuer: String,
    /// Audience claim
    pub audience: String,
}

/// Configuration for mTLS authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// Path to CA certificate file
    pub ca_cert_path: String,
    /// Require client certificate
    pub require_client_cert: bool,
    /// Allowed certificate common names (Optional - allow all if None)
    pub allowed_cns: Option<Vec<String>>,
}

/// Base configuration for any auth method
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthMethodConfig {
    /// Method type
    pub method_type: String,
    /// Whether this method is enabled
    pub enabled: bool,
    /// Required for authentication
    pub required: bool,
    /// Configuration data (method-specific JSON)
    pub config: serde_json::Value,
}

/// Registered authentication method
#[derive(Clone, Debug)]
pub struct RegisteredAuthMethod {
    pub method_type: AuthMethodType,
    pub config: AuthMethodConfig,
}

/// Auth method registry for a tenant
/// 
/// Manages multiple authentication methods per tenant
pub struct AuthMethodRegistry {
    /// Tenant ID
    tenant_id: String,
    /// Registered methods
    methods: Arc<RwLock<HashMap<String, RegisteredAuthMethod>>>,
}

impl AuthMethodRegistry {
    /// Create a new auth method registry for a tenant
    pub fn new(tenant_id: String) -> Self {
        Self {
            tenant_id,
            methods: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an API Key authentication method
    pub async fn register_api_key(
        &self,
        name: String,
        config: ApiKeyConfig,
    ) -> AuthResult<()> {
        let method_config = AuthMethodConfig {
            method_type: AuthMethodType::ApiKey.as_str().to_string(),
            enabled: true,
            required: false,
            config: serde_json::to_value(&config).map_err(|e| {
                AuthError::ConfigurationError(format!("Failed to serialize API key config: {}", e))
            })?,
        };

        let mut methods = self.methods.write().await;
        methods.insert(
            name,
            RegisteredAuthMethod {
                method_type: AuthMethodType::ApiKey,
                config: method_config,
            },
        );

        Ok(())
    }

    /// Register a JWT authentication method
    pub async fn register_jwt(
        &self,
        name: String,
        config: JwtConfig,
    ) -> AuthResult<()> {
        let method_config = AuthMethodConfig {
            method_type: AuthMethodType::Jwt.as_str().to_string(),
            enabled: true,
            required: false,
            config: serde_json::to_value(&config).map_err(|e| {
                AuthError::ConfigurationError(format!("Failed to serialize JWT config: {}", e))
            })?,
        };

        let mut methods = self.methods.write().await;
        methods.insert(
            name,
            RegisteredAuthMethod {
                method_type: AuthMethodType::Jwt,
                config: method_config,
            },
        );

        Ok(())
    }

    /// Register an mTLS authentication method
    pub async fn register_mtls(
        &self,
        name: String,
        config: MtlsConfig,
    ) -> AuthResult<()> {
        let method_config = AuthMethodConfig {
            method_type: AuthMethodType::Mtls.as_str().to_string(),
            enabled: true,
            required: false,
            config: serde_json::to_value(&config).map_err(|e| {
                AuthError::ConfigurationError(format!("Failed to serialize mTLS config: {}", e))
            })?,
        };

        let mut methods = self.methods.write().await;
        methods.insert(
            name,
            RegisteredAuthMethod {
                method_type: AuthMethodType::Mtls,
                config: method_config,
            },
        );

        Ok(())
    }

    /// Register a custom authentication method
    pub async fn register_custom(
        &self,
        name: String,
        config: serde_json::Value,
    ) -> AuthResult<()> {
        let custom_type = name.clone();
        let method_config = AuthMethodConfig {
            method_type: custom_type.clone(),
            enabled: true,
            required: false,
            config,
        };

        let mut methods = self.methods.write().await;
        methods.insert(
            name,
            RegisteredAuthMethod {
                method_type: AuthMethodType::Custom(custom_type),
                config: method_config,
            },
        );

        Ok(())
    }

    /// Get a registered auth method
    pub async fn get_method(&self, name: &str) -> AuthResult<RegisteredAuthMethod> {
        let methods = self.methods.read().await;
        methods
            .get(name)
            .cloned()
            .ok_or_else(|| AuthError::ConfigurationError(format!("Auth method {} not found", name)))
    }

    /// Get all registered auth methods
    pub async fn get_all_methods(&self) -> Vec<RegisteredAuthMethod> {
        let methods = self.methods.read().await;
        methods.values().cloned().collect()
    }

    /// Enable/disable an auth method
    pub async fn set_method_enabled(&self, name: &str, enabled: bool) -> AuthResult<()> {
        let mut methods = self.methods.write().await;
        if let Some(method) = methods.get_mut(name) {
            method.config.enabled = enabled;
            Ok(())
        } else {
            Err(AuthError::ConfigurationError(format!("Auth method {} not found", name)))
        }
    }

    /// Get all enabled auth methods
    pub async fn get_enabled_methods(&self) -> Vec<RegisteredAuthMethod> {
        let methods = self.methods.read().await;
        methods
            .values()
            .filter(|m| m.config.enabled)
            .cloned()
            .collect()
    }

    /// Unregister an auth method
    pub async fn unregister_method(&self, name: &str) -> AuthResult<()> {
        let mut methods = self.methods.write().await;
        methods
            .remove(name)
            .ok_or_else(|| AuthError::ConfigurationError(format!("Auth method {} not found", name)))?;
        Ok(())
    }

    /// Check if a method exists
    pub async fn has_method(&self, name: &str) -> bool {
        let methods = self.methods.read().await;
        methods.contains_key(name)
    }

    /// Get tenant ID
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_api_key() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let config = ApiKeyConfig {
            header_name: "X-API-Key".to_string(),
            api_key: "secret123".to_string(),
        };

        registry.register_api_key("default".to_string(), config).await.unwrap();

        let method = registry.get_method("default").await.unwrap();
        assert_eq!(method.method_type, AuthMethodType::ApiKey);
    }

    #[tokio::test]
    async fn test_register_jwt() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let config = JwtConfig {
            signing_secret: "secret".to_string(),
            algorithm: "HS256".to_string(),
            token_lifetime_secs: 3600,
            issuer: "commy".to_string(),
            audience: "tenant1".to_string(),
        };

        registry.register_jwt("default".to_string(), config).await.unwrap();

        let method = registry.get_method("default").await.unwrap();
        assert_eq!(method.method_type, AuthMethodType::Jwt);
    }

    #[tokio::test]
    async fn test_multiple_methods() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());

        let api_key_config = ApiKeyConfig {
            header_name: "X-API-Key".to_string(),
            api_key: "secret123".to_string(),
        };
        registry.register_api_key("api_key".to_string(), api_key_config).await.unwrap();

        let jwt_config = JwtConfig {
            signing_secret: "secret".to_string(),
            algorithm: "HS256".to_string(),
            token_lifetime_secs: 3600,
            issuer: "commy".to_string(),
            audience: "tenant1".to_string(),
        };
        registry.register_jwt("jwt".to_string(), jwt_config).await.unwrap();

        let all_methods = registry.get_all_methods().await;
        assert_eq!(all_methods.len(), 2);
    }

    #[tokio::test]
    async fn test_enable_disable_method() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let config = ApiKeyConfig {
            header_name: "X-API-Key".to_string(),
            api_key: "secret123".to_string(),
        };

        registry.register_api_key("default".to_string(), config).await.unwrap();
        registry.set_method_enabled("default", false).await.unwrap();

        let enabled = registry.get_enabled_methods().await;
        assert!(enabled.is_empty());
    }

    #[tokio::test]
    async fn test_custom_method() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let config = serde_json::json!({
            "custom_field": "custom_value"
        });

        registry.register_custom("custom".to_string(), config).await.unwrap();

        let method = registry.get_method("custom").await.unwrap();
        assert!(matches!(method.method_type, AuthMethodType::Custom(_)));
    }

    #[test]
    fn test_auth_method_type_as_str() {
        assert_eq!(AuthMethodType::ApiKey.as_str(), "api_key");
        assert_eq!(AuthMethodType::Jwt.as_str(), "jwt");
        assert_eq!(AuthMethodType::Mtls.as_str(), "mtls");
        assert_eq!(AuthMethodType::Custom("my_auth".to_string()).as_str(), "my_auth");
    }

    #[test]
    fn test_auth_method_type_from_str() {
        assert_eq!(AuthMethodType::from_str("api_key"), AuthMethodType::ApiKey);
        assert_eq!(AuthMethodType::from_str("jwt"), AuthMethodType::Jwt);
        assert_eq!(AuthMethodType::from_str("mtls"), AuthMethodType::Mtls);
        assert_eq!(
            AuthMethodType::from_str("custom_name"),
            AuthMethodType::Custom("custom_name".to_string())
        );
    }

    #[test]
    fn test_tenant_id() {
        let registry = AuthMethodRegistry::new("tenant_xyz".to_string());
        assert_eq!(registry.tenant_id(), "tenant_xyz");
    }

    #[tokio::test]
    async fn test_get_all_methods_empty() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let methods = registry.get_all_methods().await;
        assert!(methods.is_empty());
    }

    #[tokio::test]
    async fn test_has_method_true_and_false() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        assert!(!registry.has_method("api_key").await);

        let config = ApiKeyConfig {
            header_name: "X-API-Key".to_string(),
            api_key: "secret".to_string(),
        };
        registry.register_api_key("api_key".to_string(), config).await.unwrap();

        assert!(registry.has_method("api_key").await);
        assert!(!registry.has_method("nonexistent").await);
    }

    #[tokio::test]
    async fn test_unregister_method() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let config = ApiKeyConfig {
            header_name: "X-API-Key".to_string(),
            api_key: "secret".to_string(),
        };
        registry.register_api_key("api_key".to_string(), config).await.unwrap();
        assert!(registry.has_method("api_key").await);

        registry.unregister_method("api_key").await.unwrap();
        assert!(!registry.has_method("api_key").await);
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_returns_error() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let result = registry.unregister_method("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_method_not_found_returns_error() {
        let registry = AuthMethodRegistry::new("tenant1".to_string());
        let result = registry.get_method("does_not_exist").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_mtls() {
        let registry = AuthMethodRegistry::new("tenant_mtls".to_string());
        let config = MtlsConfig {
            ca_cert_path: "/etc/certs/ca.pem".to_string(),
            require_client_cert: true,
            allowed_cns: Some(vec!["client.example.com".to_string()]),
        };
        registry.register_mtls("mtls".to_string(), config).await.unwrap();

        let method = registry.get_method("mtls").await.unwrap();
        assert_eq!(method.method_type, AuthMethodType::Mtls);
        assert!(method.config.enabled);
        assert_eq!(method.config.method_type, "mtls");

        // Also verify it appears in get_all_methods
        let all = registry.get_all_methods().await;
        assert_eq!(all.len(), 1);
    }
}
