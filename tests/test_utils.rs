//! Test Utilities - Proper Test Cleanup and Isolation
//!
//! This module provides utilities for creating isolated test environments
//! with automatic cleanup to prevent test artifacts from accumulating.

use commy::manager::core::ManagerConfig;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test environment with automatic cleanup
pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub config: ManagerConfig,
}

impl TestEnvironment {
    /// Create a new isolated test environment
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = ManagerConfig {
            listen_port: 0, // Random port for testing
            bind_address: "127.0.0.1".to_string(),
            max_files: 100,
            max_file_size: 10 * 1024 * 1024, // 10MB
            default_ttl_seconds: 30,         // Short TTL for tests
            heartbeat_timeout_seconds: 5,
            cleanup_interval_seconds: 1, // Aggressive cleanup for tests
            database_path: temp_dir.path().join("test_db.sqlite"),
            files_directory: temp_dir.path().to_path_buf(),
            tls_cert_path: None,
            tls_key_path: None,
            require_tls: false,
            performance_config: Default::default(),
            security_config: Default::default(),
            enable_mesh_capabilities: false, // Disable mesh for testing
        };
        // Ensure integration tests that use this TestEnvironment opt into
        // test-only behaviors which the library gates behind the TEST_ENV
        // environment variable when compiled as a normal (non-unit-test)
        // build. Integration tests compile the library without
        // `cfg(test)`, so set the variable here so the library permits
        // usage of MockAuthProvider during tests.
        if std::env::var("TEST_ENV").unwrap_or_default() != "1" {
            std::env::set_var("TEST_ENV", "1");
        }

        Ok(TestEnvironment { temp_dir, config })
    }

    /// Get a unique test file path within the temporary directory
    pub fn test_file_path(&self, filename: &str) -> PathBuf {
        self.temp_dir.path().join(filename)
    }
}

/// RAII guard for test cleanup
pub struct TestCleanupGuard {
    cleanup_fn: Box<dyn FnOnce() + Send>,
}

impl TestCleanupGuard {
    pub fn new<F>(cleanup_fn: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        TestCleanupGuard {
            cleanup_fn: Box::new(cleanup_fn),
        }
    }
}

impl Drop for TestCleanupGuard {
    fn drop(&mut self) {
        // Execute cleanup function
        let cleanup_fn = std::mem::replace(&mut self.cleanup_fn, Box::new(|| {}));
        cleanup_fn();
        println!("🧹 Test cleanup completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_creation() {
        let env = TestEnvironment::new().unwrap();
        assert!(env.temp_dir.path().exists());
        assert!(env.config.files_directory.starts_with(env.temp_dir.path()));
    }

    #[test]
    fn test_cleanup_guard() {
        use std::sync::{Arc, Mutex};

        let cleaned_up = Arc::new(Mutex::new(false));
        let cleaned_up_clone = cleaned_up.clone();

        {
            let _guard = TestCleanupGuard::new(move || {
                *cleaned_up_clone.lock().unwrap() = true;
            });
            // Guard should not have cleaned up yet
            assert!(!*cleaned_up.lock().unwrap());
        }
        // Guard should have cleaned up when dropped
        assert!(*cleaned_up.lock().unwrap());
    }
}
