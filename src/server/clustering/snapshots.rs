//! Service state snapshots for replication
//!
//! Provides serialization and deserialization of service memory state for transfer between servers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Snapshot of a service's state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSnapshot {
    /// Tenant name
    pub tenant_name: String,

    /// Service name
    pub service_name: String,

    /// Version number of this snapshot
    pub version: u64,

    /// Unix timestamp when snapshot was taken (ms since epoch)
    pub timestamp: u64,

    /// Serialized service data (bytes from memory)
    pub data: Vec<u8>,

    /// SHA256 checksum of the data
    pub checksum: String,

    /// Total size of service data in bytes
    pub total_size: u64,

    /// Whether this is an incremental snapshot (vs full)
    pub is_incremental: bool,

    /// If incremental, the version range included
    pub version_range: Option<(u64, u64)>,

    /// Metadata about the service
    pub metadata: SnapshotMetadata,
}

/// Metadata about a service snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Number of variables in the service
    pub variable_count: usize,

    /// Total bytes used by service (including overhead)
    pub memory_used: u64,

    /// List of variable names
    pub variable_names: Vec<String>,

    /// Server ID that created this snapshot
    pub source_server: String,

    /// Optional custom metadata
    pub custom: HashMap<String, String>,
}

impl ServiceSnapshot {
    /// Create a new service snapshot
    pub fn new(
        tenant_name: String,
        service_name: String,
        version: u64,
        data: Vec<u8>,
        checksum: String,
    ) -> Self {
        let timestamp = Self::current_timestamp();
        let total_size = data.len() as u64;

        Self {
            tenant_name,
            service_name,
            version,
            timestamp,
            data,
            checksum,
            total_size,
            is_incremental: false,
            version_range: None,
            metadata: SnapshotMetadata {
                variable_count: 0,
                memory_used: total_size,
                variable_names: vec![],
                source_server: String::new(),
                custom: HashMap::new(),
            },
        }
    }

    /// Create an incremental snapshot
    pub fn incremental(
        tenant_name: String,
        service_name: String,
        from_version: u64,
        to_version: u64,
        data: Vec<u8>,
        checksum: String,
    ) -> Self {
        let timestamp = Self::current_timestamp();
        let total_size = data.len() as u64;

        Self {
            tenant_name,
            service_name,
            version: to_version,
            timestamp,
            data,
            checksum,
            total_size,
            is_incremental: true,
            version_range: Some((from_version, to_version)),
            metadata: SnapshotMetadata {
                variable_count: 0,
                memory_used: total_size,
                variable_names: vec![],
                source_server: String::new(),
                custom: HashMap::new(),
            },
        }
    }

    /// Get current Unix timestamp in milliseconds
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Verify checksum matches data
    pub fn verify_checksum(&self) -> bool {
        let calculated = Self::calculate_checksum(&self.data);
        calculated == self.checksum
    }

    /// Calculate SHA256 checksum of data
    pub fn calculate_checksum(data: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Update metadata
    pub fn with_metadata(mut self, metadata: SnapshotMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Update source server
    pub fn with_source(mut self, server_id: String) -> Self {
        self.metadata.source_server = server_id;
        self
    }

    /// Get snapshot size in bytes
    pub fn size(&self) -> u64 {
        self.total_size
    }

    /// Get snapshot version
    pub fn version(&self) -> u64 {
        self.version
    }
}

/// Snapshot transfer state for resumable transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotTransfer {
    /// Unique transfer ID
    pub transfer_id: String,

    /// Snapshot being transferred
    pub snapshot: ServiceSnapshot,

    /// Current transfer progress in bytes
    pub bytes_transferred: u64,

    /// Total bytes to transfer
    pub total_bytes: u64,

    /// List of successfully transferred chunks
    pub completed_chunks: Vec<u64>,

    /// Last chunk offset
    pub last_chunk_offset: u64,

    /// Transfer start timestamp
    pub started_at: u64,

    /// Last activity timestamp
    pub last_activity: u64,

    /// Whether transfer is complete
    pub is_complete: bool,
}

impl SnapshotTransfer {
    /// Create a new transfer
    pub fn new(transfer_id: String, snapshot: ServiceSnapshot) -> Self {
        let now = Self::current_timestamp();
        let total_bytes = snapshot.size();

        Self {
            transfer_id,
            snapshot,
            bytes_transferred: 0,
            total_bytes,
            completed_chunks: vec![],
            last_chunk_offset: 0,
            started_at: now,
            last_activity: now,
            is_complete: false,
        }
    }

    /// Get current timestamp
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Mark chunk as transferred
    pub fn mark_chunk_transferred(&mut self, offset: u64, size: u64) {
        self.bytes_transferred += size;
        self.completed_chunks.push(offset);
        self.last_chunk_offset = offset;
        self.last_activity = Self::current_timestamp();

        if self.bytes_transferred >= self.total_bytes {
            self.is_complete = true;
        }
    }

    /// Get progress percentage (0-100)
    pub fn progress_percent(&self) -> u32 {
        if self.total_bytes == 0 {
            100
        } else {
            ((self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0).round() as u32
        }
    }

    /// Check if transfer is stalled (no activity for duration)
    pub fn is_stalled(&self, max_inactive_ms: u64) -> bool {
        let now = Self::current_timestamp();
        (now - self.last_activity) > max_inactive_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let data = b"test data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data.clone(),
            checksum,
        );

        assert_eq!(snapshot.tenant_name, "tenant1");
        assert_eq!(snapshot.service_name, "service1");
        assert_eq!(snapshot.version, 1);
        assert!(!snapshot.is_incremental);
        assert!(snapshot.verify_checksum());
    }

    #[test]
    fn test_checksum_calculation() {
        let data = b"test data".to_vec();
        let checksum1 = ServiceSnapshot::calculate_checksum(&data);
        let checksum2 = ServiceSnapshot::calculate_checksum(&data);

        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum1.len(), 64); // SHA256 is 64 hex characters
    }

    #[test]
    fn test_snapshot_verification() {
        let data = b"test data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let mut snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data,
            checksum,
        );

        assert!(snapshot.verify_checksum());

        // Corrupt checksum
        snapshot.checksum = "invalid_checksum".to_string();
        assert!(!snapshot.verify_checksum());
    }

    #[test]
    fn test_incremental_snapshot() {
        let data = b"incremental data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let snapshot = ServiceSnapshot::incremental(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            2,
            data,
            checksum,
        );

        assert!(snapshot.is_incremental);
        assert_eq!(snapshot.version_range, Some((1, 2)));
    }

    #[test]
    fn test_snapshot_serialization() {
        let data = b"test data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data,
            checksum,
        );

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: ServiceSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.tenant_name, snapshot.tenant_name);
        assert_eq!(deserialized.version, snapshot.version);
    }

    #[test]
    fn test_snapshot_transfer_creation() {
        let data = b"test data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data,
            checksum,
        );

        let transfer = SnapshotTransfer::new("transfer_1".to_string(), snapshot.clone());

        assert_eq!(transfer.transfer_id, "transfer_1");
        assert_eq!(transfer.bytes_transferred, 0);
        assert_eq!(transfer.total_bytes, snapshot.size());
        assert!(!transfer.is_complete);
    }

    #[test]
    fn test_snapshot_transfer_progress() {
        let data = b"test data 12345678".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data,
            checksum,
        );

        let mut transfer = SnapshotTransfer::new("transfer_1".to_string(), snapshot.clone());

        transfer.mark_chunk_transferred(0, 5);
        assert_eq!(transfer.progress_percent(), 28); // 5/18 * 100 = 27.78 -> rounds to 28%

        transfer.mark_chunk_transferred(5, 13);
        assert!(transfer.is_complete);
        assert_eq!(transfer.progress_percent(), 100);
    }

    #[test]
    fn test_snapshot_transfer_stall_detection() {
        let data = b"test data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data,
            checksum,
        );

        let transfer = SnapshotTransfer::new("transfer_1".to_string(), snapshot);

        // Should not be stalled immediately
        assert!(!transfer.is_stalled(5000));
    }

    #[test]
    fn test_snapshot_with_metadata() {
        let data = b"test data".to_vec();
        let checksum = ServiceSnapshot::calculate_checksum(&data);

        let metadata = SnapshotMetadata {
            variable_count: 10,
            memory_used: 1024,
            variable_names: vec!["var1".to_string(), "var2".to_string()],
            source_server: "server1".to_string(),
            custom: HashMap::new(),
        };

        let snapshot = ServiceSnapshot::new(
            "tenant1".to_string(),
            "service1".to_string(),
            1,
            data,
            checksum,
        )
        .with_metadata(metadata);

        assert_eq!(snapshot.metadata.variable_count, 10);
        assert_eq!(snapshot.metadata.source_server, "server1");
    }

    #[test]
    fn test_snapshot_with_source() {
        let snapshot = ServiceSnapshot::new(
            "tenant".to_string(),
            "service".to_string(),
            1,
            vec![1, 2, 3],
            "abc".to_string(),
        ).with_source("server_42".to_string());

        assert_eq!(snapshot.metadata.source_server, "server_42");
    }

    #[test]
    fn test_snapshot_size() {
        let data = vec![0u8; 256];
        let snapshot = ServiceSnapshot::new(
            "tenant".to_string(),
            "service".to_string(),
            1,
            data,
            "abc".to_string(),
        );
        assert_eq!(snapshot.size(), 256);
    }

    #[test]
    fn test_snapshot_version() {
        let snapshot = ServiceSnapshot::new(
            "tenant".to_string(),
            "service".to_string(),
            42,
            vec![],
            "abc".to_string(),
        );
        assert_eq!(snapshot.version(), 42);
    }
}
