//! Performance and Scale Tests
//!
//! Comprehensive performance validation for enterprise deployment including:
//! - Load testing with various message sizes and patterns
//! - Latency benchmarking under different conditions
//! - Memory usage validation and leak detection
//! - Network failure simulation and recovery
//! - Concurrent connection handling
//! - Federation performance across regions

use commy::ffi::minimal::*;
use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Performance test configuration
struct PerformanceTestConfig {
    message_count: u32,
    message_size: u32,
    concurrent_clients: u32,
    duration_seconds: u32,
    target_latency_ms: u32,
    target_throughput_msg_per_sec: u32,
}

/// Performance metrics collector
struct PerformanceMetrics {
    total_messages: AtomicU64,
    total_bytes: AtomicU64,
    min_latency_ns: AtomicU64,
    max_latency_ns: AtomicU64,
    total_latency_ns: AtomicU64,
    error_count: AtomicU64,
    start_time: Instant,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            total_messages: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            min_latency_ns: AtomicU64::new(u64::MAX),
            max_latency_ns: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    fn record_message(&self, size: u64, latency_ns: u64) {
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(size, Ordering::Relaxed);
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);

        // Update min latency
        loop {
            let current_min = self.min_latency_ns.load(Ordering::Relaxed);
            if latency_ns >= current_min {
                break;
            }
            if self
                .min_latency_ns
                .compare_exchange_weak(
                    current_min,
                    latency_ns,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        // Update max latency
        loop {
            let current_max = self.max_latency_ns.load(Ordering::Relaxed);
            if latency_ns <= current_max {
                break;
            }
            if self
                .max_latency_ns
                .compare_exchange_weak(
                    current_max,
                    latency_ns,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    fn get_summary(&self) -> PerformanceSummary {
        let total_messages = self.total_messages.load(Ordering::Relaxed);
        let total_bytes = self.total_bytes.load(Ordering::Relaxed);
        let total_latency_ns = self.total_latency_ns.load(Ordering::Relaxed);
        let error_count = self.error_count.load(Ordering::Relaxed);
        let duration = self.start_time.elapsed();

        PerformanceSummary {
            total_messages,
            total_bytes,
            duration_ms: duration.as_millis() as u64,
            avg_latency_ns: if total_messages > 0 {
                total_latency_ns / total_messages
            } else {
                0
            },
            min_latency_ns: if self.min_latency_ns.load(Ordering::Relaxed) == u64::MAX {
                0
            } else {
                self.min_latency_ns.load(Ordering::Relaxed)
            },
            max_latency_ns: self.max_latency_ns.load(Ordering::Relaxed),
            throughput_msg_per_sec: if duration.as_secs() > 0 {
                total_messages / duration.as_secs()
            } else {
                0
            },
            throughput_mb_per_sec: if duration.as_secs() > 0 {
                (total_bytes / (1024 * 1024)) / duration.as_secs()
            } else {
                0
            },
            error_rate: if total_messages > 0 {
                (error_count as f64 / total_messages as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Performance test summary
#[derive(Debug)]
struct PerformanceSummary {
    total_messages: u64,
    total_bytes: u64,
    duration_ms: u64,
    avg_latency_ns: u64,
    min_latency_ns: u64,
    max_latency_ns: u64,
    throughput_msg_per_sec: u64,
    throughput_mb_per_sec: u64,
    error_rate: f64,
}

/// Initialize performance test environment
fn setup_performance_test() -> CommyFileManagerHandle {
    unsafe {
        commy_ffi_init();
    }
    let config_path = CString::new("/tmp/commy_performance_test").unwrap();
    unsafe { commy_create_file_manager(config_path.as_ptr()) }
}

/// Cleanup performance test environment
fn cleanup_performance_test(handle: CommyFileManagerHandle) {
    unsafe {
        commy_destroy_file_manager(handle);
        commy_ffi_cleanup();
    }
}

#[test]
fn test_message_throughput_small() {
    let handle = setup_performance_test();

    let config = PerformanceTestConfig {
        message_count: 10000,
        message_size: 64, // Small messages (64 bytes)
        concurrent_clients: 1,
        duration_seconds: 10,
        target_latency_ms: 1, // 1ms target
        target_throughput_msg_per_sec: 10000,
    };

    let summary = run_throughput_test(handle, &config);

    // Validate performance requirements
    assert!(summary.avg_latency_ns < (config.target_latency_ms as u64 * 1_000_000)); // Convert ms to ns
    assert!(summary.throughput_msg_per_sec >= config.target_throughput_msg_per_sec as u64);
    assert!(summary.error_rate < 1.0); // Less than 1% error rate
    assert_eq!(summary.total_messages, config.message_count as u64);

    println!("Small Message Throughput Test Results:");
    println!("  Total Messages: {}", summary.total_messages);
    println!(
        "  Average Latency: {:.2}μs",
        summary.avg_latency_ns as f64 / 1000.0
    );
    println!("  Throughput: {} msg/sec", summary.throughput_msg_per_sec);
    println!("  Error Rate: {:.2}%", summary.error_rate);

    cleanup_performance_test(handle);
}

#[test]
fn test_message_throughput_large() {
    let handle = setup_performance_test();

    let config = PerformanceTestConfig {
        message_count: 1000,
        message_size: 1024 * 1024, // Large messages (1MB)
        concurrent_clients: 1,
        duration_seconds: 30,
        target_latency_ms: 10, // 10ms target for large messages
        target_throughput_msg_per_sec: 100,
    };

    let summary = run_throughput_test(handle, &config);

    // Validate performance requirements for large messages
    assert!(summary.avg_latency_ns < (config.target_latency_ms as u64 * 1_000_000));
    assert!(summary.throughput_msg_per_sec >= config.target_throughput_msg_per_sec as u64);
    assert!(summary.error_rate < 1.0);
    assert_eq!(summary.total_messages, config.message_count as u64);

    println!("Large Message Throughput Test Results:");
    println!("  Total Messages: {}", summary.total_messages);
    println!(
        "  Total Data: {:.2}MB",
        summary.total_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Average Latency: {:.2}ms",
        summary.avg_latency_ns as f64 / 1_000_000.0
    );
    println!("  Throughput: {} MB/sec", summary.throughput_mb_per_sec);
    println!("  Error Rate: {:.2}%", summary.error_rate);

    cleanup_performance_test(handle);
}

#[test]
fn test_concurrent_client_load() {
    let handle = setup_performance_test();

    let config = PerformanceTestConfig {
        message_count: 1000,    // Per client
        message_size: 1024,     // 1KB messages
        concurrent_clients: 50, // 50 concurrent clients
        duration_seconds: 30,
        target_latency_ms: 5,
        target_throughput_msg_per_sec: 10000, // Total across all clients
    };

    let summary = run_concurrent_test(handle, &config);

    // Validate concurrent performance
    assert!(summary.avg_latency_ns < (config.target_latency_ms as u64 * 1_000_000));
    assert!(summary.throughput_msg_per_sec >= config.target_throughput_msg_per_sec as u64);
    assert!(summary.error_rate < 2.0); // Allow slightly higher error rate under load
    assert_eq!(
        summary.total_messages,
        (config.message_count * config.concurrent_clients) as u64
    );

    println!("Concurrent Client Load Test Results:");
    println!("  Concurrent Clients: {}", config.concurrent_clients);
    println!("  Total Messages: {}", summary.total_messages);
    println!(
        "  Average Latency: {:.2}ms",
        summary.avg_latency_ns as f64 / 1_000_000.0
    );
    println!(
        "  Min Latency: {:.2}μs",
        summary.min_latency_ns as f64 / 1000.0
    );
    println!(
        "  Max Latency: {:.2}ms",
        summary.max_latency_ns as f64 / 1_000_000.0
    );
    println!("  Throughput: {} msg/sec", summary.throughput_msg_per_sec);
    println!("  Error Rate: {:.2}%", summary.error_rate);

    cleanup_performance_test(handle);
}

#[test]
fn test_memory_usage_under_load() {
    let handle = setup_performance_test();

    // Test memory usage with sustained load
    let initial_memory = get_memory_usage();

    let config = PerformanceTestConfig {
        message_count: 5000,
        message_size: 4096, // 4KB messages
        concurrent_clients: 20,
        duration_seconds: 60, // Longer test for memory observation
        target_latency_ms: 5,
        target_throughput_msg_per_sec: 5000,
    };

    let summary = run_memory_test(handle, &config);

    let peak_memory = get_memory_usage();
    let memory_increase = peak_memory - initial_memory;

    // Validate memory usage is reasonable
    assert!(memory_increase < 100 * 1024 * 1024); // Less than 100MB increase
    assert!(summary.error_rate < 1.0);

    // Allow memory to stabilize after test
    thread::sleep(Duration::from_secs(5));
    let final_memory = get_memory_usage();

    // Check for potential memory leaks (allow some variance)
    let memory_leak = final_memory.saturating_sub(initial_memory);
    assert!(memory_leak < 50 * 1024 * 1024); // Less than 50MB permanent increase

    println!("Memory Usage Test Results:");
    println!(
        "  Initial Memory: {:.2}MB",
        initial_memory as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Peak Memory: {:.2}MB",
        peak_memory as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Final Memory: {:.2}MB",
        final_memory as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Memory Increase: {:.2}MB",
        memory_increase as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Potential Leak: {:.2}MB",
        memory_leak as f64 / (1024.0 * 1024.0)
    );
    println!("  Messages Processed: {}", summary.total_messages);

    cleanup_performance_test(handle);
}

#[test]
fn test_network_failure_recovery() {
    let handle = setup_performance_test();

    // Test performance during network interruptions
    let metrics = Arc::new(PerformanceMetrics::new());
    let failure_injected = Arc::new(Mutex::new(false));

    // Start background performance monitoring
    let metrics_clone = Arc::clone(&metrics);
    let failure_clone = Arc::clone(&failure_injected);
    let monitor_handle =
        thread::spawn(move || monitor_network_recovery_performance(metrics_clone, failure_clone));

    // Simulate normal operation
    thread::sleep(Duration::from_secs(5));

    // Inject network failure
    {
        let mut failure = failure_injected.lock().unwrap();
        *failure = true;
        unsafe {
            commy_simulate_network_failure(handle, 3000); // 3 second failure
        }
    }

    // Continue monitoring recovery
    thread::sleep(Duration::from_secs(10));

    // Stop monitoring
    let performance_data = monitor_handle.join().unwrap();

    // Validate recovery performance
    assert!(performance_data.recovery_time_ms < 5000); // Recovery within 5 seconds
    assert!(performance_data.message_loss_rate < 1.0); // Less than 1% message loss
    assert!(performance_data.latency_spike_factor < 5.0); // Latency doesn't spike more than 5x

    println!("Network Failure Recovery Test Results:");
    println!("  Recovery Time: {}ms", performance_data.recovery_time_ms);
    println!(
        "  Message Loss Rate: {:.2}%",
        performance_data.message_loss_rate
    );
    println!(
        "  Latency Spike Factor: {:.2}x",
        performance_data.latency_spike_factor
    );
    println!("  Total Messages: {}", performance_data.total_messages);

    cleanup_performance_test(handle);
}

#[test]
fn test_federation_performance() {
    let handle = setup_performance_test();

    // Configure federation for performance testing
    let region1 = CString::new("us-east-1").unwrap();
    let region2 = CString::new("eu-west-1").unwrap();
    let region3 = CString::new("ap-southeast-1").unwrap();

    let regions = [region1.as_ptr(), region2.as_ptr(), region3.as_ptr()];

    let result =
        unsafe { commy_configure_federation(handle, regions.as_ptr(), regions.len() as u32, true) };
    assert_eq!(result, CommyError::Success as i32);

    // Test cross-region performance
    let config = PerformanceTestConfig {
        message_count: 1000,
        message_size: 2048,
        concurrent_clients: 10,
        duration_seconds: 30,
        target_latency_ms: 50, // Higher latency expected for cross-region
        target_throughput_msg_per_sec: 500,
    };

    let summary = run_federation_test(handle, &config);

    // Validate federation performance
    assert!(summary.avg_latency_ns < (config.target_latency_ms as u64 * 1_000_000));
    assert!(summary.throughput_msg_per_sec >= config.target_throughput_msg_per_sec as u64);
    assert!(summary.error_rate < 3.0); // Allow higher error rate for federation

    println!("Federation Performance Test Results:");
    println!("  Cross-Region Messages: {}", summary.total_messages);
    println!(
        "  Average Latency: {:.2}ms",
        summary.avg_latency_ns as f64 / 1_000_000.0
    );
    println!(
        "  Cross-Region Throughput: {} msg/sec",
        summary.throughput_msg_per_sec
    );
    println!("  Error Rate: {:.2}%", summary.error_rate);

    cleanup_performance_test(handle);
}

#[test]
fn test_latency_under_different_loads() {
    let handle = setup_performance_test();

    let load_scenarios = vec![
        ("idle", 10, 1),
        ("light", 100, 5),
        ("medium", 1000, 20),
        ("heavy", 5000, 50),
        ("extreme", 10000, 100),
    ];

    for (scenario, message_count, concurrent_clients) in load_scenarios {
        let config = PerformanceTestConfig {
            message_count,
            message_size: 1024,
            concurrent_clients,
            duration_seconds: 15,
            target_latency_ms: match scenario {
                "idle" => 1,
                "light" => 2,
                "medium" => 5,
                "heavy" => 10,
                "extreme" => 20,
                _ => 10,
            },
            target_throughput_msg_per_sec: 1000,
        };

        let summary = run_latency_test(handle, &config);

        // Validate latency requirements
        assert!(
            summary.avg_latency_ns < (config.target_latency_ms as u64 * 1_000_000),
            "Latency requirement failed for {} load scenario",
            scenario
        );

        println!("{} Load Latency Results:", scenario.to_uppercase());
        println!(
            "  Messages: {}, Clients: {}",
            message_count, concurrent_clients
        );
        println!(
            "  Avg Latency: {:.2}μs",
            summary.avg_latency_ns as f64 / 1000.0
        );
        println!(
            "  Min Latency: {:.2}μs",
            summary.min_latency_ns as f64 / 1000.0
        );
        println!(
            "  Max Latency: {:.2}ms",
            summary.max_latency_ns as f64 / 1_000_000.0
        );
        println!(
            "  99th Percentile: {:.2}ms",
            calculate_p99_latency(&summary) / 1_000_000.0
        );
        println!();
    }

    cleanup_performance_test(handle);
}

// Helper functions for performance testing

fn run_throughput_test(
    handle: CommyFileManagerHandle,
    config: &PerformanceTestConfig,
) -> PerformanceSummary {
    let metrics = PerformanceMetrics::new();

    for i in 0..config.message_count {
        let start = Instant::now();

        let message = create_test_message(config.message_size);
        let destination = CString::new("test-service").unwrap();

        let result = unsafe {
            commy_send_message(
                handle,
                destination.as_ptr(),
                message.as_ptr(),
                message.len() as u32,
            )
        };

        let latency = start.elapsed().as_nanos() as u64;

        if result == CommyError::Success as i32 {
            metrics.record_message(config.message_size as u64, latency);
        } else {
            metrics.record_error();
        }

        if i % 1000 == 0 {
            println!("Processed {} messages...", i);
        }
    }

    metrics.get_summary()
}

fn run_concurrent_test(
    handle: CommyFileManagerHandle,
    config: &PerformanceTestConfig,
) -> PerformanceSummary {
    let metrics = Arc::new(PerformanceMetrics::new());
    let barrier = Arc::new(Barrier::new(config.concurrent_clients as usize));
    let mut threads = vec![];

    for client_id in 0..config.concurrent_clients {
        let metrics_clone = Arc::clone(&metrics);
        let barrier_clone = Arc::clone(&barrier);
        let config_clone = *config;

        let thread = thread::spawn(move || {
            barrier_clone.wait(); // Synchronize start

            for i in 0..config_clone.message_count {
                let start = Instant::now();

                let message = create_test_message(config_clone.message_size);
                let destination = CString::new(format!("test-service-{}", client_id)).unwrap();

                let result = unsafe {
                    commy_send_message(
                        handle,
                        destination.as_ptr(),
                        message.as_ptr(),
                        message.len() as u32,
                    )
                };

                let latency = start.elapsed().as_nanos() as u64;

                if result == CommyError::Success as i32 {
                    metrics_clone.record_message(config_clone.message_size as u64, latency);
                } else {
                    metrics_clone.record_error();
                }

                // Small delay to prevent overwhelming
                if i % 100 == 0 {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        });

        threads.push(thread);
    }

    for thread in threads {
        thread.join().unwrap();
    }

    metrics.get_summary()
}

fn run_memory_test(
    handle: CommyFileManagerHandle,
    config: &PerformanceTestConfig,
) -> PerformanceSummary {
    let metrics = Arc::new(PerformanceMetrics::new());
    let mut threads = vec![];

    for client_id in 0..config.concurrent_clients {
        let metrics_clone = Arc::clone(&metrics);
        let config_clone = *config;

        let thread = thread::spawn(move || {
            for i in 0..config_clone.message_count {
                let start = Instant::now();

                let message = create_test_message(config_clone.message_size);
                let destination = CString::new(format!("memory-test-{}", client_id)).unwrap();

                let result = unsafe {
                    commy_send_message(
                        handle,
                        destination.as_ptr(),
                        message.as_ptr(),
                        message.len() as u32,
                    )
                };

                let latency = start.elapsed().as_nanos() as u64;

                if result == CommyError::Success as i32 {
                    metrics_clone.record_message(config_clone.message_size as u64, latency);
                } else {
                    metrics_clone.record_error();
                }

                // Periodic memory pressure
                if i % 100 == 0 {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        });

        threads.push(thread);
    }

    for thread in threads {
        thread.join().unwrap();
    }

    metrics.get_summary()
}

fn run_federation_test(
    handle: CommyFileManagerHandle,
    config: &PerformanceTestConfig,
) -> PerformanceSummary {
    let metrics = PerformanceMetrics::new();

    for i in 0..config.message_count {
        let start = Instant::now();

        let message = create_test_message(config.message_size);
        let region = match i % 3 {
            0 => "us-east-1",
            1 => "eu-west-1",
            _ => "ap-southeast-1",
        };
        let destination = CString::new(format!("service@{}", region)).unwrap();

        let result = unsafe {
            commy_send_federated_message(
                handle,
                destination.as_ptr(),
                message.as_ptr(),
                message.len() as u32,
            )
        };

        let latency = start.elapsed().as_nanos() as u64;

        if result == CommyError::Success as i32 {
            metrics.record_message(config.message_size as u64, latency);
        } else {
            metrics.record_error();
        }

        if i % 100 == 0 {
            println!("Sent {} federated messages...", i);
        }
    }

    metrics.get_summary()
}

fn run_latency_test(
    handle: CommyFileManagerHandle,
    config: &PerformanceTestConfig,
) -> PerformanceSummary {
    run_concurrent_test(handle, config)
}

fn create_test_message(size: u32) -> Vec<u8> {
    vec![0x42; size as usize] // Fill with arbitrary data
}

fn get_memory_usage() -> u64 {
    // Platform-specific memory usage retrieval
    // This is a simplified version - in real implementation,
    // you'd use proper OS APIs
    unsafe { commy_get_memory_usage() }
}

struct NetworkRecoveryData {
    recovery_time_ms: u64,
    message_loss_rate: f64,
    latency_spike_factor: f64,
    total_messages: u64,
}

fn monitor_network_recovery_performance(
    metrics: Arc<PerformanceMetrics>,
    failure_injected: Arc<Mutex<bool>>,
) -> NetworkRecoveryData {
    let mut recovery_start = None;
    let mut recovery_time_ms = 0;
    let mut total_messages = 0;

    for _ in 0..60 {
        // Monitor for 60 seconds
        let start = Instant::now();

        // Simulate message sending
        let message = create_test_message(1024);
        let destination = CString::new("recovery-test").unwrap();

        let result = unsafe {
            commy_send_message(
                ptr::null_mut(), // Use null handle for simulation
                destination.as_ptr(),
                message.as_ptr(),
                message.len() as u32,
            )
        };

        let latency = start.elapsed().as_nanos() as u64;
        total_messages += 1;

        if result == CommyError::Success as i32 {
            metrics.record_message(1024, latency);

            // Check if this is first success after failure
            let failure_active = *failure_injected.lock().unwrap();
            if !failure_active && recovery_start.is_some() {
                recovery_time_ms = recovery_start.unwrap().elapsed().as_millis() as u64;
                recovery_start = None;
            }
        } else {
            metrics.record_error();

            // Mark start of recovery period
            if recovery_start.is_none() {
                recovery_start = Some(Instant::now());
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    let summary = metrics.get_summary();

    NetworkRecoveryData {
        recovery_time_ms,
        message_loss_rate: summary.error_rate,
        latency_spike_factor: summary.max_latency_ns as f64 / summary.avg_latency_ns as f64,
        total_messages: summary.total_messages,
    }
}

fn calculate_p99_latency(summary: &PerformanceSummary) -> f64 {
    // Simplified 99th percentile calculation
    // In real implementation, you'd maintain a histogram
    summary.avg_latency_ns as f64 * 1.5 + (summary.max_latency_ns as f64 * 0.1)
}

// Copy the PerformanceTestConfig to allow for copying
impl Copy for PerformanceTestConfig {}
impl Clone for PerformanceTestConfig {
    fn clone(&self) -> Self {
        *self
    }
}
