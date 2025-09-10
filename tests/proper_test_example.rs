//! Simple test demonstrating proper cleanup using the new test infrastructure

use test_utils::{TestCleanupGuard, TestEnvironment};

mod test_utils;

#[tokio::test]
async fn test_with_automatic_cleanup() {
    // Create isolated test environment - automatically cleans up when dropped
    let test_env = TestEnvironment::new().expect("Failed to create test environment");
    let _cleanup_guard = TestCleanupGuard::new(|| {
        println!("🧹 Cleaning up test resources for test_with_automatic_cleanup");
    });

    // Use isolated directory for any file operations
    let test_file_path = test_env.test_file_path("test_file.bin");

    // Write some test data
    std::fs::write(&test_file_path, b"test data").expect("Failed to write test file");

    // Verify file exists
    assert!(test_file_path.exists(), "Test file should exist");

    // Perform test operations...
    let data = std::fs::read(&test_file_path).expect("Failed to read test file");
    assert_eq!(data, b"test data");

    println!("✅ Test completed - file will be automatically cleaned up");
    // File is automatically removed when test_env drops its TempDir
    // No test artifacts left behind!
}

#[tokio::test]
async fn test_concurrent_operations_isolated() {
    let test_env = TestEnvironment::new().expect("Failed to create test environment");
    let _cleanup_guard = TestCleanupGuard::new(|| {
        println!("🧹 Cleaning up test resources for test_concurrent_operations_isolated");
    });

    // Each operation works in the same isolated directory
    let mut handles = Vec::new();

    for i in 0..3 {
        let test_dir = test_env.temp_dir.path().to_path_buf();
        let handle = tokio::spawn(async move {
            let test_file = test_dir.join(format!("concurrent_test_{}.bin", i));
            let test_data = format!("test data {}", i);

            // Write test data
            std::fs::write(&test_file, test_data.as_bytes())?;

            // Read it back
            let read_data = std::fs::read(&test_file)?;
            let read_string = String::from_utf8(read_data)?;

            Ok::<String, Box<dyn std::error::Error + Send + Sync>>(read_string)
        });

        handles.push(handle);
    }

    // Wait for all operations
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.expect("Task should not panic");
        let data = result.expect("File operation should succeed");
        assert_eq!(data, format!("test data {}", i));
    }

    println!("✅ Concurrent operations completed with proper isolation");
    // All test files automatically cleaned up when test_env drops
}
