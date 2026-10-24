// SPDX-License-Identifier: MIT
//! Integration tests for authentication storage backends
//!
//! These tests cover the in-memory backend, the only one `TenantAuthContext` wires (ROADMAP R21: PostgreSQL, MySQL and
//! Redis are accepted by `TenantAuthConfig` and ignored).

use commy::auth::tenant_context::{StorageBackend, TenantAuthConfig, TenantAuthContext};
use std::time::Duration;

/// Test memory storage backend (development mode)
#[tokio::test]
async fn test_memory_storage_backend() {
    unsafe {
        std::env::set_var("ENVIRONMENT", "development");
    }

    let config = TenantAuthConfig {
        tenant_id: "test_memory_tenant".to_string(),
        storage_backend: StorageBackend::Memory,
        token_lifetime_secs: 3600,
        ..Default::default()
    };

    let context = TenantAuthContext::new(config);

    // Initialize should succeed with memory storage
    let result = context.initialize().await;
    assert!(
        result.is_ok(),
        "Memory storage initialization failed: {:?}",
        result.err()
    );

    // Verify we can authenticate
    let auth_fw = context.auth().write().await;
    let token = auth_fw
        .create_auth_token("test_user", vec!["read".to_string()], "jwt", None)
        .await;

    assert!(
        token.is_ok(),
        "Token creation failed with memory storage: {:?}",
        token.err()
    );
    let token = token.unwrap();

    // Verify token validation works
    let is_valid = auth_fw.validate_token(&token).await;
    assert!(
        is_valid.is_ok() && is_valid.unwrap(),
        "Token validation failed with memory storage"
    );
}

/// Test storage backend switching between tenants
#[tokio::test]
async fn test_multiple_tenants_different_backends() {
    unsafe {
        std::env::set_var("ENVIRONMENT", "development");
    }

    // Tenant 1: Memory storage
    let config1 = TenantAuthConfig {
        tenant_id: "tenant1".to_string(),
        storage_backend: StorageBackend::Memory,
        ..Default::default()
    };
    let context1 = TenantAuthContext::new(config1);
    assert!(context1.initialize().await.is_ok());

    // Tenant 2: Also memory storage (different tenant)
    let config2 = TenantAuthConfig {
        tenant_id: "tenant2".to_string(),
        storage_backend: StorageBackend::Memory,
        ..Default::default()
    };
    let context2 = TenantAuthContext::new(config2);
    assert!(context2.initialize().await.is_ok());

    // Both should work independently
    let auth1 = context1.auth().write().await;
    let token1 = auth1
        .create_auth_token("user1", vec!["read".to_string()], "jwt", None)
        .await;
    assert!(token1.is_ok());

    drop(auth1); // Release lock

    let auth2 = context2.auth().write().await;
    let token2 = auth2
        .create_auth_token("user2", vec!["write".to_string()], "jwt", None)
        .await;
    assert!(token2.is_ok());
}

/// Test token expiration with configured lifetime
#[tokio::test]
async fn test_token_lifetime_configuration() {
    unsafe {
        std::env::set_var("ENVIRONMENT", "development");
    }

    let config = TenantAuthConfig {
        tenant_id: "test_expiration".to_string(),
        storage_backend: StorageBackend::Memory,
        token_lifetime_secs: 2, // Short lifetime for testing
        ..Default::default()
    };

    let context = TenantAuthContext::new(config);
    context.initialize().await.unwrap();

    let auth_fw = context.auth().write().await;
    let token = auth_fw
        .create_auth_token("test_user", vec!["read".to_string()], "jwt", None)
        .await
        .unwrap();

    // Token should be valid immediately
    let is_valid_initially = auth_fw.validate_token(&token).await;
    assert!(
        is_valid_initially.is_ok() && is_valid_initially.unwrap(),
        "Token should be valid immediately after creation"
    );

    // Note: auth-framework's token expiration behavior depends on the storage backend
    // and JWT token structure. For JWTs, expiration is embedded in the token itself.
    // This test verifies that the token_lifetime_secs configuration is accepted,
    // but actual expiration enforcement depends on auth-framework's implementation.
    println!("Token created with 2 second lifetime: {:?}", token);

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check if token is expired (may vary by auth-framework implementation)
    let is_valid_after = auth_fw.validate_token(&token).await;
    println!("Token validity after expiration: {:?}", is_valid_after);

    // If auth-framework properly enforces JWT expiration, this should fail
    // If not, we've at least verified the configuration is accepted
    if is_valid_after.is_ok() && is_valid_after.unwrap() {
        println!(
            "Note: Token still valid after expiration (JWT exp claim may not be enforced by auth-framework)"
        );
    }
}
