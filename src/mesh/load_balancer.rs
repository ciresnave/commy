//! Load Balancer Module
//!
//! Provides intelligent load balancing algorithms, health checking,
//! and failover capabilities for the distributed mesh.

use crate::mesh::service_discovery::ServiceRegistration;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Load balancing algorithms
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    PerformanceBased,
    Random,
    ConsistentHash,
}

/// Health status for services
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Unhealthy,
}

/// Circuit breaker states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

/// Service information with load balancing metadata
#[derive(Debug, Clone)]
pub struct LoadBalancedService {
    /// Service registration details
    pub registration: ServiceRegistration,

    /// Current health status
    pub health_status: HealthStatus,

    /// Circuit breaker state
    pub circuit_breaker: CircuitBreakerState,

    /// Current connection count
    pub current_connections: u32,

    /// Average response time
    pub avg_response_time: Duration,

    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,

    /// Service weight for weighted algorithms
    pub weight: u32,

    /// Last health check timestamp
    pub last_health_check: Instant,
}

/// Load balancer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    /// Load balancing algorithm to use
    pub algorithm: LoadBalancingAlgorithm,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Circuit breaker failure threshold
    pub circuit_breaker_threshold: u32,

    /// Circuit breaker timeout
    pub circuit_breaker_timeout: Duration,

    /// Maximum retries for failed requests
    pub max_retries: u32,

    /// Request timeout
    pub request_timeout: Duration,
}

/// Load balance result
#[derive(Debug, Clone)]
pub struct LoadBalanceResult {
    /// Selected service
    pub service: LoadBalancedService,

    /// Selection reason
    pub reason: String,

    /// Selection time in microseconds
    pub selection_time_us: u64,

    /// Alternative services considered
    pub alternatives: Vec<Uuid>,
}

/// Load balancer statistics
#[derive(Debug, Clone, Default)]
pub struct LoadBalancerStats {
    /// Total decisions made
    pub total_decisions: u64,

    /// Healthy services count
    pub healthy_services: usize,

    /// Unhealthy services count
    pub unhealthy_services: usize,

    /// Circuit breakers open count
    pub circuit_breakers_open: usize,

    /// Average decision time in microseconds
    pub avg_decision_time_us: f64,

    /// Requests routed per algorithm
    pub algorithm_usage: HashMap<String, u64>,
}

/// Intelligent load balancer
pub struct LoadBalancer {
    /// Configuration
    config: LoadBalancerConfig,

    /// Services registry
    services: Arc<RwLock<HashMap<Uuid, LoadBalancedService>>>,

    /// Round-robin counter
    round_robin_counter: Arc<RwLock<usize>>,

    /// Statistics
    stats: Arc<RwLock<LoadBalancerStats>>,

    /// Circuit breaker timers
    circuit_breaker_timers: Arc<RwLock<HashMap<Uuid, Instant>>>,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            config,
            services: Arc::new(RwLock::new(HashMap::new())),
            round_robin_counter: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(LoadBalancerStats::default())),
            circuit_breaker_timers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a service with the load balancer
    pub async fn register_service(&self, service: LoadBalancedService) -> Result<()> {
        let service_id = service.registration.service_id;

        {
            let mut services = self.services.write().await;
            services.insert(service_id, service);
        }

        tracing::info!("Service registered with load balancer: {}", service_id);
        Ok(())
    }

    /// Unregister a service from the load balancer
    pub async fn unregister_service(&self, service_id: Uuid) -> Result<()> {
        {
            let mut services = self.services.write().await;
            services.remove(&service_id);
        }

        {
            let mut timers = self.circuit_breaker_timers.write().await;
            timers.remove(&service_id);
        }

        tracing::info!("Service unregistered from load balancer: {}", service_id);
        Ok(())
    }

    /// Update service health status
    pub async fn update_service_health(
        &self,
        service_id: Uuid,
        health_status: HealthStatus,
        metrics: Option<(Duration, f64)>, // response_time, error_rate
    ) -> Result<()> {
        let mut services = self.services.write().await;
        if let Some(service) = services.get_mut(&service_id) {
            service.health_status = health_status.clone();
            service.last_health_check = Instant::now();

            if let Some((response_time, error_rate)) = metrics {
                service.avg_response_time = response_time;
                service.error_rate = error_rate;
            }

            // Update circuit breaker state
            self.update_circuit_breaker(service).await;

            tracing::debug!(
                "Service health updated: {} -> {:?}",
                service_id,
                health_status
            );
        }

        Ok(())
    }

    /// Select a service for load balancing
    pub async fn select_service(&self, request_context: Option<&str>) -> Result<LoadBalanceResult> {
        let start_time = Instant::now();
        let services = self.services.read().await;

        if services.is_empty() {
            return Err(anyhow!("No services registered"));
        }

        // Filter healthy services
        let healthy_services: Vec<_> = services
            .values()
            .filter(|s| {
                s.health_status == HealthStatus::Healthy
                    && s.circuit_breaker == CircuitBreakerState::Closed
            })
            .collect();

        let healthy_count = healthy_services.len();

        let available_services = if !healthy_services.is_empty() {
            healthy_services
        } else {
            // Fall back to warning status services if no healthy ones
            let warning_services: Vec<_> = services
                .values()
                .filter(|s| {
                    s.health_status == HealthStatus::Warning
                        && s.circuit_breaker != CircuitBreakerState::Open
                })
                .collect();

            if warning_services.is_empty() {
                return Err(anyhow!("No healthy services available"));
            }
            warning_services
        };

        // Select service based on algorithm
        let selected_service = match self.config.algorithm {
            LoadBalancingAlgorithm::RoundRobin => {
                self.select_round_robin(&available_services).await?
            }
            LoadBalancingAlgorithm::LeastConnections => {
                self.select_least_connections(&available_services).await?
            }
            LoadBalancingAlgorithm::WeightedRoundRobin => {
                self.select_weighted_round_robin(&available_services)
                    .await?
            }
            LoadBalancingAlgorithm::PerformanceBased => {
                self.select_performance_based(&available_services).await?
            }
            LoadBalancingAlgorithm::Random => self.select_random(&available_services).await?,
            LoadBalancingAlgorithm::ConsistentHash => {
                self.select_consistent_hash(&available_services, request_context)
                    .await?
            }
        };

        let selection_time_us = start_time.elapsed().as_micros() as u64;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_decisions += 1;
            stats.avg_decision_time_us = (stats.avg_decision_time_us
                * (stats.total_decisions - 1) as f64
                + selection_time_us as f64)
                / stats.total_decisions as f64;
            stats.healthy_services = healthy_count;
            stats.unhealthy_services = services.len() - healthy_count;
            stats.circuit_breakers_open = services
                .values()
                .filter(|s| s.circuit_breaker == CircuitBreakerState::Open)
                .count();
        }

        Ok(LoadBalanceResult {
            service: selected_service.clone(),
            reason: format!("{:?} algorithm", self.config.algorithm),
            selection_time_us,
            alternatives: available_services
                .iter()
                .filter(|s| s.registration.service_id != selected_service.registration.service_id)
                .map(|s| s.registration.service_id)
                .collect(),
        })
    }

    /// Get load balancer statistics
    pub async fn get_stats(&self) -> LoadBalancerStats {
        let stats = self.stats.read().await;
        LoadBalancerStats {
            total_decisions: stats.total_decisions,
            healthy_services: stats.healthy_services,
            unhealthy_services: stats.unhealthy_services,
            circuit_breakers_open: stats.circuit_breakers_open,
            avg_decision_time_us: stats.avg_decision_time_us,
            algorithm_usage: stats.algorithm_usage.clone(),
        }
    }

    /// Update circuit breaker state for a service
    async fn update_circuit_breaker(&self, service: &mut LoadBalancedService) {
        match service.circuit_breaker {
            CircuitBreakerState::Closed => {
                if service.error_rate > 0.5 {
                    // More than 50% error rate, open circuit breaker
                    service.circuit_breaker = CircuitBreakerState::Open;
                    let mut timers = self.circuit_breaker_timers.write().await;
                    timers.insert(service.registration.service_id, Instant::now());
                    tracing::warn!(
                        "Circuit breaker opened for service: {}",
                        service.registration.service_id
                    );
                }
            }
            CircuitBreakerState::Open => {
                let timers = self.circuit_breaker_timers.read().await;
                if let Some(open_time) = timers.get(&service.registration.service_id) {
                    if open_time.elapsed() > self.config.circuit_breaker_timeout {
                        service.circuit_breaker = CircuitBreakerState::HalfOpen;
                        tracing::info!(
                            "Circuit breaker half-opened for service: {}",
                            service.registration.service_id
                        );
                    }
                }
            }
            CircuitBreakerState::HalfOpen => {
                if service.error_rate < 0.1 {
                    // Less than 10% error rate, close circuit breaker
                    service.circuit_breaker = CircuitBreakerState::Closed;
                    let mut timers = self.circuit_breaker_timers.write().await;
                    timers.remove(&service.registration.service_id);
                    tracing::info!(
                        "Circuit breaker closed for service: {}",
                        service.registration.service_id
                    );
                } else if service.error_rate > 0.3 {
                    // More than 30% error rate, reopen circuit breaker
                    service.circuit_breaker = CircuitBreakerState::Open;
                    let mut timers = self.circuit_breaker_timers.write().await;
                    timers.insert(service.registration.service_id, Instant::now());
                    tracing::warn!(
                        "Circuit breaker reopened for service: {}",
                        service.registration.service_id
                    );
                }
            }
        }
    }

    // Load balancing algorithm implementations

    async fn select_round_robin<'a>(
        &self,
        services: &'a [&'a LoadBalancedService],
    ) -> Result<&'a LoadBalancedService> {
        if services.is_empty() {
            return Err(anyhow!("No services available"));
        }

        let mut counter = self.round_robin_counter.write().await;
        let index = *counter % services.len();
        *counter += 1;

        Ok(services[index])
    }

    async fn select_least_connections<'a>(
        &self,
        services: &'a [&'a LoadBalancedService],
    ) -> Result<&'a LoadBalancedService> {
        if services.is_empty() {
            return Err(anyhow!("No services available"));
        }

        let selected = services
            .iter()
            .min_by_key(|s| s.current_connections)
            .unwrap();

        Ok(selected)
    }

    async fn select_weighted_round_robin<'a>(
        &self,
        services: &'a [&'a LoadBalancedService],
    ) -> Result<&'a LoadBalancedService> {
        if services.is_empty() {
            return Err(anyhow!("No services available"));
        }

        let total_weight: u32 = services.iter().map(|s| s.weight).sum();
        if total_weight == 0 {
            return self.select_round_robin(services).await;
        }

        let mut counter = self.round_robin_counter.write().await;
        let target = *counter % total_weight as usize;
        *counter += 1;

        let mut current_weight = 0u32;
        for service in services {
            current_weight += service.weight;
            if current_weight > target as u32 {
                return Ok(service);
            }
        }

        Ok(services[0]) // Fallback
    }

    async fn select_performance_based<'a>(
        &self,
        services: &'a [&'a LoadBalancedService],
    ) -> Result<&'a LoadBalancedService> {
        if services.is_empty() {
            return Err(anyhow!("No services available"));
        }

        // Score based on response time and error rate
        let selected = services
            .iter()
            .min_by(|a, b| {
                let score_a = a.avg_response_time.as_millis() as f64 + (a.error_rate * 1000.0);
                let score_b = b.avg_response_time.as_millis() as f64 + (b.error_rate * 1000.0);
                score_a.partial_cmp(&score_b).unwrap()
            })
            .unwrap();

        Ok(selected)
    }

    async fn select_random<'a>(
        &self,
        services: &'a [&'a LoadBalancedService],
    ) -> Result<&'a LoadBalancedService> {
        if services.is_empty() {
            return Err(anyhow!("No services available"));
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        Instant::now().hash(&mut hasher);
        let hash = hasher.finish();
        let index = (hash as usize) % services.len();

        Ok(services[index])
    }

    async fn select_consistent_hash<'a>(
        &self,
        services: &'a [&'a LoadBalancedService],
        context: Option<&str>,
    ) -> Result<&'a LoadBalancedService> {
        if services.is_empty() {
            return Err(anyhow!("No services available"));
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        if let Some(ctx) = context {
            ctx.hash(&mut hasher);
        } else {
            "default".hash(&mut hasher);
        }
        let hash = hasher.finish();
        let index = (hash as usize) % services.len();

        Ok(services[index])
    }

    /// Add service method (alias for register_service)
    pub async fn add_service(&self, registration: ServiceRegistration) -> Result<()> {
        let service = LoadBalancedService {
            registration,
            health_status: HealthStatus::Healthy,
            circuit_breaker: CircuitBreakerState::Closed,
            current_connections: 0,
            avg_response_time: Duration::from_millis(100),
            error_rate: 0.0,
            weight: 1,
            last_health_check: Instant::now(),
        };

        self.register_service(service).await
    }

    /// Remove service method (alias for unregister_service)
    pub async fn remove_service(&self, service_id: Uuid) -> Result<()> {
        self.unregister_service(service_id).await
    }

    /// Start method for compatibility
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Load balancer started");
        Ok(())
    }
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            algorithm: LoadBalancingAlgorithm::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(60),
            max_retries: 3,
            request_timeout: Duration::from_secs(30),
        }
    }
}
