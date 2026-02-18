//! Peer information and status management

use std::time::{Duration, Instant};

/// Current status of a peer server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerStatus {
    /// Peer is healthy and reachable
    Healthy,
    /// Peer is suspected to be unavailable (missed heartbeat)
    Suspected,
    /// Peer is confirmed down
    Down,
}

/// Information about a peer server in the cluster
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Unique identifier for this peer server
    pub server_id: String,

    /// Network address (host:port) for connecting to this peer
    pub address: String,

    /// Current status of this peer
    pub status: PeerStatus,

    /// Timestamp of last successful heartbeat
    pub last_seen: Instant,

    /// Number of consecutive missed heartbeats
    pub missed_heartbeats: u32,

    /// Total bytes received from this peer
    pub bytes_received: u64,

    /// Total bytes sent to this peer
    pub bytes_sent: u64,
}

impl PeerInfo {
    /// Create new peer information
    pub fn new(server_id: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            address: address.into(),
            status: PeerStatus::Healthy,
            last_seen: Instant::now(),
            missed_heartbeats: 0,
            bytes_received: 0,
            bytes_sent: 0,
        }
    }

    /// Check if this peer is considered healthy
    pub fn is_healthy(&self) -> bool {
        self.status == PeerStatus::Healthy
    }

    /// Update last seen time (mark as responding to heartbeat)
    pub fn heartbeat_received(&mut self) {
        self.status = PeerStatus::Healthy;
        self.missed_heartbeats = 0;
        self.last_seen = Instant::now();
    }

    /// Record a missed heartbeat
    pub fn heartbeat_missed(&mut self) {
        self.missed_heartbeats += 1;

        // After 2 missed heartbeats, mark as suspected
        if self.missed_heartbeats >= 2 {
            self.status = PeerStatus::Suspected;
        }

        // After 5 missed heartbeats, mark as down
        if self.missed_heartbeats >= 5 {
            self.status = PeerStatus::Down;
        }
    }

    /// Update bytes received counter
    pub fn add_bytes_received(&mut self, bytes: u64) {
        self.bytes_received = self.bytes_received.saturating_add(bytes);
    }

    /// Update bytes sent counter
    pub fn add_bytes_sent(&mut self, bytes: u64) {
        self.bytes_sent = self.bytes_sent.saturating_add(bytes);
    }

    /// Get time since last heartbeat
    pub fn time_since_heartbeat(&self) -> Duration {
        self.last_seen.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_info_creation() {
        let peer = PeerInfo::new("server_2", "127.0.0.1:9001");
        assert_eq!(peer.server_id, "server_2");
        assert_eq!(peer.address, "127.0.0.1:9001");
        assert_eq!(peer.status, PeerStatus::Healthy);
        assert!(peer.is_healthy());
    }

    #[test]
    fn test_peer_heartbeat_received() {
        let mut peer = PeerInfo::new("server_2", "127.0.0.1:9001");
        peer.missed_heartbeats = 3;
        peer.status = PeerStatus::Suspected;

        peer.heartbeat_received();
        assert_eq!(peer.status, PeerStatus::Healthy);
        assert_eq!(peer.missed_heartbeats, 0);
        assert!(peer.is_healthy());
    }

    #[test]
    fn test_peer_heartbeat_missed() {
        let mut peer = PeerInfo::new("server_2", "127.0.0.1:9001");

        peer.heartbeat_missed();
        assert_eq!(peer.missed_heartbeats, 1);
        assert_eq!(peer.status, PeerStatus::Healthy); // Still healthy

        peer.heartbeat_missed();
        assert_eq!(peer.missed_heartbeats, 2);
        assert_eq!(peer.status, PeerStatus::Suspected); // Now suspected

        for _ in 0..3 {
            peer.heartbeat_missed();
        }
        assert_eq!(peer.status, PeerStatus::Down); // Now down
    }

    #[test]
    fn test_peer_bytes_tracking() {
        let mut peer = PeerInfo::new("server_2", "127.0.0.1:9001");
        assert_eq!(peer.bytes_received, 0);
        assert_eq!(peer.bytes_sent, 0);

        peer.add_bytes_received(1024);
        peer.add_bytes_sent(512);

        assert_eq!(peer.bytes_received, 1024);
        assert_eq!(peer.bytes_sent, 512);
    }

    #[test]
    fn test_peer_time_since_heartbeat() {
        let peer = PeerInfo::new("server_2", "127.0.0.1:9001");
        let elapsed = peer.time_since_heartbeat();
        assert!(elapsed.as_millis() < 100); // Should be nearly 0
    }
}
