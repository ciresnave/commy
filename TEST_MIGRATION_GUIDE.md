# Test Migration Guide

## Overview

This guide explains how to migrate existing tests to use the TestEnvironment pattern for proper resource isolation and cleanup.

## TestEnvironment Pattern

### Basic Usage

```rust
use crate::test_utils::{TestEnvironment, TestCleanupGuard};

#[tokio::test]
async fn test_example() {
    let env = TestEnvironment::new("test_example").await;
    let _cleanup = TestCleanupGuard::new(&env);

    // Your test code here - all files will be created in env.files_dir()
    // Automatic cleanup happens when _cleanup goes out of scope
}
```

### Benefits

- **Isolation**: Each test gets its own temporary directory
- **Automatic Cleanup**: Files are automatically removed after test completion
- **No Conflicts**: Tests can run concurrently without interference
- **Consistent Paths**: Standardized directory structure for all tests

## Migration Patterns

### Pattern 1: Simple Path Updates (✅ Ready to Migrate)

For tests that only need path updates:

**Before:**

```rust
#[tokio::test]
async fn test_something() {
    let config = create_test_config("./test_files", 8080).await;
    // ... test code
}
```

**After:**

```rust
use crate::test_utils::{TestEnvironment, TestCleanupGuard};

#[tokio::test]
async fn test_something() {
    let env = TestEnvironment::new("test_something").await;
    let _cleanup = TestCleanupGuard::new(&env);

    let config = create_test_config(env.files_dir(), 8080).await;
    // ... test code
}
```

### Pattern 2: API Updates Required (⚠️ Needs API Modernization)

For tests using outdated API structures:

**Current Issue:**

```rust
// This fails because SharedFileRequest structure has evolved
let request = SharedFileRequest {
    file_path: PathBuf::from("test.txt"),
    size: 1024,
    checksum: "abc123".to_string(),
    requester_id: "test".to_string(),
    chunk_ranges: vec![], // Missing: identifier, pattern, pattern_config, etc.
};
```

**Required Updates:**

1. Update SharedFileRequest to include all required fields
2. Update configuration structures to match current API
3. Update manager creation calls to use current constructor

### Pattern 3: FFI Tests (✅ Successfully Migrated)

Already updated in `tests/enterprise_full_suite.rs`:

```rust
use crate::test_utils::{TestEnvironment, TestCleanupGuard};

#[tokio::test]
async fn test_basic_ffi_functionality() {
    let env = TestEnvironment::new("ffi_basic_test").await;
    let _cleanup = TestCleanupGuard::new(&env);

    // Use env.db_dir() and env.files_dir() for FFI operations
    let db_dir = CString::new(env.db_dir().to_string_lossy().as_ref()).unwrap();
    let files_dir = CString::new(env.files_dir().to_string_lossy().as_ref()).unwrap();

    // ... FFI test code
}
```

## Migration Status

### ✅ Successfully Migrated

- `tests/enterprise_full_suite.rs` - FFI tests with proper isolation
- `tests/proper_test_example.rs` - Reference implementation

### ⚠️ Partially Updated (Need API Updates)

- `tests/simple_integration.rs` - TestEnvironment added, but needs SharedFileManager API updates
- `tests/phase2_memory_mapping_test.rs` - TestEnvironment added, but needs SharedFileRequest API updates

### 📋 Not Yet Started

Use `grep_search` to find other test files that need migration.

## API Update Requirements

### SharedFileRequest Structure

Current tests fail because they're missing required fields:

```rust
// Old structure (fails)
SharedFileRequest {
    file_path: PathBuf::from("test.txt"),
    size: 1024,
    checksum: "abc123".to_string(),
    requester_id: "test".to_string(),
    chunk_ranges: vec![],
}

// New structure (required fields)
SharedFileRequest {
    identifier: "unique_id".to_string(),
    file_path: PathBuf::from("test.txt"),
    size: 1024,
    checksum: "abc123".to_string(),
    requester_id: "test".to_string(),
    chunk_ranges: vec![],
    pattern: FilePattern::Exact,  // New required field
    pattern_config: PatternConfig::default(),  // New required field
    // ... other new fields as needed
}
```

### Manager Configuration

Update manager creation calls to use current API. The `SharedFileManager` constructor
now accepts a `ManagerConfig` value. For example:

```rust
use commy::manager::core::{ManagerConfig, SharedFileManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a default configuration (override fields as needed)
    let config = ManagerConfig::default();

    // Construct the manager with the configuration
    let manager = SharedFileManager::new(config).await?;

    // Use `manager` for file operations, starting the service, etc.
    Ok(())
}
```

## Testing the Migration

### Run Individual Tests

```bash
cargo test --test proper_test_example  # Reference implementation
cargo test --test enterprise_full_suite  # FFI tests
```

### Verify Cleanup

After running tests, check that no artifacts remain:

```bash
ls -la  # Should not see .mmap files or test directories
```

### Check for Hardcoded Paths

```bash
grep -r "test_files\|\.mmap\|temp_\|/tmp" tests/  # Should find minimal results
```

## Next Steps

1. **Find Remaining Tests**: Use `file_search` to find all test files
2. **Categorize Tests**: Determine which need simple path updates vs API updates
3. **Migrate Simple Tests**: Update tests that only need path changes
4. **Document API Changes**: Create guide for updating SharedFileRequest and related structures
5. **Test Migration**: Verify each migrated test works correctly

## Best Practices

### Directory Usage

- Use `env.db_dir()` for database files
- Use `env.files_dir()` for file operations
- Use `env.temp_dir()` for temporary files

### Cleanup Guards

- Always create `TestCleanupGuard` early in test
- Store as `_cleanup` to ensure it's not dropped early
- Multiple guards can be created if needed

### Test Naming

- Use descriptive test names for TestEnvironment constructor
- Helps identify which test created which directories during debugging

### Error Handling

- TestEnvironment creation can fail - use `.await` properly
- Consider using `Result` return types for complex test setup

## Example Test Template

```rust
use crate::test_utils::{TestEnvironment, TestCleanupGuard};

#[tokio::test]
async fn test_your_feature() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnvironment::new("your_feature_test").await;
    let _cleanup = TestCleanupGuard::new(&env);

    // 1. Create configuration using env.files_dir() and env.db_dir()
    let config = create_test_config(env.files_dir(), 8080).await;

    // 2. Set up test data in isolated directories
    let test_file = env.files_dir().join("test.txt");
    tokio::fs::write(&test_file, "test data").await?;

    // 3. Run your test
    let result = your_function_under_test(&config).await?;

    // 4. Assert results
    assert_eq!(result.status, "success");

    // 5. Cleanup happens automatically when _cleanup drops
    Ok(())
}
```
