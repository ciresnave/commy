//! Unique File ID Management System
//!
//! This module provides distributed file ID allocation, reuse, and coordination
//! across multiple SharedFileManager instances. It ensures unique IDs while
//! enabling efficient reuse of deallocated IDs.

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Default ID range size allocated to each peer
pub const DEFAULT_ID_RANGE_SIZE: u64 = 10000;

/// Maximum number of reusable IDs to keep in memory
pub const MAX_REUSABLE_IDS: usize = 1000;

/// ID range allocation strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AllocationStrategy {
    /// Sequential allocation starting from base
    Sequential { base: u64 },
    /// Random allocation within range
    Random { min: u64, max: u64 },
    /// Hash-based allocation for deterministic distribution
    HashBased { peer_id: Uuid, range_size: u64 },
    /// Round-robin allocation across peers
    RoundRobin { peer_count: u32 },
}

/// ID range assigned to a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdRange {
    /// Range identifier
    pub range_id: Uuid,
    /// Peer that owns this range
    pub peer_id: Uuid,
    /// Start of the range (inclusive)
    pub start: u64,
    /// End of the range (inclusive)
    pub end: u64,
    /// Current position in the range
    pub current: u64,
    /// Allocation strategy for this range
    pub strategy: AllocationStrategy,
    /// When this range was allocated
    pub allocated_at: chrono::DateTime<chrono::Utc>,
    /// Whether this range is still active
    pub active: bool,
}

/// Reusable file ID with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReusableId {
    /// The file ID that can be reused
    pub id: u64,
    /// When this ID was released
    pub released_at: chrono::DateTime<chrono::Utc>,
    /// Why this ID was released
    pub release_reason: IdReleaseReason,
    /// Original identifier of the file
    pub original_identifier: String,
    /// Size of the original file
    pub original_size: u64,
}

/// Reason for ID release
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IdReleaseReason {
    /// File was manually deleted
    ManualDeletion,
    /// File expired due to TTL
    TtlExpiration,
    /// File was cleaned up due to inactivity
    InactivityCleanup,
    /// System shutdown cleanup
    SystemShutdown,
    /// Error condition
    Error(String),
}

/// ID allocation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdAllocationRequest {
    /// Request identifier
    pub request_id: Uuid,
    /// Requesting peer
    pub peer_id: Uuid,
    /// Number of IDs requested
    pub count: usize,
    /// Preferred allocation strategy
    pub preferred_strategy: Option<AllocationStrategy>,
    /// Priority of the request
    pub priority: AllocationPriority,
}

/// Priority for ID allocation requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum AllocationPriority {
    /// Low priority (background tasks)
    Low,
    /// Normal priority (regular operations)
    #[default]
    Normal,
    /// High priority (user-facing operations)
    High,
    /// Critical priority (system operations)
    Critical,
}

/// ID allocation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdAllocationResponse {
    /// Request identifier
    pub request_id: Uuid,
    /// Allocated IDs
    pub allocated_ids: Vec<u64>,
    /// Allocated range (if applicable)
    pub allocated_range: Option<IdRange>,
    /// Allocation success status
    pub success: bool,
    /// Error message if allocation failed
    pub error: Option<String>,
}

/// Statistics for ID management
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IdManagementStats {
    /// Total IDs allocated
    pub total_allocated: u64,
    /// Total IDs released
    pub total_released: u64,
    /// Total IDs reused
    pub total_reused: u64,
    /// Currently active IDs
    pub active_ids: u64,
    /// Available reusable IDs
    pub reusable_ids_count: usize,
    /// Number of allocated ranges
    pub allocated_ranges: usize,
    /// ID allocation efficiency (reuse rate)
    pub reuse_efficiency: f64,
}

/// File ID manager for distributed coordination
#[derive(Debug)]
pub struct FileIdManager {
    /// Our peer ID
    peer_id: Uuid,
    /// Currently allocated ranges
    allocated_ranges: Arc<RwLock<HashMap<Uuid, IdRange>>>,
    /// Next ID to allocate in each range
    range_positions: Arc<RwLock<HashMap<Uuid, AtomicU64>>>,
    /// Reusable IDs from deallocated files
    reusable_ids: Arc<RwLock<VecDeque<ReusableId>>>,
    /// Global next ID counter (fallback for single-node)
    global_next_id: AtomicU64,
    /// ID management statistics
    stats: Arc<RwLock<IdManagementStats>>,
    /// Active file IDs (for tracking)
    active_ids: Arc<RwLock<HashMap<u64, String>>>, // ID -> identifier
}

impl FileIdManager {
    /// Create a new file ID manager
    pub fn new(peer_id: Uuid) -> Self {
        Self {
            peer_id,
            allocated_ranges: Arc::new(RwLock::new(HashMap::new())),
            range_positions: Arc::new(RwLock::new(HashMap::new())),
            reusable_ids: Arc::new(RwLock::new(VecDeque::new())),
            global_next_id: AtomicU64::new(1),
            stats: Arc::new(RwLock::new(IdManagementStats::default())),
            active_ids: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Allocate a new file ID
    pub async fn allocate_id(&self, identifier: String) -> Result<u64, ManagerError> {
        // First try to reuse an existing ID
        if let Some(reusable_id) = self.get_reusable_id().await {
            info!(
                "Reusing file ID {} for identifier '{}'",
                reusable_id.id, identifier
            );

            // Track this ID as active
            {
                let mut active_ids = self.active_ids.write().await;
                active_ids.insert(reusable_id.id, identifier);
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_reused += 1;
                stats.active_ids += 1;
                stats.reuse_efficiency = stats.total_reused as f64 / stats.total_allocated as f64;
            }

            return Ok(reusable_id.id);
        }

        // Try to allocate from our ranges
        if let Some(id) = self.allocate_from_ranges().await {
            info!(
                "Allocated new file ID {} from range for identifier '{}'",
                id, identifier
            );

            // Track this ID as active
            {
                let mut active_ids = self.active_ids.write().await;
                active_ids.insert(id, identifier);
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_allocated += 1;
                stats.active_ids += 1;
            }

            return Ok(id);
        }

        // Fallback to global counter for single-node operation
        let id = self.global_next_id.fetch_add(1, Ordering::SeqCst);
        warn!(
            "Using fallback ID allocation: {} for identifier '{}'",
            id, identifier
        );

        // Track this ID as active
        {
            let mut active_ids = self.active_ids.write().await;
            active_ids.insert(id, identifier);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_allocated += 1;
            stats.active_ids += 1;
        }

        Ok(id)
    }

    /// Release a file ID for potential reuse
    pub async fn release_id(
        &self,
        id: u64,
        reason: IdReleaseReason,
        original_size: u64,
    ) -> Result<(), ManagerError> {
        // Get the original identifier
        let identifier = {
            let mut active_ids = self.active_ids.write().await;
            active_ids
                .remove(&id)
                .unwrap_or_else(|| format!("unknown-{}", id))
        };

        // Add to reusable IDs if we have space
        {
            let mut reusable_ids = self.reusable_ids.write().await;

            // Remove oldest if we're at capacity
            if reusable_ids.len() >= MAX_REUSABLE_IDS {
                if let Some(oldest) = reusable_ids.pop_front() {
                    debug!("Removing oldest reusable ID {} to make space", oldest.id);
                }
            }

            let reusable_id = ReusableId {
                id,
                released_at: chrono::Utc::now(),
                release_reason: reason.clone(),
                original_identifier: identifier.clone(),
                original_size,
            };

            reusable_ids.push_back(reusable_id);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_released += 1;
            stats.active_ids = stats.active_ids.saturating_sub(1);
            stats.reusable_ids_count = self.reusable_ids.read().await.len();
        }

        info!(
            "Released file ID {} (identifier: '{}') for reason: {:?}",
            id, identifier, &reason
        );
        Ok(())
    }

    /// Add an allocated range to this manager
    pub async fn add_range(&self, range: IdRange) -> Result<(), ManagerError> {
        let range_id = range.range_id;

        {
            let mut ranges = self.allocated_ranges.write().await;
            ranges.insert(range_id, range.clone());
        }

        {
            let mut positions = self.range_positions.write().await;
            positions.insert(range_id, AtomicU64::new(range.start));
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.allocated_ranges += 1;
        }

        info!(
            "Added ID range {} to manager: {} - {} (size: {})",
            range_id,
            range.start,
            range.end,
            range.end - range.start + 1
        );

        Ok(())
    }

    /// Request ID allocation from coordinator
    pub async fn request_id_allocation(
        &self,
        count: usize,
        strategy: Option<AllocationStrategy>,
    ) -> Result<IdAllocationResponse, ManagerError> {
        let request = IdAllocationRequest {
            request_id: Uuid::new_v4(),
            peer_id: self.peer_id,
            count,
            preferred_strategy: strategy,
            priority: AllocationPriority::Normal,
        };

        // TODO: Send request to coordination manager
        // For now, create a simple range allocation
        let range_start = self
            .global_next_id
            .fetch_add(DEFAULT_ID_RANGE_SIZE, Ordering::SeqCst);
        let range_end = range_start + DEFAULT_ID_RANGE_SIZE - 1;

        let range = IdRange {
            range_id: Uuid::new_v4(),
            peer_id: self.peer_id,
            start: range_start,
            end: range_end,
            current: range_start,
            strategy: AllocationStrategy::Sequential { base: range_start },
            allocated_at: chrono::Utc::now(),
            active: true,
        };

        // Add this range to our manager
        self.add_range(range.clone()).await?;

        let response = IdAllocationResponse {
            request_id: request.request_id,
            allocated_ids: Vec::new(), // Range allocation, not individual IDs
            allocated_range: Some(range),
            success: true,
            error: None,
        };

        Ok(response)
    }

    /// Get current ID management statistics
    pub async fn get_stats(&self) -> IdManagementStats {
        let mut stats = self.stats.read().await.clone();
        stats.reusable_ids_count = self.reusable_ids.read().await.len();
        stats.allocated_ranges = self.allocated_ranges.read().await.len();
        stats
    }

    /// Get list of allocated ranges
    pub async fn get_allocated_ranges(&self) -> Vec<IdRange> {
        self.allocated_ranges
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get list of reusable IDs
    pub async fn get_reusable_ids(&self) -> Vec<ReusableId> {
        self.reusable_ids.read().await.iter().cloned().collect()
    }

    /// Check if an ID is currently active
    pub async fn is_id_active(&self, id: u64) -> bool {
        self.active_ids.read().await.contains_key(&id)
    }

    /// Get the identifier for an active ID
    pub async fn get_id_identifier(&self, id: u64) -> Option<String> {
        self.active_ids.read().await.get(&id).cloned()
    }

    /// Force cleanup of old reusable IDs
    pub async fn cleanup_old_reusable_ids(&self, max_age_hours: u64) -> usize {
        let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let mut reusable_ids = self.reusable_ids.write().await;
        let original_count = reusable_ids.len();

        // Remove IDs older than cutoff
        reusable_ids.retain(|id| id.released_at > cutoff_time);

        let cleaned_count = original_count - reusable_ids.len();
        if cleaned_count > 0 {
            info!(
                "Cleaned up {} old reusable IDs (older than {} hours)",
                cleaned_count, max_age_hours
            );
        }

        cleaned_count
    }

    /// Get a reusable ID if available
    async fn get_reusable_id(&self) -> Option<ReusableId> {
        let mut reusable_ids = self.reusable_ids.write().await;
        reusable_ids.pop_front()
    }

    /// Allocate an ID from our assigned ranges
    async fn allocate_from_ranges(&self) -> Option<u64> {
        let ranges = self.allocated_ranges.read().await;
        let positions = self.range_positions.read().await;

        for (range_id, range) in ranges.iter() {
            if !range.active {
                continue;
            }

            if let Some(position) = positions.get(range_id) {
                let current = position.load(Ordering::SeqCst);
                if current <= range.end {
                    // Allocate this ID
                    position.store(current + 1, Ordering::SeqCst);
                    return Some(current);
                }
            }
        }

        None
    }
}

// Default is derived above

impl AllocationStrategy {
    /// Create a sequential allocation strategy
    pub fn sequential(base: u64) -> Self {
        AllocationStrategy::Sequential { base }
    }

    /// Create a random allocation strategy
    pub fn random(min: u64, max: u64) -> Self {
        AllocationStrategy::Random { min, max }
    }

    /// Create a hash-based allocation strategy
    pub fn hash_based(peer_id: Uuid, range_size: u64) -> Self {
        AllocationStrategy::HashBased {
            peer_id,
            range_size,
        }
    }

    /// Create a round-robin allocation strategy
    pub fn round_robin(peer_count: u32) -> Self {
        AllocationStrategy::RoundRobin { peer_count }
    }
}

impl IdRange {
    /// Create a new ID range
    pub fn new(peer_id: Uuid, start: u64, end: u64, strategy: AllocationStrategy) -> Self {
        Self {
            range_id: Uuid::new_v4(),
            peer_id,
            start,
            end,
            current: start,
            strategy,
            allocated_at: chrono::Utc::now(),
            active: true,
        }
    }

    /// Get the size of this range
    pub fn size(&self) -> u64 {
        self.end - self.start + 1
    }

    /// Get the number of IDs remaining in this range
    pub fn remaining(&self) -> u64 {
        if self.current > self.end {
            0
        } else {
            self.end - self.current + 1
        }
    }

    /// Check if this range is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.current > self.end
    }

    /// Get usage percentage of this range
    pub fn usage_percentage(&self) -> f64 {
        let used = self.current - self.start;
        let total = self.size();
        if total == 0 {
            100.0
        } else {
            (used as f64 / total as f64) * 100.0
        }
    }
}
