//! Vector Clock Implementation for Multi-Server Consistency
//!
//! Vector clocks enable causality tracking across distributed servers.
//! They detect concurrent events and enable deterministic conflict resolution.
//!
//! # Overview
//!
//! A vector clock is a logical clock that assigns a vector of integers to each event.
//! Each server maintains one component of the vector, incremented when it performs an operation.
//!
//! ## Properties
//!
//! - **Causality Detection**: If VC(A) < VC(B), then A happened before B
//! - **Concurrency Detection**: If VC(A) || VC(B), then A and B are concurrent
//! - **Server Identification**: Each server has a unique ID in the vector
//!
//! ## Usage
//!
//! ```ignore
//! let mut vc = VectorClock::new(&["server1", "server2", "server3"]);
//! vc.increment("server1");  // Happens on server1
//! let vc_copy = vc.clone();
//! vc.increment("server2");  // Happens on server2
//! assert!(vc.happens_before(&vc_copy).is_none());  // Concurrent
//! ```

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

/// A vector clock for tracking causality in distributed systems.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorClock {
    /// Timestamp for each server (server_id -> logical_time)
    clocks: HashMap<String, u64>,
    /// All known server IDs in deterministic order
    server_ids: Vec<String>,
}

impl VectorClock {
    /// Create a new vector clock with the given server IDs.
    ///
    /// # Arguments
    ///
    /// * `server_ids` - List of all server IDs in the cluster (must be sorted)
    pub fn new(server_ids: &[&str]) -> Self {
        let mut clocks = HashMap::new();
        let server_ids: Vec<String> = server_ids.iter().map(|s| s.to_string()).collect();

        for server_id in &server_ids {
            clocks.insert(server_id.clone(), 0);
        }

        VectorClock { clocks, server_ids }
    }

    /// Increment the clock for a specific server.
    ///
    /// # Arguments
    ///
    /// * `server_id` - The server that performed the operation
    ///
    /// # Panics
    ///
    /// Panics if the server_id is not in the vector clock's known servers.
    pub fn increment(&mut self, server_id: &str) {
        if let Some(clock) = self.clocks.get_mut(server_id) {
            *clock += 1;
        } else {
            panic!("Unknown server_id: {}", server_id);
        }
    }

    /// Merge with another vector clock (take maximum of each component).
    ///
    /// This is used when receiving an event from another server.
    pub fn merge(&mut self, other: &VectorClock) {
        for server_id in &self.server_ids {
            let other_time = other.clocks.get(server_id).copied().unwrap_or(0);
            let self_time = self.clocks.get(server_id).copied().unwrap_or(0);
            self.clocks
                .insert(server_id.clone(), self_time.max(other_time));
        }
    }

    /// Get the timestamp for a specific server.
    pub fn get(&self, server_id: &str) -> u64 {
        self.clocks.get(server_id).copied().unwrap_or(0)
    }

    /// Check if this vector clock happens before another.
    ///
    /// Returns:
    /// - `Some(Ordering::Less)` if self < other (self happened before)
    /// - `Some(Ordering::Greater)` if self > other (self happened after)
    /// - `Some(Ordering::Equal)` if self == other (same event)
    /// - `None` if clocks are concurrent (neither happened before the other)
    pub fn compare(&self, other: &VectorClock) -> Option<Ordering> {
        let mut has_less = false;
        let mut has_greater = false;

        for server_id in &self.server_ids {
            let self_time = self.clocks.get(server_id).copied().unwrap_or(0);
            let other_time = other.clocks.get(server_id).copied().unwrap_or(0);

            match self_time.cmp(&other_time) {
                Ordering::Less => has_less = true,
                Ordering::Greater => has_greater = true,
                Ordering::Equal => {}
            }
        }

        match (has_less, has_greater) {
            (false, false) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Less),
            (false, true) => Some(Ordering::Greater),
            (true, true) => None, // Concurrent
        }
    }

    /// Check if this vector clock happened before another.
    pub fn happens_before(&self, other: &VectorClock) -> Option<bool> {
        self.compare(other).map(|ord| ord == Ordering::Less)
    }

    /// Check if this vector clock happened after another.
    pub fn happens_after(&self, other: &VectorClock) -> Option<bool> {
        self.compare(other).map(|ord| ord == Ordering::Greater)
    }

    /// Check if clocks are concurrent (neither happened before the other).
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        self.compare(other).is_none()
    }

    /// Get all server IDs known to this clock.
    pub fn server_ids(&self) -> &[String] {
        &self.server_ids
    }

    /// Get the vector as a sorted list of (server_id, time) tuples.
    ///
    /// Useful for display and debugging.
    pub fn as_vec(&self) -> Vec<(String, u64)> {
        let mut result: Vec<_> = self.clocks.iter().map(|(k, v)| (k.clone(), *v)).collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
}

/// Represents a versioned value with its vector clock.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionedValue<T: Clone> {
    /// The actual value
    pub value: T,
    /// Vector clock at the time this value was created/updated
    pub version: VectorClock,
    /// Which server last modified this value
    pub last_modified_by: String,
}

impl<T: Clone> VersionedValue<T> {
    /// Create a new versioned value.
    pub fn new(value: T, version: VectorClock, last_modified_by: String) -> Self {
        VersionedValue {
            value,
            version,
            last_modified_by,
        }
    }

    /// Check if this version happened before another.
    pub fn happens_before(&self, other: &VersionedValue<T>) -> Option<bool> {
        self.version.happens_before(&other.version)
    }

    /// Check if this version is concurrent with another.
    pub fn is_concurrent(&self, other: &VersionedValue<T>) -> bool {
        self.version.is_concurrent(&other.version)
    }
}

/// Conflict resolution strategy for concurrent writes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictResolution<T: Clone> {
    /// Keep the value that has higher sum of vector clock components
    /// Serves as a tie-breaker for concurrent updates
    HighestLogicalTime,

    /// Keep the value from the server with lexicographically highest ID
    /// Only used when logical time is equal
    ServerPriority,

    /// User-defined resolution: return both conflicting values
    /// Application must decide which to keep
    ApplicationDefined(Vec<T>),
}

/// Result of applying a remote write locally.
#[derive(Clone, Debug)]
pub enum WriteResult<T: Clone> {
    /// Remote write happened after local, apply it
    Applied,

    /// Local write happened after remote, keep local
    Ignored,

    /// Both writes are concurrent
    Conflict(Vec<T>),
}

/// Merge function type for resolving conflicts.
pub type MergeFn<T> = fn(&VersionedValue<T>, &VersionedValue<T>) -> VersionedValue<T>;

/// Applies a remote value update to a local value, respecting causality.
///
/// # Arguments
///
/// * `local` - Current local value
/// * `remote` - Remote value received
/// * `resolve_conflict` - Function to resolve concurrent writes
///
/// # Returns
///
/// Returns the new local value and whether it was changed.
pub fn apply_remote_write<T: Clone>(
    local: &VersionedValue<T>,
    remote: &VersionedValue<T>,
    resolve_conflict: MergeFn<T>,
) -> (VersionedValue<T>, bool) {
    match local.version.compare(&remote.version) {
        Some(Ordering::Less) => {
            // Remote happened after local, apply it
            (remote.clone(), true)
        }
        Some(Ordering::Greater) | Some(Ordering::Equal) => {
            // Local happened after or is same, keep local
            (local.clone(), false)
        }
        None => {
            // Concurrent, resolve conflict
            let resolved = resolve_conflict(local, remote);
            (resolved, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_creation() {
        let vc = VectorClock::new(&["server1", "server2", "server3"]);
        assert_eq!(vc.get("server1"), 0);
        assert_eq!(vc.get("server2"), 0);
        assert_eq!(vc.get("server3"), 0);
    }

    #[test]
    fn test_vector_clock_increment() {
        let mut vc = VectorClock::new(&["server1", "server2"]);
        vc.increment("server1");
        assert_eq!(vc.get("server1"), 1);
        assert_eq!(vc.get("server2"), 0);

        vc.increment("server1");
        assert_eq!(vc.get("server1"), 2);
    }

    #[test]
    fn test_vector_clock_happens_before() {
        let mut vc1 = VectorClock::new(&["server1", "server2"]);
        let mut vc2 = VectorClock::new(&["server1", "server2"]);

        vc1.increment("server1");
        vc1.merge(&vc2);
        vc2.increment("server1");
        vc2.increment("server2");

        assert_eq!(vc1.happens_before(&vc2), Some(true));
        assert_eq!(vc2.happens_before(&vc1), Some(false));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let mut vc1 = VectorClock::new(&["server1", "server2"]);
        let mut vc2 = VectorClock::new(&["server1", "server2"]);

        vc1.increment("server1");
        vc2.increment("server2");

        assert!(vc1.is_concurrent(&vc2));
        assert!(vc2.is_concurrent(&vc1));
        assert_eq!(vc1.compare(&vc2), None);
    }

    #[test]
    fn test_vector_clock_merge() {
        let mut vc1 = VectorClock::new(&["server1", "server2"]);
        let mut vc2 = VectorClock::new(&["server1", "server2"]);

        vc1.increment("server1");
        vc1.increment("server1");
        vc2.increment("server2");

        vc1.merge(&vc2);

        assert_eq!(vc1.get("server1"), 2);
        assert_eq!(vc1.get("server2"), 1);
    }

    #[test]
    fn test_vector_clock_equality() {
        let mut vc1 = VectorClock::new(&["server1", "server2"]);
        let mut vc2 = VectorClock::new(&["server1", "server2"]);

        vc1.increment("server1");
        vc2.increment("server1");

        assert_eq!(vc1, vc2);
        assert_eq!(vc1.compare(&vc2), Some(Ordering::Equal));
    }

    #[test]
    fn test_versioned_value() {
        let vc = VectorClock::new(&["server1", "server2"]);
        let v = VersionedValue::new(42, vc, "server1".to_string());

        assert_eq!(v.value, 42);
        assert_eq!(v.last_modified_by, "server1");
    }

    #[test]
    fn test_apply_remote_write_happens_before() {
        let mut local_vc = VectorClock::new(&["server1", "server2"]);
        let mut remote_vc = VectorClock::new(&["server1", "server2"]);

        local_vc.increment("server1");
        local_vc.merge(&remote_vc);
        remote_vc.increment("server1");
        remote_vc.increment("server2");

        let local = VersionedValue::new(10, local_vc, "server1".to_string());
        let remote = VersionedValue::new(20, remote_vc, "server2".to_string());

        let resolve = |_l: &VersionedValue<i32>, r: &VersionedValue<i32>| r.clone();

        let (result, changed) = apply_remote_write(&local, &remote, resolve);
        assert!(changed);
        assert_eq!(result.value, 20);
    }

    #[test]
    fn test_apply_remote_write_happens_after() {
        let mut local_vc = VectorClock::new(&["server1", "server2"]);
        let mut remote_vc = VectorClock::new(&["server1", "server2"]);

        remote_vc.increment("server1");
        local_vc.increment("server1");
        local_vc.increment("server2");

        let local = VersionedValue::new(10, local_vc, "server2".to_string());
        let remote = VersionedValue::new(20, remote_vc, "server1".to_string());

        let resolve = |_l: &VersionedValue<i32>, r: &VersionedValue<i32>| r.clone();

        let (result, changed) = apply_remote_write(&local, &remote, resolve);
        assert!(!changed);
        assert_eq!(result.value, 10);
    }

    #[test]
    fn test_apply_remote_write_concurrent() {
        let mut local_vc = VectorClock::new(&["server1", "server2"]);
        let mut remote_vc = VectorClock::new(&["server1", "server2"]);

        local_vc.increment("server1");
        remote_vc.increment("server2");

        let local = VersionedValue::new(10, local_vc, "server1".to_string());
        let remote = VersionedValue::new(20, remote_vc, "server2".to_string());

        let resolve = |_l: &VersionedValue<i32>, r: &VersionedValue<i32>| r.clone();

        let (result, changed) = apply_remote_write(&local, &remote, resolve);
        assert!(changed);
        assert_eq!(result.value, 20); // Resolution chose remote
    }

    #[test]
    fn test_vector_clock_as_vec() {
        let mut vc = VectorClock::new(&["server1", "server2", "server3"]);
        vc.increment("server1");
        vc.increment("server3");

        let vec = vc.as_vec();
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], ("server1".to_string(), 1));
        assert_eq!(vec[1], ("server2".to_string(), 0));
        assert_eq!(vec[2], ("server3".to_string(), 1));
    }

    #[test]
    fn test_causality_chain() {
        let mut vc1 = VectorClock::new(&["server1", "server2"]);
        let mut vc2 = VectorClock::new(&["server1", "server2"]);
        let mut vc3 = VectorClock::new(&["server1", "server2"]);

        // vc1: server1 increments
        vc1.increment("server1");

        // vc2: server2 receives vc1, then increments
        vc2.merge(&vc1);
        vc2.increment("server2");

        // vc3: server1 receives vc2, then increments
        vc3.merge(&vc2);
        vc3.increment("server1");

        // Verify causality chain
        assert_eq!(vc1.happens_before(&vc2), Some(true));
        assert_eq!(vc2.happens_before(&vc3), Some(true));
        assert_eq!(vc1.happens_before(&vc3), Some(true));
    }

    #[test]
    fn test_multiple_concurrent_events() {
        let mut vc1 = VectorClock::new(&["server1", "server2", "server3"]);
        let mut vc2 = VectorClock::new(&["server1", "server2", "server3"]);
        let mut vc3 = VectorClock::new(&["server1", "server2", "server3"]);

        vc1.increment("server1");
        vc2.increment("server2");
        vc3.increment("server3");

        assert!(vc1.is_concurrent(&vc2));
        assert!(vc2.is_concurrent(&vc3));
        assert!(vc1.is_concurrent(&vc3));
    }
}
