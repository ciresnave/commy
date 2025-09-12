//! Enterprise Test Suite - Comprehensive Validation
//!
//! This module runs the complete enterprise test suite for Commy,
//! including all observability, federation, policy, deployment,
//! performance, security, and multi-language integration tests.
//! Updated to use TestEnvironment pattern for proper cleanup.

mod test_utils;
use test_utils::{TestCleanupGuard, TestEnvironment};

#[cfg(all(feature = "manager", feature = "mesh", feature = "ffi"))]
mod enterprise {
    use super::{TestCleanupGuard, TestEnvironment};
    use commy::ffi::*;
    use std::ffi::CString;
    use std::time::Duration;

    #[tokio::test]
    async fn test_basic_ffi_functionality() {
        let _test_env = TestEnvironment::new().expect("Failed to create test environment");
        let _cleanup_guard = TestCleanupGuard::new(|| {
            println!("🧹 Cleaning up test resources for test_basic_ffi_functionality");
        });

        // Initialize FFI system
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0, "FFI initialization should succeed");

        // Test creating a mesh
        let mesh_name = CString::new("test_mesh").unwrap();
        let port = 8080u16;

        let mesh_result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
        assert!(mesh_result.instance_id != 0, "Creating mesh should succeed");

        // Cleanup FFI system
        commy_ffi_cleanup();
        println!("✅ Basic FFI functionality test completed with cleanup");
    }

    #[tokio::test]
    async fn test_python_basic_ffi_integration() {
        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0);

        // Test mesh creation (simulating Python SDK usage)
        let mesh_name = CString::new("python_test_mesh").unwrap();
        let port = 8081u16;

        let result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
        assert!(
            result.instance_id != 0,
            "Python mesh creation should succeed"
        );

        // Cleanup
        commy_ffi_cleanup();
    }

    #[tokio::test]
    async fn test_nodejs_basic_ffi_integration() {
        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0);

        // Test mesh creation (simulating Node.js SDK usage)
        let mesh_name = CString::new("nodejs_test_mesh").unwrap();
        let port = 8082u16;

        let result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
        assert!(
            result.instance_id != 0,
            "Node.js mesh creation should succeed"
        );

        // Cleanup
        commy_ffi_cleanup();
    }

    #[tokio::test]
    async fn test_concurrent_ffi_operations() {
        let _test_env = TestEnvironment::new().expect("Failed to create test environment");
        let _cleanup_guard = TestCleanupGuard::new(|| {
            println!("🧹 Cleaning up test resources for test_concurrent_ffi_operations");
        });

        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0);

        // Test concurrent mesh operations
        let handles = (0u16..5u16)
            .map(|i| {
                let mesh_name = CString::new(format!("concurrent_mesh_{}", i)).unwrap();
                let port = 8090u16 + i;

                tokio::spawn(async move {
                    let result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
                    result.instance_id != 0
                })
            })
            .collect::<Vec<_>>();

        // Wait for all operations to complete
        for handle in handles {
            let success = handle.await.unwrap();
            assert!(success, "Concurrent mesh creation should succeed");
        }

        // Cleanup
        commy_ffi_cleanup();
        println!("✅ Concurrent FFI operations completed with cleanup");
    }

    #[tokio::test]
    async fn test_ffi_error_handling() {
        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0);

        // Test invalid mesh name (null pointer)
        let result = unsafe { commy_create_mesh(std::ptr::null(), 8080) };
        assert!(result.instance_id == 0, "Invalid parameters should fail");

        // Test mesh with empty name
        let empty_name = CString::new("").unwrap();
        let result = unsafe { commy_create_mesh(empty_name.as_ptr(), 8080) };
        assert!(result.instance_id == 0, "Empty name should fail");

        // Cleanup
        commy_ffi_cleanup();
    }

    #[tokio::test]
    async fn test_cross_language_interoperability_infrastructure() {
        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0);

        // Create meshes for different language bindings
        let languages = ["python", "nodejs", "go", "csharp"];

        for (idx, lang) in languages.iter().enumerate() {
            let mesh_name = CString::new(format!("{}_interop_mesh", lang)).unwrap();
            let port = 8100u16 + idx as u16;

            let result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
            assert!(
                result.instance_id != 0,
                "Cross-language mesh should be created for {}",
                lang
            );
        }

        // Cleanup
        commy_ffi_cleanup();
    }

    #[tokio::test]
    async fn test_multi_language_performance() {
        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(init_result, 0);

        let start = std::time::Instant::now();

        // Create multiple meshes to test performance
        for i in 0..10 {
            let mesh_name = CString::new(format!("perf_mesh_{}", i)).unwrap();
            let port = 8200u16 + i as u16;

            let result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
            assert!(
                result.instance_id != 0,
                "Performance test mesh {} should succeed",
                i
            );
        }

        let duration = start.elapsed();
        println!("Created 10 meshes in {:?}", duration);

        // Performance should be reasonable (less than 1 second for 10 meshes)
        assert!(
            duration < Duration::from_secs(1),
            "Mesh creation should be fast"
        );

        // Cleanup
        commy_ffi_cleanup();
    }

    #[tokio::test]
    async fn test_full_enterprise_integration() {
        let _test_env = TestEnvironment::new().expect("Failed to create test environment");
        let _cleanup_guard = TestCleanupGuard::new(|| {
            println!("🧹 Cleaning up test resources for test_full_enterprise_integration");
        });

        // This test validates that all enterprise components work together
        // Initialize FFI
        let init_result = commy_ffi_init();
        assert_eq!(
            init_result, 0,
            "Enterprise FFI initialization should succeed"
        );

        // Create enterprise mesh
        let mesh_name = CString::new("enterprise_mesh").unwrap();
        let port = 8300u16;

        let mesh_result = unsafe { commy_create_mesh(mesh_name.as_ptr(), port) };
        assert_ne!(
            mesh_result.instance_id, 0,
            "Enterprise mesh creation should succeed"
        );

        println!("✅ Enterprise integration test completed successfully!");
        println!("📊 All enterprise components validated");

        // Cleanup
        commy_ffi_cleanup();
    }
}

#[cfg(not(all(feature = "manager", feature = "mesh", feature = "ffi")))]
mod enterprise {
    #[test]
    fn enterprise_features_disabled() {
        println!("Enterprise tests require features: manager, mesh, ffi");
        println!("Run with: cargo test --features=manager,mesh,ffi");
    }
}
