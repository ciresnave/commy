//! Transport Manager Implementation
//!
//! Provides the core logic for intelligent transport selection and routing.

use super::transport::*;
use super::{
    shared_memory::SharedMemoryTransport, FallbackBehavior, PerformanceProfile, RoutingDecision,
    RoutingReason, SelectedTransport, SharedFileOperation, SharedFileOperationResponse,
    SharedFileRequest, TransportPreference,
};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

impl TransportManager {
    /// Create a new transport manager with the given configuration
    pub async fn new(config: TransportConfig) -> Result<Self, TransportError> {
        let local_transport =
            SharedMemoryTransport::new(config.shared_memory_config.clone()).await?;
        let network_transport = NetworkTransport::new(config.network_config.clone()).await?;
        let performance_monitor = PerformanceMonitor::new(config.performance_thresholds.clone());
        let config = Arc::new(config);

        Ok(Self {
            local_transport,
            network_transport,
            performance_monitor,
            config,
        })
    }

    /// Make a routing decision based on the request and current performance
    pub async fn route_request(
        &self,
        request: &SharedFileRequest,
    ) -> Result<RoutingDecision, TransportError> {
        let current_performance = self.get_performance_monitor().get_current_snapshot().await;

        let decision = match &request.transport_preference {
            TransportPreference::RequireLocal => RoutingDecision {
                transport: SelectedTransport::SharedMemory,
                reason: RoutingReason::UserPreference,
                confidence: 1.0,
                expected_performance: self.estimate_local_performance(request).await,
            },

            TransportPreference::RequireNetwork => RoutingDecision {
                transport: SelectedTransport::Network,
                reason: RoutingReason::UserPreference,
                confidence: 1.0,
                expected_performance: self.estimate_network_performance(request).await,
            },

            TransportPreference::LocalOnly => RoutingDecision {
                transport: SelectedTransport::SharedMemory,
                reason: RoutingReason::UserPreference,
                confidence: 1.0,
                expected_performance: self.estimate_local_performance(request).await,
            },

            TransportPreference::NetworkOnly => RoutingDecision {
                transport: SelectedTransport::Network,
                reason: RoutingReason::UserPreference,
                confidence: 1.0,
                expected_performance: self.estimate_network_performance(request).await,
            },

            TransportPreference::PreferLocal => {
                self.decide_with_preference(request, &current_performance, true)
                    .await?
            }

            TransportPreference::PreferNetwork => {
                self.decide_with_preference(request, &current_performance, false)
                    .await?
            }

            TransportPreference::AutoOptimize => {
                self.optimize_transport_selection(request, &current_performance)
                    .await?
            }

            TransportPreference::Adaptive => {
                self.adaptive_transport_selection(request, &current_performance)
                    .await?
            }
        };

        Ok(decision)
    }

    /// Execute a request using the selected transport
    pub async fn execute_request(
        &self,
        request: SharedFileRequest,
        routing_decision: &RoutingDecision,
    ) -> Result<SharedFileOperationResponse, TransportError> {
        let start_time = Instant::now();
        let request_ref = request.clone(); // Keep a copy for metrics recording

        let result = match &routing_decision.transport {
            SelectedTransport::SharedMemory => self.execute_local_request(request.clone()).await,

            SelectedTransport::Network => self.execute_network_request(request.clone()).await,

            SelectedTransport::Hybrid { primary, fallback } => {
                // Try primary transport first
                let primary_result = match primary.as_ref() {
                    SelectedTransport::SharedMemory => {
                        self.execute_local_request(request.clone()).await
                    }
                    SelectedTransport::Network => {
                        self.execute_network_request(request.clone()).await
                    }
                    _ => {
                        return Err(TransportError::InvalidConfiguration(
                            "Nested hybrid transport not supported".to_string(),
                        ))
                    }
                };

                // Fallback if primary fails
                if primary_result.is_err()
                    && self.get_config().fallback_behavior != FallbackBehavior::Fail
                {
                    match fallback.as_ref() {
                        SelectedTransport::SharedMemory => {
                            self.execute_local_request(request).await
                        }
                        SelectedTransport::Network => self.execute_network_request(request).await,
                        _ => primary_result,
                    }
                } else {
                    primary_result
                }
            }
        };

        // Record performance metrics
        let latency = start_time.elapsed();
        self.record_performance_sample(
            &routing_decision.transport,
            latency,
            result.is_ok(),
            &request_ref,
        )
        .await;

        result
    }

    /// Decide transport with a preference (local or network)
    async fn decide_with_preference(
        &self,
        request: &SharedFileRequest,
        _performance: &PerformanceSnapshot,
        prefer_local: bool,
    ) -> Result<RoutingDecision, TransportError> {
        let local_perf = self.estimate_local_performance(request).await;
        let network_perf = self.estimate_network_performance(request).await;

        // Check if preferred transport meets requirements
        let preferred_transport = if prefer_local {
            SelectedTransport::SharedMemory
        } else {
            SelectedTransport::Network
        };

        let preferred_performance = if prefer_local {
            &local_perf
        } else {
            &network_perf
        };

        if self.meets_performance_requirements(request, preferred_performance) {
            Ok(RoutingDecision {
                transport: preferred_transport,
                reason: RoutingReason::UserPreference,
                confidence: 0.8,
                expected_performance: preferred_performance.clone(),
            })
        } else {
            // Fallback to other transport
            let fallback_transport = if prefer_local {
                SelectedTransport::Network
            } else {
                SelectedTransport::SharedMemory
            };

            let fallback_performance = if prefer_local {
                &network_perf
            } else {
                &local_perf
            };

            Ok(RoutingDecision {
                transport: fallback_transport,
                reason: RoutingReason::Fallback {
                    original_failure: "Preferred transport doesn't meet performance requirements"
                        .to_string(),
                },
                confidence: 0.6,
                expected_performance: fallback_performance.clone(),
            })
        }
    }

    /// Optimize transport selection based on current metrics
    async fn optimize_transport_selection(
        &self,
        request: &SharedFileRequest,
        performance: &PerformanceSnapshot,
    ) -> Result<RoutingDecision, TransportError> {
        let local_perf = self.estimate_local_performance(request).await;
        let network_perf = self.estimate_network_performance(request).await;

        // Score each transport based on multiple factors
        let local_score = self.calculate_transport_score(request, &local_perf, &performance.local);
        let network_score =
            self.calculate_transport_score(request, &network_perf, &performance.network);

        let (selected_transport, selected_performance, confidence) = if local_score > network_score
        {
            (
                SelectedTransport::SharedMemory,
                local_perf,
                local_score / (local_score + network_score),
            )
        } else {
            (
                SelectedTransport::Network,
                network_perf,
                network_score / (local_score + network_score),
            )
        };

        Ok(RoutingDecision {
            transport: selected_transport,
            reason: RoutingReason::PerformanceOptimization {
                metric: format!(
                    "Score: local={:.2}, network={:.2}",
                    local_score, network_score
                ),
            },
            confidence,
            expected_performance: selected_performance,
        })
    }

    /// Adaptive transport selection with machine learning-like behavior
    async fn adaptive_transport_selection(
        &self,
        request: &SharedFileRequest,
        performance: &PerformanceSnapshot,
    ) -> Result<RoutingDecision, TransportError> {
        // This would use historical data to predict best performance
        // For now, implement a simple heuristic-based approach

        let message_size = request.operation.estimated_data_size();
        let current_connections = performance.network.active_connections;

        // Large messages often benefit from network transport
        if message_size
            > self
                .get_config()
                .performance_thresholds
                .large_message_threshold_bytes
        {
            return Ok(RoutingDecision {
                transport: SelectedTransport::Network,
                reason: RoutingReason::PerformanceOptimization {
                    metric: format!("Large message size: {} bytes", message_size),
                },
                confidence: 0.7,
                expected_performance: self.estimate_network_performance(request).await,
            });
        }

        // High connection count may saturate local transport
        if current_connections
            > self
                .get_config()
                .performance_thresholds
                .high_connection_threshold
        {
            return Ok(RoutingDecision {
                transport: SelectedTransport::Network,
                reason: RoutingReason::LoadBalancing,
                confidence: 0.6,
                expected_performance: self.estimate_network_performance(request).await,
            });
        }

        // Default to optimized selection
        self.optimize_transport_selection(request, performance)
            .await
    }

    /// Calculate a score for a transport option
    fn calculate_transport_score(
        &self,
        _request: &SharedFileRequest,
        estimated_perf: &PerformanceProfile,
        current_metrics: &TransportMetrics,
    ) -> f64 {
        let mut score = 0.0;

        // Latency component (lower is better)
        let latency_score = if estimated_perf.expected_latency_us > 0 {
            1.0 / (estimated_perf.expected_latency_us as f64)
        } else {
            1.0
        };
        score += latency_score * 1000.0; // Weight latency heavily

        // Throughput component (higher is better)
        score += (estimated_perf.expected_throughput_mbps as f64) * 10.0;

        // Reliability component
        score += current_metrics.success_rate * 100.0;

        // Connection load penalty
        if current_metrics.active_connections > 0 {
            score *= 1.0 - (current_metrics.active_connections as f64 / 1000.0).min(0.5);
        }

        score.max(0.0)
    }

    /// Check if a performance profile meets the requirements
    fn meets_performance_requirements(
        &self,
        request: &SharedFileRequest,
        performance: &PerformanceProfile,
    ) -> bool {
        let reqs = &request.performance_requirements;
        if let Some(max_latency) = reqs.max_latency_ms {
            if performance.expected_latency_us > max_latency {
                return false;
            }
        }

        if let Some(min_throughput) = reqs.min_throughput_mbps {
            if performance.expected_throughput_mbps < min_throughput {
                return false;
            }
        }

        // Note: expected_reliability field doesn't exist in PerformanceProfile

        true
    }

    /// Execute a request using local shared memory transport
    async fn execute_local_request(
        &self,
        request: SharedFileRequest,
    ) -> Result<SharedFileOperationResponse, TransportError> {
        self.get_local_transport().execute_request(request).await
    }

    /// Execute a request using network transport
    async fn execute_network_request(
        &self,
        request: SharedFileRequest,
    ) -> Result<SharedFileOperationResponse, TransportError> {
        self.get_network_transport().execute_request(request).await
    }

    /// Estimate performance for local transport
    async fn estimate_local_performance(&self, _request: &SharedFileRequest) -> PerformanceProfile {
        // This would use historical data and current system state
        // For now, provide reasonable estimates

        let base_latency = 50.0; // 50 microseconds base latency
        let throughput = 1000.0; // 1 GB/s typical shared memory throughput

        PerformanceProfile {
            expected_latency_us: base_latency as u32,
            expected_throughput_mbps: throughput as u32,
            high_performance: true,
            tier: crate::manager::PerformanceTier::UltraLow,
        }
    }

    /// Estimate performance for network transport
    async fn estimate_network_performance(
        &self,
        _request: &SharedFileRequest,
    ) -> PerformanceProfile {
        // This would consider network conditions, RTT, bandwidth, etc.
        // For now, provide reasonable estimates

        let base_latency = 1000.0; // 1ms base network latency
        let throughput = 100.0; // 100 MB/s typical network throughput

        PerformanceProfile {
            expected_latency_us: base_latency as u32,
            expected_throughput_mbps: throughput as u32,
            high_performance: false,
            tier: crate::manager::PerformanceTier::Medium,
        }
    }

    /// Record a performance sample for future decision making
    async fn record_performance_sample(
        &self,
        transport: &SelectedTransport,
        latency: Duration,
        success: bool,
        request: &SharedFileRequest,
    ) {
        let sample = PerformanceSample {
            timestamp: Utc::now(),
            latency_us: latency.as_micros() as f64,
            throughput_mbps: if latency.as_micros() > 0 {
                (request.operation.estimated_data_size() as f64) / (latency.as_micros() as f64)
            } else {
                0.0
            },
            success_rate: if success { 1.0 } else { 0.0 },
            connection_count: match transport {
                SelectedTransport::SharedMemory => 1,
                SelectedTransport::Network => {
                    self.network_transport().active_connections().len() as u32
                }
                SelectedTransport::Hybrid { .. } => 1,
            },
            message_size: request.operation.estimated_data_size(),
        };

        self.performance_monitor()
            .record_sample(transport, sample)
            .await;
    }

    /// Get access to the performance monitor
    pub fn get_performance_monitor(&self) -> &PerformanceMonitor {
        self.performance_monitor()
    }

    /// Get access to the configuration
    pub fn get_config(&self) -> &TransportConfig {
        self.config()
    }

    /// Get access to the local transport
    pub fn get_local_transport(&self) -> &SharedMemoryTransport {
        self.local_transport()
    }

    /// Get access to the network transport
    pub fn get_network_transport(&self) -> &NetworkTransport {
        self.network_transport()
    }
}

/// Errors that can occur in the transport layer
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Configuration error: {0}")]
    InvalidConfiguration(String),

    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),

    #[error("Operation timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Transport not available: {transport}")]
    TransportUnavailable { transport: String },

    #[error("Permission denied for operation: {operation}")]
    PermissionDenied { operation: String },

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("Memory mapping error: {0}")]
    MemoryMapping(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for TransportError {
    fn from(err: anyhow::Error) -> Self {
        // Convert anyhow::Error -> CommyError -> TransportError using the central mapping
        let com_err = crate::errors::CommyError::Other(err.to_string());
        TransportError::from(com_err)
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for TransportError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        // Convert the boxed error into our crate-local CommyError when possible,
        // then map to the appropriate TransportError variant. This consumes the box.
        let com_err = crate::errors::CommyError::Other(err.to_string());
        map_commy_error_to_transport_error(com_err, None)
    }
}

impl From<crate::errors::CommyError> for TransportError {
    fn from(err: crate::errors::CommyError) -> Self {
        map_commy_error_to_transport_error(err, None)
    }
}

/// Map a crate-level CommyError into a TransportError while accepting optional
/// context such as the serialization format. This preserves path/format
/// information when available.
pub fn map_commy_error_to_transport_error(
    com_err: crate::errors::CommyError,
    format: Option<crate::manager::SerializationFormat>,
) -> TransportError {
    match com_err {
        crate::errors::CommyError::JsonSerialization(e) => {
            let fmt = format.unwrap_or(crate::manager::SerializationFormat::Json);
            TransportError::Serialization(format!("{:?}: {}", fmt, e))
        }
        crate::errors::CommyError::BinarySerialization(s) => {
            let fmt = format.unwrap_or(crate::manager::SerializationFormat::Binary);
            TransportError::Serialization(format!("{:?}: {}", fmt, s))
        }
        crate::errors::CommyError::MessagePackSerialization(s) => {
            let fmt = format.unwrap_or(crate::manager::SerializationFormat::MessagePack);
            TransportError::Serialization(format!("{:?}: {}", fmt, s))
        }
        crate::errors::CommyError::CborSerialization(s) => {
            let fmt = format.unwrap_or(crate::manager::SerializationFormat::Cbor);
            TransportError::Serialization(format!("{:?}: {}", fmt, s))
        }
        // No legacy Serialize variant exists — explicit serialization variants
        // are handled above. Any other variant will fall through into the
        // generic `other` arm below.
        crate::errors::CommyError::Io { source: e, path } => {
            if let Some(p) = path {
                TransportError::FileSystem(format!("{}: {}", p.display(), e))
            } else {
                TransportError::FileSystem(format!("{}", e))
            }
        }
        crate::errors::CommyError::PluginLoad(s) => TransportError::Protocol(s),
        crate::errors::CommyError::BufferTooSmall => {
            TransportError::ResourceUnavailable("buffer too small".to_string())
        }
        crate::errors::CommyError::InvalidArgument(s) => TransportError::Protocol(s),
        crate::errors::CommyError::Other(s) => TransportError::Protocol(s),
        // Any newer/richer variants we haven't explicitly handled map to a
        // generic protocol error preserving the Display form. This keeps the
        // migration conservative and avoids a large exhaustive match update.
        other => TransportError::Protocol(format!("{}", other)),
    }
}

// Extension traits for operations
impl SharedFileOperation {
    /// Estimate the data size for this operation
    pub fn estimated_data_size(&self) -> u64 {
        match self {
            SharedFileOperation::Read { .. } => 0, // Read doesn't send data
            SharedFileOperation::Write { data, .. } => data.len() as u64,
            SharedFileOperation::Append { data, .. } => data.len() as u64,
            SharedFileOperation::Create { initial_data, .. } => {
                initial_data.as_ref().map(|d| d.len() as u64).unwrap_or(0)
            }
            SharedFileOperation::Delete { .. } => 0,
            SharedFileOperation::Copy { .. } => 1024, // Estimate based on metadata
            SharedFileOperation::Move { .. } => 1024, // Estimate based on metadata
            SharedFileOperation::List { .. } => 0,
            SharedFileOperation::GetInfo { .. } => 0,
            SharedFileOperation::SetPermissions { .. } => 64, // Small metadata update
            SharedFileOperation::Resize { .. } => 64,         // Small metadata update
        }
    }
}
