//! Service Discovery Module
//!
//! Provides service registration, discovery, and capability matching
//! across the distributed mesh network.

use crate::config::DiscoveryConfiguration;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Service capability information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceCapabilities {
    /// Supported serialization formats
    pub serialization_formats: Vec<SerializationFormat>,

    /// Supported topology patterns
    pub topology_patterns: Vec<TopologyPattern>,

    /// Performance characteristics
    pub performance_profile: PerformanceProfile,

    /// Security requirements
    pub security_level: SecurityLevel,
}

/// Supported serialization formats
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SerializationFormat {
    Json,
    Binary,
    MessagePack,
    Compact,
    Cbor,
    ZeroCopy,
}

/// Topology patterns supported by a service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TopologyPattern {
    OneToOne,
    OneToMany,
    ManyToOne,
    Broadcast,
    PubSub,
    RequestResponse,
}

/// Performance profile of a service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceProfile {
    /// Expected latency in microseconds
    pub expected_latency_us: u64,

    /// Expected throughput in messages per second
    pub expected_throughput_mps: u64,

    /// CPU usage level (0.0 to 1.0)
    pub cpu_usage_level: f64,

    /// Memory usage in MB
    pub memory_usage_mb: u64,

    /// High performance flag
    pub high_performance: bool,
}

/// Security level requirements
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityLevel {
    None,
    Basic,
    Standard,
    High,
    Maximum,
}

/// Service registration information
#[derive(Debug, Clone)]
pub struct ServiceRegistration {
    /// Unique service ID
    pub service_id: Uuid,

    /// Service name
    pub name: String,

    /// Service version
    pub version: String,

    /// Node ID hosting this service
    pub node_id: Uuid,

    /// Service capabilities
    pub capabilities: ServiceCapabilities,

    /// Network endpoints
    pub endpoints: Vec<ServiceEndpoint>,

    /// Service tags for filtering
    pub tags: Vec<String>,

    /// Health check configuration
    pub health_check: Option<HealthCheckConfig>,

    /// Registration timestamp
    pub registered_at: Instant,

    /// Last heartbeat
    pub last_heartbeat: Instant,

    /// TTL for this registration
    pub ttl: Duration,
}

/// Service network endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    /// Endpoint protocol (tcp, udp, unix, shared_memory)
    pub protocol: String,

    /// Endpoint address
    pub address: String,

    /// Port number (if applicable)
    pub port: Option<u16>,

    /// Endpoint-specific metadata
    pub metadata: HashMap<String, String>,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check method
    pub method: HealthCheckMethod,

    /// Check interval
    pub interval: Duration,

    /// Check timeout
    pub timeout: Duration,

    /// Unhealthy threshold
    pub unhealthy_threshold: u32,

    /// Healthy threshold
    pub healthy_threshold: u32,
}

/// Health check methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthCheckMethod {
    /// HTTP/HTTPS health check
    Http { endpoint: String },

    /// TCP connection check
    Tcp { address: String, port: u16 },

    /// Custom command execution
    Command { command: String, args: Vec<String> },

    /// Simple heartbeat
    Heartbeat,
}

/// Service discovery query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceQuery {
    /// Service name pattern (optional)
    pub name_pattern: Option<String>,

    /// Required capabilities
    pub required_capabilities: Option<ServiceCapabilities>,

    /// Service tags to match
    pub tags: Vec<String>,

    /// Security level requirement
    pub min_security_level: Option<SecurityLevel>,

    /// Performance requirements
    pub performance_requirements: Option<PerformanceRequirements>,
}

/// Performance requirements for service discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRequirements {
    /// Maximum acceptable latency in microseconds
    pub max_latency_us: Option<u64>,

    /// Minimum required throughput in messages per second
    pub min_throughput_mps: Option<u64>,

    /// Require high performance flag
    pub require_high_performance: bool,
}

/// Service discovery response
#[derive(Debug, Clone)]
pub struct ServiceDiscoveryResponse {
    /// Matching services
    pub services: Vec<ServiceRegistration>,

    /// Query execution time
    pub query_time_us: u64,

    /// Total services checked
    pub total_services: usize,

    /// Response timestamp
    pub timestamp: Instant,
}

/// Service discovery engine
pub struct ServiceDiscovery {
    /// Configuration
    config: DiscoveryConfiguration,

    /// Local service registry
    services: Arc<RwLock<HashMap<Uuid, ServiceRegistration>>>,

    /// Service name index for fast lookups
    name_index: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,

    /// Tag index for fast filtering
    tag_index: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,

    /// Discovery statistics
    stats: Arc<RwLock<DiscoveryStats>>,
}

/// Discovery statistics
#[derive(Debug, Default)]
pub struct DiscoveryStats {
    /// Total registrations
    pub total_registrations: u64,

    /// Total discoveries
    pub total_discoveries: u64,

    /// Average query time in microseconds
    pub avg_query_time_us: f64,

    /// Active services count
    pub active_services: usize,

    /// Last cleanup time
    pub last_cleanup: Option<Instant>,
}

impl ServiceDiscovery {
    /// Create a new service discovery engine
    pub fn new(config: DiscoveryConfiguration) -> Self {
        Self {
            config,
            services: Arc::new(RwLock::new(HashMap::new())),
            name_index: Arc::new(RwLock::new(HashMap::new())),
            tag_index: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(DiscoveryStats::default())),
        }
    }

    /// Start the service discovery engine
    pub async fn start(&self) -> Result<()> {
        if !self.config.enable_health_checks {
            return Ok(());
        }

        // Start background tasks
        self.start_cleanup_task().await;
        self.start_heartbeat_task().await;

        tracing::info!("Service discovery engine started");
        Ok(())
    }

    /// Register a service
    pub async fn register_service(&self, registration: ServiceRegistration) -> Result<()> {
        let service_id = registration.service_id;
        let service_name = registration.name.clone();
        let service_tags = registration.tags.clone();

        // Add to main registry
        {
            let mut services = self.services.write().await;
            services.insert(service_id, registration);
        }

        // Update name index
        {
            let mut name_index = self.name_index.write().await;
            name_index
                .entry(service_name)
                .or_insert_with(Vec::new)
                .push(service_id);
        }

        // Update tag index
        {
            let mut tag_index = self.tag_index.write().await;
            for tag in service_tags {
                tag_index
                    .entry(tag)
                    .or_insert_with(Vec::new)
                    .push(service_id);
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_registrations += 1;
            stats.active_services = self.services.read().await.len();
        }

        tracing::info!("Service registered: {}", service_id);
        Ok(())
    }

    /// Discover services matching query
    pub async fn discover_services(&self, query: ServiceQuery) -> Result<ServiceDiscoveryResponse> {
        let start_time = Instant::now();
        let mut matching_services = Vec::new();

        let services = self.services.read().await;
        let total_services = services.len();

        // Filter services based on query
        for (_id, service) in services.iter() {
            if self.matches_query(service, &query).await {
                matching_services.push(service.clone());
            }
        }

        let query_time_us = start_time.elapsed().as_micros() as u64;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_discoveries += 1;
            stats.avg_query_time_us = (stats.avg_query_time_us
                * (stats.total_discoveries - 1) as f64
                + query_time_us as f64)
                / stats.total_discoveries as f64;
        }

        Ok(ServiceDiscoveryResponse {
            services: matching_services,
            query_time_us,
            total_services,
            timestamp: Instant::now(),
        })
    }

    /// Update service heartbeat
    pub async fn heartbeat(&self, service_id: Uuid) -> Result<()> {
        let mut services = self.services.write().await;

        if let Some(service) = services.get_mut(&service_id) {
            service.last_heartbeat = Instant::now();
            tracing::debug!("Heartbeat updated for service: {}", service_id);
            Ok(())
        } else {
            Err(anyhow!("Service not found: {}", service_id))
        }
    }

    /// Unregister a service
    pub async fn unregister_service(&self, service_id: Uuid) -> Result<()> {
        let service = {
            let mut services = self.services.write().await;
            services.remove(&service_id)
        };

        if let Some(service) = service {
            // Remove from name index
            {
                let mut name_index = self.name_index.write().await;
                if let Some(ids) = name_index.get_mut(&service.name) {
                    ids.retain(|&id| id != service_id);
                    if ids.is_empty() {
                        name_index.remove(&service.name);
                    }
                }
            }

            // Remove from tag index
            {
                let mut tag_index = self.tag_index.write().await;
                for tag in &service.tags {
                    if let Some(ids) = tag_index.get_mut(tag) {
                        ids.retain(|&id| id != service_id);
                        if ids.is_empty() {
                            tag_index.remove(tag);
                        }
                    }
                }
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.active_services = self.services.read().await.len();
            }

            tracing::info!("Service unregistered: {}", service_id);
            Ok(())
        } else {
            Err(anyhow!("Service not found: {}", service_id))
        }
    }

    /// Get discovery statistics
    pub async fn get_stats(&self) -> DiscoveryStats {
        let stats = self.stats.read().await;
        DiscoveryStats {
            total_registrations: stats.total_registrations,
            total_discoveries: stats.total_discoveries,
            avg_query_time_us: stats.avg_query_time_us,
            active_services: stats.active_services,
            last_cleanup: stats.last_cleanup,
        }
    }

    /// Check if service matches query criteria
    async fn matches_query(&self, service: &ServiceRegistration, query: &ServiceQuery) -> bool {
        // Check name pattern
        if let Some(pattern) = &query.name_pattern {
            if !service.name.contains(pattern) {
                return false;
            }
        }

        // Check required capabilities
        if let Some(required_caps) = &query.required_capabilities {
            if !self.capabilities_match(&service.capabilities, required_caps) {
                return false;
            }
        }

        // Check tags
        if !query.tags.is_empty() {
            let has_all_tags = query.tags.iter().all(|tag| service.tags.contains(tag));
            if !has_all_tags {
                return false;
            }
        }

        // Check security level
        if let Some(min_security) = &query.min_security_level {
            if !self.security_level_meets_requirement(
                &service.capabilities.security_level,
                min_security,
            ) {
                return false;
            }
        }

        // Check performance requirements
        if let Some(perf_req) = &query.performance_requirements {
            if !self
                .performance_meets_requirements(&service.capabilities.performance_profile, perf_req)
            {
                return false;
            }
        }

        // Check if service is still alive (TTL)
        let now = Instant::now();
        if now.duration_since(service.last_heartbeat) > service.ttl {
            return false;
        }

        true
    }

    /// Check if capabilities match requirements
    fn capabilities_match(
        &self,
        service_caps: &ServiceCapabilities,
        required_caps: &ServiceCapabilities,
    ) -> bool {
        // Check serialization formats
        for required_format in &required_caps.serialization_formats {
            if !service_caps.serialization_formats.contains(required_format) {
                return false;
            }
        }

        // Check topology patterns
        for required_pattern in &required_caps.topology_patterns {
            if !service_caps.topology_patterns.contains(required_pattern) {
                return false;
            }
        }

        true
    }

    /// Check if security level meets requirement
    fn security_level_meets_requirement(
        &self,
        service_level: &SecurityLevel,
        required_level: &SecurityLevel,
    ) -> bool {
        use SecurityLevel::*;

        let service_value = match service_level {
            None => 0,
            Basic => 1,
            Standard => 2,
            High => 3,
            Maximum => 4,
        };

        let required_value = match required_level {
            None => 0,
            Basic => 1,
            Standard => 2,
            High => 3,
            Maximum => 4,
        };

        service_value >= required_value
    }

    /// Check if performance meets requirements
    fn performance_meets_requirements(
        &self,
        profile: &PerformanceProfile,
        requirements: &PerformanceRequirements,
    ) -> bool {
        // Check latency requirement
        if let Some(max_latency) = requirements.max_latency_us {
            if profile.expected_latency_us > max_latency {
                return false;
            }
        }

        // Check throughput requirement
        if let Some(min_throughput) = requirements.min_throughput_mps {
            if profile.expected_throughput_mps < min_throughput {
                return false;
            }
        }

        // Check high performance requirement
        if requirements.require_high_performance && !profile.high_performance {
            return false;
        }

        true
    }

    /// Start cleanup task for expired services
    async fn start_cleanup_task(&self) {
        let services = Arc::clone(&self.services);
        let name_index = Arc::clone(&self.name_index);
        let tag_index = Arc::clone(&self.tag_index);
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let now = Instant::now();
                let mut expired_services = Vec::new();

                // Find expired services
                {
                    let services_read = services.read().await;
                    for (id, service) in services_read.iter() {
                        if now.duration_since(service.last_heartbeat) > service.ttl {
                            expired_services.push(*id);
                        }
                    }
                }

                // Remove expired services
                if !expired_services.is_empty() {
                    let mut services_write = services.write().await;
                    let mut name_index_write = name_index.write().await;
                    let mut tag_index_write = tag_index.write().await;

                    for service_id in expired_services {
                        if let Some(service) = services_write.remove(&service_id) {
                            // Remove from name index
                            if let Some(ids) = name_index_write.get_mut(&service.name) {
                                ids.retain(|&id| id != service_id);
                                if ids.is_empty() {
                                    name_index_write.remove(&service.name);
                                }
                            }

                            // Remove from tag index
                            for tag in &service.tags {
                                if let Some(ids) = tag_index_write.get_mut(tag) {
                                    ids.retain(|&id| id != service_id);
                                    if ids.is_empty() {
                                        tag_index_write.remove(tag);
                                    }
                                }
                            }

                            tracing::info!("Expired service removed: {}", service_id);
                        }
                    }

                    // Update statistics
                    {
                        let mut stats_write = stats.write().await;
                        stats_write.active_services = services_write.len();
                        stats_write.last_cleanup = Some(now);
                    }
                }
            }
        });
    }

    /// Start heartbeat monitoring task
    async fn start_heartbeat_task(&self) {
        let services = Arc::clone(&self.services);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                let now = Instant::now();
                let services_read = services.read().await;

                for (id, service) in services_read.iter() {
                    let time_since_heartbeat = now.duration_since(service.last_heartbeat);
                    let ttl_warning_threshold = service.ttl / 2;

                    if time_since_heartbeat > ttl_warning_threshold {
                        tracing::warn!(
                            "Service {} hasn't sent heartbeat in {:?} (TTL: {:?})",
                            id,
                            time_since_heartbeat,
                            service.ttl
                        );
                    }
                }
            }
        });
    }
}
