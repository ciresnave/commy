# Running the Examples

This directory contains example applications demonstrating the `commy` library's capability to solve dependency conflicts through Inter-Process Communication (IPC).

## Examples

### Producer and Consumer

The producer/consumer examples demonstrate basic IPC communication:

```bash
# Terminal 1 - Start the producer
cargo run --example producer

# Terminal 2 - Start the consumer
cargo run --example consumer
```

The producer creates shared data in memory-mapped files, while the consumer reads from them. This shows how separate processes can communicate without sharing dependencies.

### Key Features Demonstrated

- **Memory-mapped file IPC**: High-performance cross-process communication
- **Process synchronization**: File-based locking prevents data races
- **Real-time callbacks**: Notification system for data changes
- **Dependency isolation**: Each process can use different library versions

## Dependency Isolation Scenario

Imagine you have:

- **Service A**: Uses `serde v1.0` for JSON parsing
- **Service B**: Uses `serde v2.0` for JSON parsing

Normally, these would conflict in a single Rust binary. With `commy`:

1. Run Service A as separate process with `serde v1.0`
2. Run Service B as separate process with `serde v2.0`
3. They communicate via memory-mapped files
4. No dependency conflicts!

## Running Tests

```bash
# Run integration tests
cargo test

# Run specific test
cargo test test_dependency_isolation

# Run with output
cargo test -- --nocapture
```

The integration tests demonstrate:

- Dependency isolation between processes
- Concurrent readers and writers
- Callback system functionality
- Process synchronization
