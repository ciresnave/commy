//! File Lifecycle Management
//!
//! This module handles file lifecycle management including TTL (Time To Live)
//! tracking, automatic cleanup, expiration monitoring, and resource management.

use super::coordination::DeletionReason;
use super::*;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Default cleanup interval - how often to check for expired files
pub const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(60); // 1 minute

/// Default file TTL if none specified
pub const DEFAULT_TTL_SECONDS: u64 = 3600; // 1 hour

/// Grace period before actual deletion to allow final access
pub const DELETION_GRACE_PERIOD: Duration = Duration::from_secs(30);

/// Maximum number of files to cleanup in one batch
pub const MAX_CLEANUP_BATCH_SIZE: usize = 100;

/// Lifecycle management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// How often to run cleanup checks
    pub cleanup_interval_seconds: u64,
    /// Default TTL for files without specific TTL
    pub default_ttl_seconds: u64,
    /// Grace period before deletion
    pub grace_period_seconds: u64,
    /// Maximum batch size for cleanup operations
    pub max_cleanup_batch: usize,
    /// Enable automatic cleanup
    pub auto_cleanup_enabled: bool,
    /// Enable TTL enforcement
    pub ttl_enforcement_enabled: bool,
    /// Enable lifecycle event notifications
    pub event_notifications_enabled: bool,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            cleanup_interval_seconds: DEFAULT_CLEANUP_INTERVAL.as_secs(),
            default_ttl_seconds: DEFAULT_TTL_SECONDS,
            grace_period_seconds: DELETION_GRACE_PERIOD.as_secs(),
            max_cleanup_batch: MAX_CLEANUP_BATCH_SIZE,
            auto_cleanup_enabled: true,
            ttl_enforcement_enabled: true,
            event_notifications_enabled: true,
        }
    }
}

/// File lifecycle state tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LifecycleState {
    /// File is newly created and active
    Active,
    /// File is approaching expiration
    ExpirationWarning {
        /// Time until expiration
        expires_in_seconds: u64,
    },
    /// File has expired but is in grace period
    Expired {
        /// Time when file expired
        expired_at: DateTime<Utc>,
        /// Grace period end time
        grace_period_ends: DateTime<Utc>,
    },
    /// File is marked for deletion
    MarkedForDeletion {
        /// Reason for deletion
        reason: DeletionReason,
        /// Scheduled deletion time
        deletion_time: DateTime<Utc>,
    },
    /// File is being cleaned up
    CleaningUp,
    /// File has been deleted
    Deleted {
        /// Time of deletion
        deleted_at: DateTime<Utc>,
        /// Reason for deletion
        reason: DeletionReason,
    },
}

/// Reason for file expiration or deletion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExpirationReason {
    /// TTL expired
    TtlExpired,
    /// Manual expiration request
    Manual,
    /// Inactivity timeout
    InactivityTimeout,
    /// Resource pressure
    ResourcePressure,
    /// System shutdown
    SystemShutdown,
    /// Error condition
    ErrorCondition(String),
}

/// Lifecycle event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LifecycleEvent {
    /// File lifecycle state changed
    StateChanged {
        file_id: u64,
        identifier: String,
        old_state: LifecycleState,
        new_state: LifecycleState,
        timestamp: DateTime<Utc>,
    },
    /// File is approaching expiration
    ExpirationWarning {
        file_id: u64,
        identifier: String,
        expires_at: DateTime<Utc>,
        warning_threshold_seconds: u64,
    },
    /// File has expired
    FileExpired {
        file_id: u64,
        identifier: String,
        expired_at: DateTime<Utc>,
        reason: ExpirationReason,
    },
    /// File cleanup started
    CleanupStarted {
        file_id: u64,
        identifier: String,
        reason: ExpirationReason,
    },
    /// File cleanup completed
    CleanupCompleted {
        file_id: u64,
        identifier: String,
        cleanup_duration_ms: u64,
    },
    /// Cleanup batch completed
    BatchCleanupCompleted {
        files_processed: usize,
        files_deleted: usize,
        batch_duration_ms: u64,
    },
}

/// Lifecycle tracking information for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLifecycleInfo {
    /// File identifier
    pub file_id: u64,
    /// File identifier string
    pub identifier: String,
    /// Current lifecycle state
    pub state: LifecycleState,
    /// File creation time
    pub created_at: DateTime<Utc>,
    /// Last access time
    pub last_accessed: DateTime<Utc>,
    /// Last modification time
    pub last_modified: DateTime<Utc>,
    /// Expiration time (if any)
    pub expires_at: Option<DateTime<Utc>>,
    /// TTL in seconds
    pub ttl_seconds: Option<u64>,
    /// Whether auto-cleanup is enabled for this file
    pub auto_cleanup: bool,
    /// Number of active connections
    pub active_connections: u32,
    /// Total access count
    pub access_count: u64,
    /// Size in bytes
    pub size_bytes: u64,
    /// Custom tags for lifecycle management
    pub lifecycle_tags: Vec<String>,
}

/// File lifecycle manager
#[derive(Debug)]
pub struct LifecycleManager {
    /// Configuration
    config: LifecycleConfig,
    /// File lifecycle tracking
    file_lifecycles: Arc<RwLock<HashMap<u64, FileLifecycleInfo>>>,
    /// Cleanup task handle
    cleanup_task_handle: Option<tokio::task::JoinHandle<()>>,
    /// Event broadcaster
    event_broadcaster: tokio::sync::broadcast::Sender<LifecycleEvent>,
    /// Cleanup statistics
    cleanup_stats: Arc<RwLock<CleanupStatistics>>,
}

/// Cleanup operation statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CleanupStatistics {
    /// Total cleanup runs
    pub total_cleanup_runs: u64,
    /// Total files processed
    pub total_files_processed: u64,
    /// Total files deleted
    pub total_files_deleted: u64,
    /// Last cleanup time
    pub last_cleanup_time: Option<DateTime<Utc>>,
    /// Average cleanup duration in milliseconds
    pub avg_cleanup_duration_ms: f64,
    /// Files by deletion reason
    pub deletions_by_reason: HashMap<String, u64>,
}

impl LifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(config: LifecycleConfig) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(1000);

        Self {
            config,
            file_lifecycles: Arc::new(RwLock::new(HashMap::new())),
            cleanup_task_handle: None,
            event_broadcaster: event_tx,
            cleanup_stats: Arc::new(RwLock::new(CleanupStatistics::default())),
        }
    }

    /// Start the lifecycle management service
    pub async fn start(&mut self) -> Result<(), ManagerError> {
        if !self.config.auto_cleanup_enabled {
            info!("Lifecycle manager started but auto-cleanup is disabled");
            return Ok(());
        }

        info!(
            "Starting lifecycle manager with cleanup interval: {}s",
            self.config.cleanup_interval_seconds
        );

        // Start cleanup task
        let cleanup_interval = Duration::from_secs(self.config.cleanup_interval_seconds);
        let file_lifecycles = Arc::clone(&self.file_lifecycles);
        let event_broadcaster = self.event_broadcaster.clone();
        let cleanup_stats = Arc::clone(&self.cleanup_stats);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;

                let start_time = Instant::now();
                match Self::run_cleanup_cycle(
                    &file_lifecycles,
                    &event_broadcaster,
                    &cleanup_stats,
                    &config,
                )
                .await
                {
                    Ok((processed, deleted)) => {
                        let duration = start_time.elapsed();
                        debug!(
                            "Cleanup cycle completed: processed {} files, deleted {} files in {:?}",
                            processed, deleted, duration
                        );

                        // Broadcast batch completion event
                        let _ = event_broadcaster.send(LifecycleEvent::BatchCleanupCompleted {
                            files_processed: processed,
                            files_deleted: deleted,
                            batch_duration_ms: duration.as_millis() as u64,
                        });
                    }
                    Err(e) => {
                        error!("Cleanup cycle failed: {}", e);
                    }
                }
            }
        });

        self.cleanup_task_handle = Some(handle);
        info!("Lifecycle manager cleanup task started");
        Ok(())
    }

    /// Stop the lifecycle management service
    pub async fn stop(&mut self) {
        if let Some(handle) = self.cleanup_task_handle.take() {
            handle.abort();
            info!("Lifecycle manager cleanup task stopped");
        }
    }

    /// Subscribe to lifecycle events
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<LifecycleEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Track a new file in the lifecycle system
    pub async fn track_file(
        &self,
        file_id: u64,
        identifier: String,
        metadata: &FileMetadata,
    ) -> Result<(), ManagerError> {
        let now = Utc::now();
        let ttl_seconds = metadata.original_request.ttl_seconds;
        let expires_at = ttl_seconds.map(|ttl| now + ChronoDuration::seconds(ttl as i64));

        let lifecycle_info = FileLifecycleInfo {
            file_id,
            identifier: identifier.clone(),
            state: LifecycleState::Active,
            created_at: metadata.created_at,
            last_accessed: metadata.last_accessed,
            last_modified: metadata.last_modified,
            expires_at,
            ttl_seconds,
            auto_cleanup: metadata.original_request.auto_cleanup,
            active_connections: metadata.connection_count,
            access_count: 0, // Initialize to 0, will be updated
            size_bytes: metadata.size_bytes,
            lifecycle_tags: metadata.tags.clone(),
        };

        {
            let mut lifecycles = self.file_lifecycles.write().await;
            lifecycles.insert(file_id, lifecycle_info);
        }

        info!(
            "Started tracking file {} (ID: {}) in lifecycle system",
            identifier, file_id
        );
        Ok(())
    }

    /// Update file access information
    pub async fn update_file_access(
        &self,
        file_id: u64,
        active_connections: u32,
    ) -> Result<(), ManagerError> {
        let mut lifecycles = self.file_lifecycles.write().await;

        if let Some(info) = lifecycles.get_mut(&file_id) {
            info.last_accessed = Utc::now();
            info.active_connections = active_connections;
            info.access_count += 1;

            // Check if file should transition out of expiration warning
            if let LifecycleState::ExpirationWarning { .. } = &info.state {
                if active_connections > 0 {
                    // File is being accessed, keep it active
                    let old_state = info.state.clone();
                    info.state = LifecycleState::Active;

                    // Broadcast state change
                    if self.config.event_notifications_enabled {
                        let _ = self.event_broadcaster.send(LifecycleEvent::StateChanged {
                            file_id,
                            identifier: info.identifier.clone(),
                            old_state,
                            new_state: info.state.clone(),
                            timestamp: Utc::now(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Mark a file for deletion
    pub async fn mark_for_deletion(
        &self,
        file_id: u64,
        reason: ExpirationReason,
    ) -> Result<(), ManagerError> {
        let mut lifecycles = self.file_lifecycles.write().await;

        if let Some(info) = lifecycles.get_mut(&file_id) {
            let old_state = info.state.clone();
            let deletion_time =
                Utc::now() + ChronoDuration::seconds(self.config.grace_period_seconds as i64);

            info.state = LifecycleState::MarkedForDeletion {
                reason: DeletionReason::from_expiration_reason(&reason),
                deletion_time,
            };

            // Broadcast state change
            if self.config.event_notifications_enabled {
                let _ = self.event_broadcaster.send(LifecycleEvent::StateChanged {
                    file_id,
                    identifier: info.identifier.clone(),
                    old_state,
                    new_state: info.state.clone(),
                    timestamp: Utc::now(),
                });
            }

            info!(
                "Marked file {} for deletion due to {:?}",
                info.identifier, reason
            );
        }

        Ok(())
    }

    /// Get lifecycle information for a file
    pub async fn get_file_lifecycle(&self, file_id: u64) -> Option<FileLifecycleInfo> {
        let lifecycles = self.file_lifecycles.read().await;
        lifecycles.get(&file_id).cloned()
    }

    /// Get files that are approaching expiration
    pub async fn get_expiring_files(&self, threshold_seconds: u64) -> Vec<FileLifecycleInfo> {
        let lifecycles = self.file_lifecycles.read().await;
        let now = Utc::now();
        let threshold = ChronoDuration::seconds(threshold_seconds as i64);

        lifecycles
            .values()
            .filter(|info: &&FileLifecycleInfo| {
                info.expires_at.is_some_and(|expires_at| {
                    let time_until_expiration = expires_at - now;
                    time_until_expiration <= threshold && time_until_expiration.num_seconds() > 0
                })
            })
            .cloned()
            .collect()
    }

    /// Get cleanup statistics
    pub async fn get_cleanup_stats(&self) -> CleanupStatistics {
        self.cleanup_stats.read().await.clone()
    }

    /// Force cleanup of expired files
    pub async fn force_cleanup(&self) -> Result<(usize, usize), ManagerError> {
        let start_time = Instant::now();
        let result = Self::run_cleanup_cycle(
            &self.file_lifecycles,
            &self.event_broadcaster,
            &self.cleanup_stats,
            &self.config,
        )
        .await;

        if let Ok((processed, deleted)) = result {
            let duration = start_time.elapsed();
            info!(
                "Force cleanup completed: processed {} files, deleted {} files in {:?}",
                processed, deleted, duration
            );
        }

        result
    }

    /// Run a cleanup cycle
    async fn run_cleanup_cycle(
        file_lifecycles: &Arc<RwLock<HashMap<u64, FileLifecycleInfo>>>,
        event_broadcaster: &tokio::sync::broadcast::Sender<LifecycleEvent>,
        cleanup_stats: &Arc<RwLock<CleanupStatistics>>,
        config: &LifecycleConfig,
    ) -> Result<(usize, usize), ManagerError> {
        let now = Utc::now();
        let mut files_deleted = 0;

        // Get list of files to process
        let files_to_check: Vec<_> = {
            let lifecycles = file_lifecycles.read().await;
            lifecycles.values().cloned().collect()
        };

        let files_processed = files_to_check.len();

        for mut info in files_to_check {
            // Skip if already deleted
            if matches!(info.state, LifecycleState::Deleted { .. }) {
                continue;
            }

            // Check for TTL expiration
            if config.ttl_enforcement_enabled {
                if let Some(expires_at) = info.expires_at {
                    if now >= expires_at {
                        // File has expired
                        match &info.state {
                            LifecycleState::Active | LifecycleState::ExpirationWarning { .. } => {
                                // Move to expired state
                                let grace_period_ends = now
                                    + ChronoDuration::seconds(config.grace_period_seconds as i64);
                                let old_state = info.state.clone();
                                info.state = LifecycleState::Expired {
                                    expired_at: now,
                                    grace_period_ends,
                                };

                                // Broadcast expiration event
                                if config.event_notifications_enabled {
                                    let _ = event_broadcaster.send(LifecycleEvent::FileExpired {
                                        file_id: info.file_id,
                                        identifier: info.identifier.clone(),
                                        expired_at: now,
                                        reason: ExpirationReason::TtlExpired,
                                    });

                                    let _ = event_broadcaster.send(LifecycleEvent::StateChanged {
                                        file_id: info.file_id,
                                        identifier: info.identifier.clone(),
                                        old_state,
                                        new_state: info.state.clone(),
                                        timestamp: now,
                                    });
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Check for expiration warning
                        let warning_threshold = ChronoDuration::seconds(300); // 5 minutes
                        if now + warning_threshold >= expires_at
                            && info.state == LifecycleState::Active
                        {
                            let old_state = info.state.clone();
                            info.state = LifecycleState::ExpirationWarning {
                                expires_in_seconds: (expires_at - now).num_seconds() as u64,
                            };

                            // Broadcast warning event
                            if config.event_notifications_enabled {
                                let _ = event_broadcaster.send(LifecycleEvent::ExpirationWarning {
                                    file_id: info.file_id,
                                    identifier: info.identifier.clone(),
                                    expires_at,
                                    warning_threshold_seconds: 300,
                                });

                                let _ = event_broadcaster.send(LifecycleEvent::StateChanged {
                                    file_id: info.file_id,
                                    identifier: info.identifier.clone(),
                                    old_state,
                                    new_state: info.state.clone(),
                                    timestamp: now,
                                });
                            }
                        }
                    }
                }
            }

            // Check for files to delete
            let should_delete = match &info.state {
                LifecycleState::Expired {
                    grace_period_ends, ..
                } => now >= *grace_period_ends && info.active_connections == 0,
                LifecycleState::MarkedForDeletion { deletion_time, .. } => now >= *deletion_time,
                _ => false,
            };

            if should_delete && info.auto_cleanup {
                // Mark as cleaning up
                let old_state = info.state.clone();
                info.state = LifecycleState::CleaningUp;

                // Broadcast cleanup started
                if config.event_notifications_enabled {
                    let _ = event_broadcaster.send(LifecycleEvent::CleanupStarted {
                        file_id: info.file_id,
                        identifier: info.identifier.clone(),
                        reason: ExpirationReason::TtlExpired, // TODO: Extract actual reason
                    });
                }

                // TODO: Actual file deletion would happen here
                // For now, just mark as deleted
                let cleanup_start = Instant::now();

                info.state = LifecycleState::Deleted {
                    deleted_at: now,
                    reason: DeletionReason::TtlExpired,
                };

                files_deleted += 1;

                // Broadcast cleanup completed
                if config.event_notifications_enabled {
                    let _ = event_broadcaster.send(LifecycleEvent::CleanupCompleted {
                        file_id: info.file_id,
                        identifier: info.identifier.clone(),
                        cleanup_duration_ms: cleanup_start.elapsed().as_millis() as u64,
                    });

                    let _ = event_broadcaster.send(LifecycleEvent::StateChanged {
                        file_id: info.file_id,
                        identifier: info.identifier.clone(),
                        old_state,
                        new_state: info.state.clone(),
                        timestamp: now,
                    });
                }
            }

            // Update the lifecycle info
            {
                let mut lifecycles = file_lifecycles.write().await;
                lifecycles.insert(info.file_id, info);
            }
        }

        // Update cleanup statistics
        {
            let mut stats = cleanup_stats.write().await;
            stats.total_cleanup_runs += 1;
            stats.total_files_processed += files_processed as u64;
            stats.total_files_deleted += files_deleted as u64;
            stats.last_cleanup_time = Some(now);

            // Update average duration (simple moving average)
            let run_duration = 0; // TODO: Calculate actual duration
            if stats.total_cleanup_runs == 1 {
                stats.avg_cleanup_duration_ms = run_duration as f64;
            } else {
                stats.avg_cleanup_duration_ms = (stats.avg_cleanup_duration_ms
                    * (stats.total_cleanup_runs - 1) as f64
                    + run_duration as f64)
                    / stats.total_cleanup_runs as f64;
            }
        }

        Ok((files_processed, files_deleted))
    }
}

impl DeletionReason {
    /// Convert from ExpirationReason to DeletionReason
    fn from_expiration_reason(reason: &ExpirationReason) -> Self {
        match reason {
            ExpirationReason::TtlExpired => DeletionReason::TtlExpired,
            ExpirationReason::Manual => DeletionReason::Manual,
            ExpirationReason::InactivityTimeout => DeletionReason::Inactive,
            ExpirationReason::ResourcePressure => {
                DeletionReason::Error("Resource pressure".to_string())
            }
            ExpirationReason::SystemShutdown => DeletionReason::PeerShutdown,
            ExpirationReason::ErrorCondition(msg) => DeletionReason::Error(msg.clone()),
        }
    }
}
