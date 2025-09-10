//! Health Monitor Module
//!
//! Provides comprehensive health monitoring, metrics collection,
//! and alerting for services in the mesh.

use crate::mesh::service_discovery::ServiceRegistration;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Service ID
    pub service_id: Uuid,

    /// Check timestamp
    pub timestamp: Instant,

    /// Check success
    pub success: bool,

    /// Response time in microseconds
    pub response_time_us: u64,

    /// Health score (0.0 to 1.0)
    pub health_score: f64,

    /// Error message if failed
    pub error_message: Option<String>,

    /// Additional metrics
    pub metrics: HashMap<String, f64>,
}

/// Health status over time
#[derive(Debug, Clone)]
pub struct HealthHistory {
    /// Service ID
    pub service_id: Uuid,

    /// Recent health checks (last 100)
    pub recent_checks: Vec<HealthCheckResult>,

    /// Health trends
    pub trends: HealthTrends,

    /// Overall health score
    pub overall_health_score: f64,

    /// Uptime percentage
    pub uptime_percentage: f64,
}

/// Health trends analysis
#[derive(Debug, Clone)]
pub struct HealthTrends {
    /// Average response time in microseconds
    pub avg_response_time_us: f64,

    /// Response time trend (positive = improving)
    pub response_time_trend: f64,

    /// Success rate over last hour
    pub success_rate_1h: f64,

    /// Success rate over last day
    pub success_rate_24h: f64,

    /// Error frequency (errors per hour)
    pub error_frequency: f64,
}

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,

    /// Memory usage in MB
    pub memory_usage_mb: u64,

    /// Network I/O in bytes per second
    pub network_io_bps: u64,

    /// Disk I/O in bytes per second
    pub disk_io_bps: u64,

    /// Active connections count
    pub active_connections: u64,

    /// Request queue length
    pub queue_length: u32,

    /// Custom metrics
    pub custom_metrics: HashMap<String, f64>,
}

/// Alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Alert name
    pub name: String,

    /// Metric to monitor
    pub metric: String,

    /// Alert condition
    pub condition: AlertCondition,

    /// Threshold value
    pub threshold: f64,

    /// Alert severity
    pub severity: AlertSeverity,

    /// Cooldown period
    pub cooldown: Duration,
}

/// Alert conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    EqualTo,
    NotEqualTo,
    PercentageChange,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Triggered alert
#[derive(Debug, Clone)]
pub struct Alert {
    /// Alert ID
    pub id: Uuid,

    /// Service ID
    pub service_id: Uuid,

    /// Alert configuration
    pub config: AlertConfig,

    /// Current metric value
    pub current_value: f64,

    /// Trigger timestamp
    pub triggered_at: Instant,

    /// Alert message
    pub message: String,

    /// Alert resolved
    pub resolved: bool,

    /// Resolution timestamp
    pub resolved_at: Option<Instant>,
}

/// Health monitor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitorConfig {
    /// Health check interval
    pub check_interval: Duration,

    /// Health check timeout
    pub check_timeout: Duration,

    /// Performance metrics collection interval
    pub metrics_interval: Duration,

    /// History retention period
    pub history_retention: Duration,

    /// Alert configurations
    pub alerts: Vec<AlertConfig>,

    /// Enable detailed metrics
    pub detailed_metrics: bool,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            check_timeout: Duration::from_secs(5),
            metrics_interval: Duration::from_secs(30),
            history_retention: Duration::from_secs(3600), // 1 hour
            alerts: Vec::new(),
            detailed_metrics: true,
        }
    }
}

/// Health monitor statistics
#[derive(Debug, Clone, Default)]
pub struct HealthMonitorStats {
    /// Total health checks performed
    pub total_health_checks: u64,

    /// Total services monitored
    pub services_monitored: usize,

    /// Average check time in microseconds
    pub avg_check_time_us: f64,

    /// Active alerts count
    pub active_alerts: usize,

    /// Total alerts triggered
    pub total_alerts: u64,

    /// System uptime
    pub uptime: Duration,

    /// Last metrics collection
    pub last_metrics_collection: Option<Instant>,
}

/// Comprehensive health monitor
pub struct HealthMonitor {
    /// Configuration
    config: HealthMonitorConfig,

    /// Service health histories
    health_histories: Arc<RwLock<HashMap<Uuid, HealthHistory>>>,

    /// Performance metrics
    performance_metrics: Arc<RwLock<HashMap<Uuid, PerformanceMetrics>>>,

    /// Active alerts
    active_alerts: Arc<RwLock<HashMap<Uuid, Alert>>>,

    /// Monitor statistics
    stats: Arc<RwLock<HealthMonitorStats>>,

    /// Start time
    start_time: Instant,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(config: HealthMonitorConfig) -> Self {
        Self {
            config,
            health_histories: Arc::new(RwLock::new(HashMap::new())),
            performance_metrics: Arc::new(RwLock::new(HashMap::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HealthMonitorStats::default())),
            start_time: Instant::now(),
        }
    }

    /// Start the health monitor
    pub async fn start(&self) -> Result<()> {
        self.start_health_check_task().await;
        self.start_metrics_collection_task().await;
        self.start_alert_evaluation_task().await;
        self.start_cleanup_task().await;

        tracing::info!("Health monitor started");
        Ok(())
    }

    /// Add service to monitoring
    pub async fn add_service(&self, service: ServiceRegistration) -> Result<()> {
        let service_id = service.service_id;

        let health_history = HealthHistory {
            service_id,
            recent_checks: Vec::new(),
            trends: HealthTrends {
                avg_response_time_us: 0.0,
                response_time_trend: 0.0,
                success_rate_1h: 1.0,
                success_rate_24h: 1.0,
                error_frequency: 0.0,
            },
            overall_health_score: 1.0,
            uptime_percentage: 100.0,
        };

        {
            let mut histories = self.health_histories.write().await;
            histories.insert(service_id, health_history);
        }

        {
            let mut stats = self.stats.write().await;
            stats.services_monitored = self.health_histories.read().await.len();
        }

        tracing::info!("Service added to health monitoring: {}", service_id);
        Ok(())
    }

    /// Remove service from monitoring
    pub async fn remove_service(&self, service_id: Uuid) -> Result<()> {
        {
            let mut histories = self.health_histories.write().await;
            histories.remove(&service_id);
        }

        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.remove(&service_id);
        }

        {
            let mut stats = self.stats.write().await;
            stats.services_monitored = self.health_histories.read().await.len();
        }

        tracing::info!("Service removed from health monitoring: {}", service_id);
        Ok(())
    }

    /// Perform health check on service
    pub async fn check_service_health(&self, service_id: Uuid) -> Result<HealthCheckResult> {
        let start_time = Instant::now();

        // In a real implementation, this would perform actual health checks
        // For now, we'll simulate a health check
        let success = true; // Simulate success
        let response_time_us = start_time.elapsed().as_micros() as u64;
        let health_score = if success { 1.0 } else { 0.0 };

        let result = HealthCheckResult {
            service_id,
            timestamp: Instant::now(),
            success,
            response_time_us,
            health_score,
            error_message: None,
            metrics: HashMap::new(),
        };

        // Update health history
        {
            let mut histories = self.health_histories.write().await;
            if let Some(history) = histories.get_mut(&service_id) {
                history.recent_checks.push(result.clone());

                // Keep only last 100 checks
                if history.recent_checks.len() > 100 {
                    history.recent_checks.remove(0);
                }

                // Update trends
                self.update_health_trends(history).await;
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_health_checks += 1;
            stats.avg_check_time_us = (stats.avg_check_time_us
                * (stats.total_health_checks - 1) as f64
                + response_time_us as f64)
                / stats.total_health_checks as f64;
            stats.uptime = self.start_time.elapsed();
        }

        Ok(result)
    }

    /// Collect performance metrics for service
    pub async fn collect_performance_metrics(
        &self,
        service_id: Uuid,
    ) -> Result<PerformanceMetrics> {
        // In a real implementation, this would collect actual system metrics
        // For now, we'll simulate metrics
        let metrics = PerformanceMetrics {
            cpu_usage: 25.5,
            memory_usage_mb: 128,
            network_io_bps: 1024 * 1024, // 1MB/s
            disk_io_bps: 512 * 1024,     // 512KB/s
            active_connections: 42,
            queue_length: 5,
            custom_metrics: HashMap::new(),
        };

        {
            let mut perf_metrics = self.performance_metrics.write().await;
            perf_metrics.insert(service_id, metrics.clone());
        }

        {
            let mut stats = self.stats.write().await;
            stats.last_metrics_collection = Some(Instant::now());
        }

        Ok(metrics)
    }

    /// Get health history for service
    pub async fn get_health_history(&self, service_id: Uuid) -> Option<HealthHistory> {
        let histories = self.health_histories.read().await;
        histories.get(&service_id).cloned()
    }

    /// Get performance metrics for service
    pub async fn get_performance_metrics(&self, service_id: Uuid) -> Option<PerformanceMetrics> {
        let metrics = self.performance_metrics.read().await;
        metrics.get(&service_id).cloned()
    }

    /// Get all active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.active_alerts.read().await;
        alerts.values().cloned().collect()
    }

    /// Get monitor statistics
    pub async fn get_stats(&self) -> HealthMonitorStats {
        let mut stats = self.stats.read().await.clone();
        stats.uptime = self.start_time.elapsed();
        stats.active_alerts = self.active_alerts.read().await.len();
        stats
    }

    /// Update health trends for a service
    async fn update_health_trends(&self, history: &mut HealthHistory) {
        if history.recent_checks.is_empty() {
            return;
        }

        let now = Instant::now();
        let one_hour_ago = now - Duration::from_secs(3600);
        let twenty_four_hours_ago = now - Duration::from_secs(86400);

        // Calculate average response time
        let total_response_time: u64 = history
            .recent_checks
            .iter()
            .map(|c| c.response_time_us)
            .sum();
        history.trends.avg_response_time_us =
            total_response_time as f64 / history.recent_checks.len() as f64;

        // Calculate success rates
        let checks_1h: Vec<_> = history
            .recent_checks
            .iter()
            .filter(|c| c.timestamp > one_hour_ago)
            .collect();
        let checks_24h: Vec<_> = history
            .recent_checks
            .iter()
            .filter(|c| c.timestamp > twenty_four_hours_ago)
            .collect();

        if !checks_1h.is_empty() {
            let successful_1h = checks_1h.iter().filter(|c| c.success).count();
            history.trends.success_rate_1h = successful_1h as f64 / checks_1h.len() as f64;
        }

        if !checks_24h.is_empty() {
            let successful_24h = checks_24h.iter().filter(|c| c.success).count();
            history.trends.success_rate_24h = successful_24h as f64 / checks_24h.len() as f64;
        }

        // Calculate error frequency
        let errors_1h = checks_1h.iter().filter(|c| !c.success).count();
        history.trends.error_frequency = errors_1h as f64;

        // Calculate overall health score
        history.overall_health_score =
            (history.trends.success_rate_1h + history.trends.success_rate_24h) / 2.0;

        // Calculate uptime percentage
        let successful_checks = history.recent_checks.iter().filter(|c| c.success).count();
        history.uptime_percentage =
            (successful_checks as f64 / history.recent_checks.len() as f64) * 100.0;
    }

    /// Start health check background task
    async fn start_health_check_task(&self) {
        let health_histories = Arc::clone(&self.health_histories);
        let check_interval = self.config.check_interval;
        let stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);

            loop {
                interval.tick().await;

                let service_ids: Vec<Uuid> = {
                    let histories = health_histories.read().await;
                    histories.keys().cloned().collect()
                };

                for service_id in service_ids {
                    // Simplified health check for spawned task
                    let result = HealthCheckResult {
                        service_id,
                        success: true,
                        response_time_us: 10_000, // 10ms
                        health_score: 0.95,
                        timestamp: Instant::now(),
                        metrics: HashMap::new(),
                        error_message: None,
                    };

                    // Update history
                    {
                        let mut histories = health_histories.write().await;
                        if let Some(history) = histories.get_mut(&service_id) {
                            history.recent_checks.push(result);
                            if history.recent_checks.len() > 100 {
                                history.recent_checks.remove(0);
                            }
                        }
                    }

                    // Update stats
                    {
                        let mut stats_write = stats.write().await;
                        stats_write.total_health_checks += 1;
                    }
                }
            }
        });
    }

    /// Start metrics collection background task
    async fn start_metrics_collection_task(&self) {
        let health_histories = Arc::clone(&self.health_histories);
        let metrics_interval = self.config.metrics_interval;
        let _stats = Arc::clone(&self.stats);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(metrics_interval);

            loop {
                interval.tick().await;

                let service_ids: Vec<Uuid> = {
                    let histories = health_histories.read().await;
                    histories.keys().cloned().collect()
                };

                for service_id in service_ids {
                    // Simplified metrics collection for spawned task
                    let mut custom_metrics = HashMap::new();
                    custom_metrics.insert("cpu_usage".to_string(), 45.0);
                    custom_metrics.insert("memory_usage_mb".to_string(), 120.0);

                    // Update history with metrics
                    {
                        let mut histories = health_histories.write().await;
                        if let Some(history) = histories.get_mut(&service_id) {
                            if let Some(last_check) = history.recent_checks.last_mut() {
                                last_check.metrics = custom_metrics;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Start alert evaluation background task
    async fn start_alert_evaluation_task(&self) {
        let config_alerts = self.config.alerts.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;
                // Simplified alert evaluation for spawned task
                tracing::debug!("Evaluating alerts - {} configured", config_alerts.len());
            }
        });
    }

    /// Start cleanup background task
    async fn start_cleanup_task(&self) {
        let health_histories = Arc::clone(&self.health_histories);
        let active_alerts = Arc::clone(&self.active_alerts);
        let retention = self.config.history_retention;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                let now = Instant::now();
                let cutoff = now - retention;

                // Clean up old health check results
                {
                    let mut histories = health_histories.write().await;
                    for history in histories.values_mut() {
                        history
                            .recent_checks
                            .retain(|check| check.timestamp > cutoff);
                    }
                }

                // Clean up resolved alerts older than 1 hour
                {
                    let mut alerts = active_alerts.write().await;
                    let alert_cutoff = now - Duration::from_secs(3600);
                    alerts.retain(|_, alert| {
                        !alert.resolved
                            || alert.resolved_at.map(|t| t > alert_cutoff).unwrap_or(true)
                    });
                }
            }
        });
    }
}
