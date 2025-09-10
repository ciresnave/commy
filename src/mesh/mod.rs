//! Mesh Capabilities Module - Phase 2 Implementation
//!
//! This module provides service discovery, load balancing, health monitoring,
//! and mesh coordination capabilities that work together with the Phase 1
//! foundation layer (SharedFileManager, coordination, lifecycle, id_manager).

pub mod health_monitor;
pub mod load_balancer;
pub mod mesh_coordinator;
pub mod node_registry;
pub mod service_discovery;

#[cfg(test)]
mod tests;

// Re-export key types for easy access
pub use health_monitor::{HealthMonitor, HealthMonitorConfig};
pub use load_balancer::{LoadBalancer, LoadBalancerConfig, LoadBalancingAlgorithm};
pub use mesh_coordinator::{MeshCoordinator, MeshCoordinatorConfig, MeshNode, NodeStatus};
pub use node_registry::{NodeRegistry, NodeRegistryConfig};
pub use service_discovery::{
    ServiceCapabilities, ServiceDiscovery, ServiceEndpoint, ServiceQuery, ServiceRegistration,
};

use crate::manager::core::{ManagerConfig, SharedFileManager};
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

/// Phase 2 Mesh Manager - High-level orchestrator for mesh capabilities
///
/// This coordinates between the Phase 1 foundation (SharedFileManager) and
/// Phase 2 mesh capabilities (service discovery, load balancing, health monitoring)
pub struct MeshManager {
    /// Phase 1 foundation - core file management
    pub file_manager: Arc<SharedFileManager>,

    /// Phase 2 mesh coordinator - service discovery and coordination
    pub mesh_coordinator: Arc<MeshCoordinator>,

    /// Configuration
    pub config: MeshManagerConfig,
}

/// Configuration for the mesh manager
#[derive(Debug, Clone)]
pub struct MeshManagerConfig {
    /// Node identification
    pub node_id: Uuid,
    pub node_name: String,

    /// Manager configuration for Phase 1 foundation
    pub manager_config: ManagerConfig,

    /// Mesh coordinator configuration for Phase 2
    pub mesh_coordinator_config: MeshCoordinatorConfig,
}

impl MeshManager {
    /// Create a new mesh manager with Phase 1 and Phase 2 capabilities
    pub async fn new(config: MeshManagerConfig) -> Result<Self> {
        // Initialize Phase 1 foundation
        let file_manager = Arc::new(SharedFileManager::new(config.manager_config.clone()).await?);

        // Initialize Phase 2 mesh capabilities
        let mesh_coordinator =
            Arc::new(MeshCoordinator::new(config.mesh_coordinator_config.clone()).await?);

        Ok(Self {
            file_manager,
            mesh_coordinator,
            config,
        })
    }

    /// Start the mesh manager - this starts both Phase 1 and Phase 2 capabilities
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!(
            "Starting Commy Mesh Manager for node: {}",
            self.config.node_name
        );

        // Start Phase 1 foundation
        let file_manager = Arc::get_mut(&mut self.file_manager).unwrap();
        file_manager.start().await?;

        // Start Phase 2 mesh capabilities
        self.mesh_coordinator.start().await?;

        // Register this node's file management service
        self.register_file_service().await?;

        tracing::info!("Mesh manager started successfully");
        Ok(())
    }

    /// Register the file management service in the mesh
    async fn register_file_service(&self) -> Result<()> {
        let service_registration = ServiceRegistration {
            service_id: Uuid::new_v4(),
            name: "commy-file-manager".to_string(),
            version: "1.0.0".to_string(),
            node_id: self.config.node_id,
            capabilities: ServiceCapabilities {
                serialization_formats: vec![
                    service_discovery::SerializationFormat::Json,
                    service_discovery::SerializationFormat::Binary,
                    service_discovery::SerializationFormat::MessagePack,
                    service_discovery::SerializationFormat::Cbor,
                    service_discovery::SerializationFormat::ZeroCopy,
                    service_discovery::SerializationFormat::Compact,
                ],
                topology_patterns: vec![
                    service_discovery::TopologyPattern::OneToOne,
                    service_discovery::TopologyPattern::OneToMany,
                    service_discovery::TopologyPattern::ManyToOne,
                    service_discovery::TopologyPattern::Broadcast,
                    service_discovery::TopologyPattern::PubSub,
                    service_discovery::TopologyPattern::RequestResponse,
                ],
                performance_profile: service_discovery::PerformanceProfile {
                    expected_latency_us: 10,            // Ultra-low latency target
                    expected_throughput_mps: 1_000_000, // 1M messages/second target
                    cpu_usage_level: 0.1,               // Low CPU usage
                    memory_usage_mb: 100,               // 100MB memory usage
                    high_performance: true,
                },
                security_level: service_discovery::SecurityLevel::High,
            },
            endpoints: vec![ServiceEndpoint {
                protocol: "tcp".to_string(),
                address: self.config.manager_config.bind_address.clone(),
                port: Some(self.config.manager_config.listen_port),
                metadata: std::collections::HashMap::new(),
            }],
            tags: vec![
                "file-manager".to_string(),
                "shared-memory".to_string(),
                "ipc".to_string(),
                "commy".to_string(),
            ],
            health_check: Some(service_discovery::HealthCheckConfig {
                method: service_discovery::HealthCheckMethod::Tcp {
                    address: self.config.manager_config.bind_address.clone(),
                    port: self.config.manager_config.listen_port,
                },
                interval: std::time::Duration::from_secs(10),
                timeout: std::time::Duration::from_secs(5),
                unhealthy_threshold: 3,
                healthy_threshold: 2,
            }),
            registered_at: tokio::time::Instant::now(),
            last_heartbeat: tokio::time::Instant::now(),
            ttl: std::time::Duration::from_secs(300), // 5 minutes TTL
        };

        self.mesh_coordinator
            .register_service(service_registration)
            .await?;

        tracing::info!("File management service registered in mesh");
        Ok(())
    }

    /// Discover file management services in the mesh
    pub async fn discover_file_services(&self) -> Result<Vec<ServiceRegistration>> {
        let query = ServiceQuery {
            name_pattern: Some("commy-file-manager".to_string()),
            required_capabilities: None,
            tags: vec!["file-manager".to_string()],
            min_security_level: Some(service_discovery::SecurityLevel::Standard),
            performance_requirements: None,
        };

        let services = self.mesh_coordinator.discover_services(query).await?;
        Ok(services)
    }

    /// Get mesh statistics
    pub async fn get_mesh_stats(&self) -> Result<MeshStatistics> {
        let coordinator_stats = self.mesh_coordinator.get_stats().await;

        Ok(MeshStatistics {
            active_services: 0,   // TODO: Add service discovery stats access
            total_discoveries: 0, // TODO: Add service discovery stats access
            total_nodes: coordinator_stats.total_nodes,
            avg_query_time_us: 0.0, // TODO: Add service discovery stats access
            mesh_uptime: coordinator_stats.uptime,
        })
    }
}

/// Mesh statistics for monitoring
#[derive(Debug, Clone)]
pub struct MeshStatistics {
    pub active_services: usize,
    pub total_discoveries: u64,
    pub total_nodes: usize,
    pub avg_query_time_us: f64,
    pub mesh_uptime: std::time::Duration,
}
