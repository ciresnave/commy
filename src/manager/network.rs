//! Network Transport Implementation

use super::{
    transport::*, transport_impl::TransportError, SharedFileOperationResponse, SharedFileRequest,
};
// Duration isn't used in the current non-network implementation; keep imports
// minimal to avoid clippy warnings.

// Types used by both network-gated and non-gated implementations
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "network")]
use {
    dashmap::DashMap,
    std::net::SocketAddr,
    std::sync::Arc as StdArc,
    tokio::net::{TcpListener, TcpStream},
    tokio::time::{timeout, Duration as TokioDuration},
    tokio_rustls::TlsConnector,
};

#[cfg(feature = "network")]
impl NetworkTransport {
    /// Create a new network transport
    pub async fn new(config: NetworkConfig) -> Result<Self, TransportError> {
        let tls_connector = if config.tls_config.enabled {
            Some(Self::create_tls_connector(&config.tls_config).await?)
        } else {
            None
        };

        Ok(Self {
            config,
            active_connections: StdArc::new(DashMap::new()),
            connection_pool: StdArc::new(RwLock::new(Vec::new())),
            metrics: StdArc::new(RwLock::new(TransportMetrics::default())),
            tls_connector,
        })
    }

    /// Execute a request using network transport
    pub async fn execute_request(
        &self,
        request: SharedFileRequest,
    ) -> Result<SharedFileOperationResponse, TransportError> {
        let start_time = Instant::now();

        // Get the target endpoint from the configuration
        // For now, use the first available endpoint
        let target_endpoint_str = self.config().endpoints.first().ok_or_else(|| {
            TransportError::InvalidConfiguration("No network endpoints configured".to_string())
        })?;

        // Parse the endpoint string to create a NetworkEndpoint
        let parts: Vec<&str> = target_endpoint_str.split(':').collect();
        if parts.len() != 2 {
            return Err(TransportError::InvalidConfiguration(format!(
                "Invalid endpoint format: {}",
                target_endpoint_str
            )));
        }

        let host = parts[0].to_string();
        let port = parts[1].parse::<u16>().map_err(|_| {
            TransportError::InvalidConfiguration(format!(
                "Invalid port in endpoint: {}",
                target_endpoint_str
            ))
        })?;

        let target_endpoint = crate::manager::NetworkEndpoint {
            host,
            port,
            protocol: "TcpTls".to_string(),
            weight: 100,
            health: crate::manager::EndpointHealth::Healthy,
        };

        // Get or create connection to the target
        let connection = self.get_connection(&target_endpoint).await?;

        // Execute the request through the connection
        let result = self
            .execute_request_through_connection(request.clone(), &connection)
            .await;

        // Update metrics
        let latency = start_time.elapsed();
        self.update_metrics(latency, result.is_ok(), &request).await;

        result
    }

    /// Get or create a connection to the specified endpoint
    async fn get_connection(
        &self,
        endpoint: &NetworkEndpoint,
    ) -> Result<Arc<NetworkConnection>, TransportError> {
        let connection_key = format!("{}:{}", endpoint.host, endpoint.port);

        // Check if we have an active connection
        if let Some(connection_ref) = self.active_connections().get(&connection_key) {
            if connection_ref.state == ConnectionState::Connected {
                // Update last activity
                *connection_ref.last_activity.write().await = Utc::now();
                return Ok(connection_ref.value().clone());
            }
        }

        // Create new connection
        self.create_connection(endpoint).await
    }

    /// Create a new connection to the specified endpoint
    async fn create_connection(
        &self,
        endpoint: &NetworkEndpoint,
    ) -> Result<Arc<NetworkConnection>, TransportError> {
        let addr: SocketAddr = format!("{}:{}", endpoint.host, endpoint.port)
            .parse()
            .map_err(|e| {
                TransportError::InvalidConfiguration(format!("Invalid endpoint: {}", e))
            })?;

        // Connect with timeout
        let tcp_stream = timeout(
            TokioDuration::from_secs(self.config().connection_timeout_seconds as u64),
            TcpStream::connect(addr),
        )
        .await
        .map_err(|_| TransportError::Timeout {
            timeout_ms: self.config().connection_timeout_seconds as u64 * 1000,
        })?
        .map_err(|e| TransportError::Connection(format!("Failed to connect: {}", e)))?;

        // Configure TCP options
        if self.config().tcp_nodelay {
            tcp_stream.set_nodelay(true).map_err(|e| {
                TransportError::Connection(format!("Failed to set TCP_NODELAY: {}", e))
            })?;
        }

        // Setup TLS if enabled
        // NOTE: TLS handshake is temporarily disabled here to keep compilation
        // stable while we incrementally migrate to rustls. When enabled, the
        // connector will be used to perform a handshake and produce a TlsStream.
        let stream = NetworkStream::Tcp(tcp_stream);

        // Create protocol handler
        let protocol = ProtocolHandler::new();

        // Create connection object
        let connection = Arc::new(NetworkConnection {
            endpoint: endpoint.clone(),
            stream,
            state: ConnectionState::Connected,
            protocol,
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            created_at: Utc::now(),
            last_activity: Arc::new(RwLock::new(Utc::now())),
        });

        // Store the connection
        let connection_key = format!("{}:{}", endpoint.host, endpoint.port);
        self.active_connections()
            .insert(connection_key, connection.clone());

        Ok(connection)
    }

    /// Execute a request through an established connection
    async fn execute_request_through_connection(
        &self,
        request: SharedFileRequest,
        connection: &NetworkConnection,
    ) -> Result<SharedFileOperationResponse, TransportError> {
        // Convert request to protocol message
        let message = ProtocolMessage {
            message_id: uuid::Uuid::new_v4().to_string(),
            message_type: MessageType::FileOperation,
            payload: MessagePayload::FileRequest {
                request: Box::new(request),
            },
        };

        // Send the message
        self.send_message(connection, &message).await?;

        // Wait for response
        let response_message = self.receive_message(connection).await?;

        // Extract response from message
        match response_message.payload {
            MessagePayload::FileResponse { file_id: _, data } => {
                // For now, create a simple response - this would be more sophisticated in production
                Ok(SharedFileOperationResponse::ReadSuccess {
                    data,
                    timestamp: std::time::SystemTime::now(),
                })
            }
            MessagePayload::Error { error_message, .. } => Err(TransportError::Protocol(format!(
                "File operation failed: {}",
                error_message
            ))),
            _ => Err(TransportError::Protocol(
                "Unexpected message type in response".to_string(),
            )),
        }
    }

    /// Send a message through the connection
    async fn send_message(
        &self,
        connection: &NetworkConnection,
        message: &ProtocolMessage,
    ) -> Result<(), TransportError> {
        // Serialize the message
        let serialized = connection.protocol.serialize_message(message)?;

        // Send through the appropriate stream type
        match &connection.stream {
            NetworkStream::Tcp(stream) => {
                self.write_to_tcp_stream(stream, &serialized).await?;
            }
            #[cfg(feature = "network")]
            #[cfg(feature = "network")]
            NetworkStream::Tls(stream) => {
                // stream is a &Box<TlsStream<...>> here; pass the boxed stream directly
                self.write_to_tls_stream(stream, &serialized).await?;
            }
        }

        // Update connection stats
        {
            let mut stats = connection.stats.write().await;
            stats.messages_sent += 1;
            stats.bytes_sent += serialized.len() as u64;
        }

        Ok(())
    }

    /// Receive a message from the connection
    async fn receive_message(
        &self,
        connection: &NetworkConnection,
    ) -> Result<ProtocolMessage, TransportError> {
        // Read from the appropriate stream type
        let data = match &connection.stream {
            NetworkStream::Tcp(stream) => self.read_from_tcp_stream(stream).await?,
            #[cfg(feature = "network")]
            NetworkStream::Tls(stream) => self.read_from_tls_stream(stream).await?,
        };

        // Deserialize the message
        let message = connection.protocol.deserialize_message(&data)?;

        // Update connection stats
        {
            let mut stats = connection.stats.write().await;
            stats.messages_received += 1;
            stats.bytes_received += data.len() as u64;
        }

        Ok(message)
    }

    /// Write data to a TCP stream
    async fn write_to_tcp_stream(
        &self,
        _stream: &TcpStream,
        _data: &[u8],
    ) -> Result<(), TransportError> {
        // Create a writable reference to the stream
        // Note: In practice, you'd want to properly handle the stream ownership
        // This is a simplified implementation

        timeout(
            TokioDuration::from_secs(self.config().write_timeout_seconds as u64),
            async {
                // In a real implementation, you'd need mutable access to the stream
                // For now, this shows the structure
                Ok(())
            },
        )
        .await
        .map_err(|_| TransportError::Timeout {
            timeout_ms: self.config().write_timeout_seconds as u64 * 1000,
        })?
    }

    /// Read data from a TCP stream
    async fn read_from_tcp_stream(&self, _stream: &TcpStream) -> Result<Vec<u8>, TransportError> {
        timeout(
            TokioDuration::from_secs(self.config().read_timeout_seconds as u64),
            async {
                // In a real implementation, you'd need mutable access to the stream
                // and implement proper message framing
                // For now, return empty data to show structure
                Ok(Vec::new())
            },
        )
        .await
        .map_err(|_| TransportError::Timeout {
            timeout_ms: self.config().read_timeout_seconds as u64 * 1000,
        })?
    }

    /// Write data to a TLS stream
    #[cfg(feature = "network")]
    async fn write_to_tls_stream(
        &self,
        _stream: &tokio_rustls::client::TlsStream<TcpStream>,
        _data: &[u8],
    ) -> Result<(), TransportError> {
        timeout(
            TokioDuration::from_secs(self.config().write_timeout_seconds as u64),
            async {
                // In a real implementation, you'd need mutable access to the stream
                Ok(())
            },
        )
        .await
        .map_err(|_| TransportError::Timeout {
            timeout_ms: self.config().write_timeout_seconds as u64 * 1000,
        })?
    }

    /// Read data from a TLS stream
    #[cfg(feature = "network")]
    async fn read_from_tls_stream(
        &self,
        _stream: &tokio_rustls::client::TlsStream<TcpStream>,
    ) -> Result<Vec<u8>, TransportError> {
        timeout(
            TokioDuration::from_secs(self.config().read_timeout_seconds as u64),
            async {
                // In a real implementation, you'd need mutable access to the stream
                // and implement proper message framing
                Ok(Vec::new())
            },
        )
        .await
        .map_err(|_| TransportError::Timeout {
            timeout_ms: self.config().read_timeout_seconds as u64 * 1000,
        })?
    }

    /// Create a TLS connector from configuration
    #[cfg(feature = "network")]
    async fn create_tls_connector(config: &TlsConfig) -> Result<TlsConnector, TransportError> {
        use std::sync::Arc as StdArc;
        use tokio_rustls::rustls::{ClientConfig, RootCertStore};

        let mut root_store = RootCertStore::empty();

        // Optionally add custom CA certificate if provided
        if let Some(ref ca_path) = config.ca_cert_path {
            let ca_der = tokio::fs::read(ca_path).await.map_err(|e| {
                TransportError::InvalidConfiguration(format!("Failed to read CA cert: {}", e))
            })?;

            // Parse PEM(s) from the provided file and add
            let mut reader = std::io::BufReader::new(&ca_der[..]);
            // rustls_pemfile::certs returns an iterator of DER certificate blobs
            let certs_iter = rustls_pemfile::certs(&mut reader);
            let certs_vec = certs_iter.collect::<Result<Vec<_>, _>>().map_err(|e| {
                TransportError::InvalidConfiguration(format!("Failed to parse CA cert: {}", e))
            })?;

            if !certs_vec.is_empty() {
                // add_parsable_certificates accepts an IntoIterator<Item = CertificateDer>
                root_store.add_parsable_certificates(certs_vec);
            }
        }

        // Build a basic client config using the populated root store
        let client_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(TlsConnector::from(StdArc::new(client_config)))
    }

    /// Start a network server (for receiving connections)
    pub async fn start_server(&self, bind_addr: SocketAddr) -> Result<(), TransportError> {
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| TransportError::Connection(format!("Failed to bind server: {}", e)))?;

        // In a real implementation, this would run in a background task
        // and handle incoming connections

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((_stream, addr)) => {
                        // Handle incoming connection
                        tokio::spawn(async move {
                            // Process client connection
                            println!("Accepted connection from: {}", addr);
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Update transport metrics
    async fn update_metrics(&self, latency: Duration, success: bool, request: &SharedFileRequest) {
        let mut metrics = self.metrics().write().await;

        metrics.total_operations += 1;

        if success {
            // Update latency metrics (simplified exponential moving average)
            let latency_us = latency.as_micros() as f64;
            if metrics.avg_latency_us == 0.0 {
                metrics.avg_latency_us = latency_us;
            } else {
                metrics.avg_latency_us = metrics.avg_latency_us * 0.9 + latency_us * 0.1;
            }

            // Update success rate
            let total_ops = metrics.total_operations as f64;
            let current_successes = total_ops * metrics.success_rate + 1.0;
            metrics.success_rate = current_successes / total_ops;
            metrics.error_rate = 1.0 - metrics.success_rate;

            // Estimate throughput for data operations
            if latency.as_micros() > 0 {
                let data_size = request.operation.estimated_data_size();
                if data_size > 0 {
                    let throughput_mbps = (data_size as f64) / (latency.as_micros() as f64);
                    if metrics.avg_throughput_mbps == 0.0 {
                        metrics.avg_throughput_mbps = throughput_mbps;
                    } else {
                        metrics.avg_throughput_mbps =
                            metrics.avg_throughput_mbps * 0.9 + throughput_mbps * 0.1;
                    }

                    if throughput_mbps > metrics.peak_throughput_mbps {
                        metrics.peak_throughput_mbps = throughput_mbps;
                    }
                }
            }
        } else {
            // Update error rate
            let total_ops = metrics.total_operations as f64;
            let current_errors = total_ops * metrics.error_rate + 1.0;
            metrics.error_rate = current_errors / total_ops;
            metrics.success_rate = 1.0 - metrics.error_rate;
        }

        // Update active connections
        metrics.active_connections = self.active_connections().len() as u32;
    }

    /// Clean up inactive connections
    pub async fn cleanup_connections(&self) {
        let now = Utc::now();
        let timeout_duration = chrono::Duration::seconds(300); // 5 minutes

        let mut to_remove = Vec::new();

        for entry in self.active_connections().iter() {
            let last_activity = *entry.last_activity.read().await;
            if now.signed_duration_since(last_activity) > timeout_duration {
                to_remove.push(entry.key().clone());
            }
        }

        for key in to_remove {
            self.active_connections().remove(&key);
        }
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> Vec<(String, ConnectionStats)> {
        let mut stats = Vec::new();

        for entry in self.active_connections().iter() {
            let connection = entry.value();
            let connection_stats = connection.stats.read().await.clone();
            stats.push((entry.key().clone(), connection_stats));
        }

        stats
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(thresholds: PerformanceThresholds) -> Self {
        Self {
            history: Arc::new(RwLock::new(PerformanceHistory {
                local_samples: Vec::new(),
                network_samples: Vec::new(),
                max_samples: 1000,
            })),
            current_metrics: Arc::new(RwLock::new(PerformanceSnapshot {
                local: TransportMetrics::default(),
                network: TransportMetrics::default(),
                timestamp: Utc::now(),
            })),
            thresholds,
        }
    }

    /// Get current performance snapshot
    pub async fn get_current_snapshot(&self) -> PerformanceSnapshot {
        self.current_metrics().read().await.clone()
    }

    /// Record a performance sample
    pub async fn record_sample(&self, transport: &SelectedTransport, sample: PerformanceSample) {
        let mut history = self.history().write().await;

        match transport {
            SelectedTransport::SharedMemory => {
                history.local_samples_mut().push(sample);
                if history.local_samples().len() > history.max_samples() {
                    history.local_samples_mut().remove(0);
                }
            }
            SelectedTransport::Network => {
                history.network_samples_mut().push(sample);
                if history.network_samples().len() > history.max_samples() {
                    history.network_samples_mut().remove(0);
                }
            }
            SelectedTransport::Hybrid { .. } => {
                // For hybrid, record in both (simplified)
                history.local_samples_mut().push(sample.clone());
                history.network_samples_mut().push(sample);
            }
        }
    }

    /// Update current metrics snapshot
    pub async fn update_current_metrics(&self, local: TransportMetrics, network: TransportMetrics) {
        let mut current = self.current_metrics().write().await;
        current.local = local;
        current.network = network;
        current.timestamp = Utc::now();
    }

    /// Get performance history for analysis
    pub async fn get_performance_history(&self) -> PerformanceHistory {
        self.history().read().await.clone()
    }
}

// Implementations for Clone where needed
impl Clone for PerformanceHistory {
    fn clone(&self) -> Self {
        Self {
            local_samples: self.local_samples().clone(),
            network_samples: self.network_samples().clone(),
            max_samples: self.max_samples(),
        }
    }
}

// Feature-gated implementations for when network is not available
#[cfg(not(feature = "network"))]
impl NetworkTransport {
    pub async fn new(_config: NetworkConfig) -> Result<Self, TransportError> {
        Err(TransportError::TransportUnavailable {
            transport: "Network transport not available - compile with 'network' feature"
                .to_string(),
        })
    }

    pub async fn execute_request(
        &self,
        _request: SharedFileRequest,
    ) -> Result<SharedFileOperationResponse, TransportError> {
        Err(TransportError::TransportUnavailable {
            transport: "Network transport not available".to_string(),
        })
    }
}
