//! Shared Memory Transport Implementation
//!
//! This module provides an older/shared-memory-based transport implementation.
//! It's retained for compatibility and contains the previous API shape used in
//! early prototypes. The implementation uses memory-mapped files via
//! commy_common::FieldHolder and provides basic Create/Read/Write/Delete
//! operations backed by files in a configurable base directory.

use super::{
    transport::*, transport_impl::TransportError, SharedFileOperation, SharedFileRequest,
    SharedFileResponse,
};
use commy_common::FieldHolder;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Shared memory transport using memory-mapped files
#[derive(Debug)]
pub struct SharedMemoryTransport {
    /// Configuration for shared memory behavior
    config: SharedMemoryConfig,

    /// Active memory-mapped files indexed by file path
    active_files: Arc<RwLock<HashMap<String, FieldHolder>>>,

    /// Performance metrics collection
    metrics: Arc<RwLock<SharedMemoryMetrics>>,

    /// Base directory for shared memory files
    base_directory: PathBuf,
}

/// Performance metrics for shared memory operations
#[derive(Debug, Clone)]
pub struct SharedMemoryMetrics {
    /// Total number of requests processed
    pub total_requests: u64,

    /// Total number of successful operations
    pub successful_operations: u64,

    /// Total number of failed operations
    pub failed_operations: u64,

    /// Average latency in microseconds
    pub average_latency_us: f64,

    /// Maximum observed latency in microseconds
    pub max_latency_us: u64,

    /// Minimum observed latency in microseconds
    pub min_latency_us: u64,

    /// Total bytes read
    pub total_bytes_read: u64,

    /// Total bytes written
    pub total_bytes_written: u64,

    /// Last update timestamp
    pub last_updated: std::time::SystemTime,
}

impl Default for SharedMemoryMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_operations: 0,
            failed_operations: 0,
            average_latency_us: 0.0,
            max_latency_us: 0,
            min_latency_us: 0,
            total_bytes_read: 0,
            total_bytes_written: 0,
            last_updated: std::time::SystemTime::now(),
        }
    }
}

impl SharedMemoryTransport {
    /// Create a new shared memory transport
    pub async fn new(config: SharedMemoryConfig) -> Result<Self, TransportError> {
        let base_directory = config.files_directory.clone();

        // Ensure base directory exists
        if let Err(e) = tokio::fs::create_dir_all(&base_directory).await {
            // Attach path context and convert to TransportError via CommyError
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(base_directory.clone()),
            };
            return Err(TransportError::from(com_err));
        }

        Ok(Self {
            config,
            active_files: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(SharedMemoryMetrics::default())),
            base_directory,
        })
    }

    /// Execute a request using shared memory
    pub async fn execute_request(
        &self,
        request: SharedFileRequest,
    ) -> Result<SharedFileResponse, TransportError> {
        let start_time = Instant::now();

        let result = match &request.operation {
            SharedFileOperation::Write { path, offset, data } => {
                self.handle_write_operation(&request, path, *offset, data)
                    .await
            }
            SharedFileOperation::Read {
                path,
                offset,
                length,
            } => {
                self.handle_read_operation(&request, path, *offset, *length)
                    .await
            }
            SharedFileOperation::Create { path, initial_size } => {
                self.handle_create_operation(&request, path, *initial_size)
                    .await
            }
            SharedFileOperation::Delete { path } => {
                self.handle_delete_operation(&request, path).await
            }
            SharedFileOperation::Resize { path, new_size } => {
                self.handle_resize_operation(&request, path, *new_size)
                    .await
            }
            SharedFileOperation::GetInfo { path } => {
                self.handle_get_info_operation(&request, path).await
            }
        };

        let latency = start_time.elapsed();
        let success = result.is_ok();

        // Update metrics asynchronously
        self.update_metrics(latency, success, &request).await;

        result
    }

    /// Handle write operation to shared memory file
    async fn handle_write_operation(
        &self,
        request: &SharedFileRequest,
        path: &PathBuf,
        offset: u64,
        data: &[u8],
    ) -> Result<SharedFileResponse, TransportError> {
        let file_path = self.resolve_file_path(&request.name, path);

        // Get or create the shared memory file
        let field_holder = self.get_or_create_file(&file_path, request).await?;

        // Perform the write operation
        field_holder.write_raw(offset as usize, data).map_err(|e| {
            let com_err =
                crate::errors::CommyError::Other(format!("Write operation failed: {}", e));
            TransportError::from(com_err)
        })?;

        // Update write metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_bytes_written += data.len() as u64;
        }

        Ok(SharedFileResponse::WriteSuccess {
            bytes_written: data.len() as u64,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Handle read operation from shared memory file
    async fn handle_read_operation(
        &self,
        request: &SharedFileRequest,
        path: &PathBuf,
        offset: u64,
        length: u64,
    ) -> Result<SharedFileResponse, TransportError> {
        let file_path = self.resolve_file_path(&request.name, path);

        // Get the existing shared memory file
        let field_holder = self.get_existing_file(&file_path).await?;

        // Perform the read operation
        let data = field_holder
            .read_raw(offset as usize, length as usize)
            .map_err(|e| {
                let com_err =
                    crate::errors::CommyError::Other(format!("Read operation failed: {}", e));
                TransportError::from(com_err)
            })?;

        // Update read metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_bytes_read += data.len() as u64;
        }

        Ok(SharedFileResponse::ReadSuccess {
            data,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Handle create operation for new shared memory file
    async fn handle_create_operation(
        &self,
        request: &SharedFileRequest,
        path: &PathBuf,
        initial_size: u64,
    ) -> Result<SharedFileResponse, TransportError> {
        let file_path = self.resolve_file_path(&request.name, path);

        // Check if file already exists based on creation policy
        match request.creation_policy {
            super::CreationPolicy::Create => {
                // Always create new - remove existing if present
                if file_path.exists() {
                    let mut active_files = self.active_files.write().await;
                    active_files.remove(&file_path.to_string_lossy().to_string());
                    if let Err(e) = tokio::fs::remove_file(&file_path).await {
                        let com_err = crate::errors::CommyError::Io {
                            source: e,
                            path: Some(file_path.clone()),
                        };
                        return Err(TransportError::from(com_err));
                    }
                }
            }
        }

        // Create the shared memory file
        let field_holder = FieldHolder::create(&file_path, initial_size as usize).map_err(|e| {
            let com_err =
                crate::errors::CommyError::Other(format!("Failed to create shared file: {}", e));
            TransportError::from(com_err)
        })?;

        // Store in active files
        {
            let mut active_files = self.active_files.write().await;
            active_files.insert(file_path.to_string_lossy().to_string(), field_holder);
        }

        Ok(SharedFileResponse::CreateSuccess {
            file_size: initial_size,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Handle delete operation for shared memory file
    async fn handle_delete_operation(
        &self,
        request: &SharedFileRequest,
        path: &PathBuf,
    ) -> Result<SharedFileResponse, TransportError> {
        let file_path = self.resolve_file_path(&request.name, path);

        // Remove from active files
        {
            let mut active_files = self.active_files.write().await;
            active_files.remove(&file_path.to_string_lossy().to_string());
        }

        // Delete the physical file
        if file_path.exists() {
            tokio::fs::remove_file(&file_path).await.map_err(|e| {
                let com_err = crate::errors::CommyError::Io {
                    source: e,
                    path: Some(file_path.clone()),
                };
                TransportError::from(com_err)
            })?;
        }

        Ok(SharedFileResponse::DeleteSuccess {
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Handle resize operation for shared memory file
    async fn handle_resize_operation(
        &self,
        request: &SharedFileRequest,
        path: &PathBuf,
        new_size: u64,
    ) -> Result<SharedFileResponse, TransportError> {
        let file_path = self.resolve_file_path(&request.name, path);

        // Get existing file
        let field_holder = self.get_existing_file(&file_path).await?;

        // Resize the file
        field_holder.resize(new_size as usize).map_err(|e| {
            let com_err =
                crate::errors::CommyError::Other(format!("Resize operation failed: {}", e));
            TransportError::from(com_err)
        })?;

        Ok(SharedFileResponse::ResizeSuccess {
            new_size,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Handle get info operation for shared memory file
    async fn handle_get_info_operation(
        &self,
        request: &SharedFileRequest,
        path: &PathBuf,
    ) -> Result<SharedFileResponse, TransportError> {
        let file_path = self.resolve_file_path(&request.name, path);

        // Get file info
        if !file_path.exists() {
            return Err(TransportError::FileNotFound(
                file_path.to_string_lossy().to_string(),
            ));
        }

        let metadata = tokio::fs::metadata(&file_path).await.map_err(|e| {
            let com_err = crate::errors::CommyError::Io {
                source: e,
                path: Some(file_path.clone()),
            };
            TransportError::from(com_err)
        })?;

        Ok(SharedFileResponse::InfoSuccess {
            file_size: metadata.len(),
            created_at: metadata.created().ok(),
            modified_at: metadata.modified().ok(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Get or create a shared memory file
    async fn get_or_create_file(
        &self,
        file_path: &PathBuf,
        request: &SharedFileRequest,
    ) -> Result<FieldHolder, TransportError> {
        let file_key = file_path.to_string_lossy().to_string();

        // Check if already in active files
        {
            let active_files = self.active_files.read().await;
            if let Some(holder) = active_files.get(&file_key) {
                return Ok(holder.clone());
            }
        }

        // Create or open the file
        let field_holder = if file_path.exists() {
            FieldHolder::open(file_path).map_err(|e| {
                let com_err = crate::errors::CommyError::Other(format!(
                    "Failed to open existing file: {}",
                    e
                ));
                TransportError::from(com_err)
            })?
        } else {
            let default_size = request.max_size_bytes.unwrap_or(1024 * 1024); // 1MB default
            FieldHolder::create(file_path, default_size as usize).map_err(|e| {
                let com_err = crate::errors::CommyError::Other(format!(
                    "Failed to create shared file: {}",
                    e
                ));
                TransportError::from(com_err)
            })?
        };

        // Store in active files
        {
            let mut active_files = self.active_files.write().await;
            active_files.insert(file_key, field_holder.clone());
        }

        Ok(field_holder)
    }

    /// Get an existing shared memory file
    async fn get_existing_file(&self, file_path: &PathBuf) -> Result<FieldHolder, TransportError> {
        let file_key = file_path.to_string_lossy().to_string();

        // Check if already in active files
        {
            let active_files = self.active_files.read().await;
            if let Some(holder) = active_files.get(&file_key) {
                return Ok(holder.clone());
            }
        }

        // Try to open existing file
        if !file_path.exists() {
            return Err(TransportError::FileNotFound(
                file_path.to_string_lossy().to_string(),
            ));
        }

        let field_holder = FieldHolder::open(file_path).map_err(|e| {
            let com_err =
                crate::errors::CommyError::Other(format!("Failed to open existing file: {}", e));
            TransportError::from(com_err)
        })?;

        // Store in active files for future use
        {
            let mut active_files = self.active_files.write().await;
            active_files.insert(file_key, field_holder.clone());
        }

        Ok(field_holder)
    }

    /// Resolve file path from request name and operation path
    fn resolve_file_path(&self, request_name: &str, operation_path: &PathBuf) -> PathBuf {
        if operation_path.is_absolute() {
            operation_path.clone()
        } else {
            self.base_directory.join(format!(
                "{}_{}",
                request_name,
                operation_path.to_string_lossy()
            ))
        }
    }

    /// Update metrics for the transport
    async fn update_metrics(&self, latency: Duration, success: bool, _request: &SharedFileRequest) {
        let mut metrics = self.metrics.write().await;

        metrics.total_requests += 1;
        metrics.last_updated = std::time::SystemTime::now();

        if success {
            metrics.successful_operations += 1;
        } else {
            metrics.failed_operations += 1;
        }

        let latency_us = latency.as_micros() as u64;

        // Update latency statistics
        if metrics.total_requests == 1 {
            metrics.min_latency_us = latency_us;
            metrics.max_latency_us = latency_us;
            metrics.average_latency_us = latency_us as f64;
        } else {
            metrics.min_latency_us = metrics.min_latency_us.min(latency_us);
            metrics.max_latency_us = metrics.max_latency_us.max(latency_us);

            // Update running average
            let previous_total = metrics.average_latency_us * (metrics.total_requests - 1) as f64;
            metrics.average_latency_us =
                (previous_total + latency_us as f64) / metrics.total_requests as f64;
        }
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> SharedMemoryMetrics {
        self.metrics.read().await.clone()
    }

    /// Reset performance metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = SharedMemoryMetrics::default();
    }
}

// TODO: The rest of the shared memory implementation will be added
// in Phase 1 of the roadmap as a complete rewrite to match the current
// architecture. The current implementation was based on an older API design.
