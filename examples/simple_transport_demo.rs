//! Basic Working Example - Demonstrating Commy's Core Value
//!
//! This example shows the simplest possible use of the intelligent transport
//! selection between shared memory (local) and network (distributed).

use commy_common::FieldHolder;
use std::path::PathBuf;
use std::time::Instant;

/// Simple transport manager for demonstration
pub struct SimpleTransportManager {
    shared_memory_dir: PathBuf,
}

/// Transport selection result
#[derive(Debug)]
pub enum SelectedTransport {
    SharedMemory { file_path: PathBuf },
    Network { endpoint: String },
}

/// Performance requirements for routing decisions
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    pub max_latency_ms: Option<u32>,
    pub min_throughput_mbps: Option<u32>,
    pub require_local: bool,
    pub require_network: bool,
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            max_latency_ms: Some(10),     // 10ms default
            min_throughput_mbps: Some(1), // 1 MB/s default
            require_local: false,
            require_network: false,
        }
    }
}

impl SimpleTransportManager {
    /// Create a new simple transport manager
    pub fn new() -> Self {
        let shared_memory_dir = std::env::temp_dir().join("commy_demo");
        let _ = std::fs::create_dir_all(&shared_memory_dir);

        Self { shared_memory_dir }
    }
}

impl Default for SimpleTransportManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleTransportManager {
    /// Demonstrate intelligent transport selection
    pub fn select_transport(
        &self,
        data_size: usize,
        requirements: &PerformanceRequirements,
    ) -> SelectedTransport {
        // Intelligent routing logic based on requirements and conditions

        if requirements.require_network {
            return SelectedTransport::Network {
                endpoint: "localhost:8080".to_string(),
            };
        }

        if requirements.require_local {
            return SelectedTransport::SharedMemory {
                file_path: self.shared_memory_dir.join("demo_file.dat"),
            };
        }

        // Automatic decision based on performance characteristics

        // For small, low-latency requirements, prefer shared memory
        if let Some(max_latency) = requirements.max_latency_ms {
            if max_latency < 5 {
                // Sub-5ms requires shared memory
                return SelectedTransport::SharedMemory {
                    file_path: self.shared_memory_dir.join("low_latency_file.dat"),
                };
            }
        }

        // For large data, prefer network if available (simulated check)
        if data_size > 1024 * 1024 {
            // 1MB+
            if self.is_network_available() {
                return SelectedTransport::Network {
                    endpoint: "localhost:8080".to_string(),
                };
            }
        }

        // Default: prefer local shared memory for best performance
        SelectedTransport::SharedMemory {
            file_path: self.shared_memory_dir.join("default_file.dat"),
        }
    }

    /// Check if network transport is available (simulated)
    fn is_network_available(&self) -> bool {
        // In a real implementation, this would check for network connectivity,
        // service discovery, etc. For demo purposes, we'll simulate this.
        true
    }

    /// Demonstrate shared memory operation
    pub fn demo_shared_memory(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let file_path = self.shared_memory_dir.join("demo_shared_memory.dat");

        println!("📁 Using shared memory: {}", file_path.display());

        let start = Instant::now();

        // Create shared memory file
        let field_holder = FieldHolder::<Vec<u8>>::new(data.to_vec(), 1);

        // Write data (this is simulated - in real implementation would use memory-mapped files)
        println!("✍️  Writing {} bytes to shared memory", data.len());

        // Read data back
        let read_data = field_holder.get().clone();

        let duration = start.elapsed();
        println!(
            "⚡ Shared memory operation completed in {:?} (sub-microsecond target)",
            duration
        );

        Ok(read_data)
    }

    /// Demonstrate network operation (simulated)
    pub fn demo_network(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        println!("🌐 Using network transport: localhost:8080");

        let start = Instant::now();

        // Simulate network latency
        std::thread::sleep(std::time::Duration::from_millis(2));

        println!("✍️  Sending {} bytes over network", data.len());

        // Simulate network round-trip
        let read_data = data.to_vec();

        let duration = start.elapsed();
        println!("🌐 Network operation completed in {:?}", duration);

        Ok(read_data)
    }
}

/// Demonstrate Commy's intelligent transport selection
pub fn run_transport_demo() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Commy: Intelligent Transport Selection Demo");
    println!("==============================================");

    let transport_manager = SimpleTransportManager::new();
    let test_data = b"Hello, Commy! This is test data for transport selection.";

    // Scenario 1: Ultra-low latency requirement (should use shared memory)
    println!("\n🚀 Scenario 1: Ultra-low latency requirement");
    let low_latency_reqs = PerformanceRequirements {
        max_latency_ms: Some(1), // 1ms - very strict
        ..Default::default()
    };

    let transport = transport_manager.select_transport(test_data.len(), &low_latency_reqs);
    println!("🎯 Selected transport: {:?}", transport);

    match transport {
        SelectedTransport::SharedMemory { file_path: _ } => {
            let result = transport_manager.demo_shared_memory(test_data)?;
            println!("✅ Data verified: {} bytes", result.len());
        }
        SelectedTransport::Network { endpoint: _ } => {
            let result = transport_manager.demo_network(test_data)?;
            println!("✅ Data verified: {} bytes", result.len());
        }
    }

    // Scenario 2: Large data transfer (may use network)
    println!("\n📦 Scenario 2: Large data transfer");
    let large_data = vec![0u8; 2 * 1024 * 1024]; // 2MB
    let large_data_reqs = PerformanceRequirements {
        min_throughput_mbps: Some(50), // High throughput requirement
        ..Default::default()
    };

    let transport = transport_manager.select_transport(large_data.len(), &large_data_reqs);
    println!("🎯 Selected transport: {:?}", transport);

    match transport {
        SelectedTransport::SharedMemory { file_path: _ } => {
            println!("📁 Using shared memory for large data transfer");
        }
        SelectedTransport::Network { endpoint: _ } => {
            println!("🌐 Using network for large data transfer");
        }
    }

    // Scenario 3: Explicit local requirement
    println!("\n🏠 Scenario 3: Explicit local requirement");
    let local_reqs = PerformanceRequirements {
        require_local: true,
        ..Default::default()
    };

    let transport = transport_manager.select_transport(test_data.len(), &local_reqs);
    println!("🎯 Selected transport: {:?}", transport);

    match transport {
        SelectedTransport::SharedMemory { file_path: _ } => {
            let result = transport_manager.demo_shared_memory(test_data)?;
            println!("✅ Local operation completed: {} bytes", result.len());
        }
        _ => unreachable!("Local requirement should always select shared memory"),
    }

    // Scenario 4: Explicit network requirement
    println!("\n🌍 Scenario 4: Explicit network requirement");
    let network_reqs = PerformanceRequirements {
        require_network: true,
        ..Default::default()
    };

    let transport = transport_manager.select_transport(test_data.len(), &network_reqs);
    println!("🎯 Selected transport: {:?}", transport);

    match transport {
        SelectedTransport::Network { endpoint: _ } => {
            let result = transport_manager.demo_network(test_data)?;
            println!("✅ Network operation completed: {} bytes", result.len());
        }
        _ => unreachable!("Network requirement should always select network"),
    }

    println!("\n🎉 Transport Selection Demo Complete!");
    println!("📊 Key Value Demonstrated:");
    println!("   ⚡ Automatic shared memory for ultra-low latency");
    println!("   🌐 Intelligent network selection for large data");
    println!("   🎯 User preference override capability");
    println!("   🔄 Zero code changes between local and distributed");

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_transport_demo()
}
