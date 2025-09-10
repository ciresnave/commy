//! Node Registry Module
//!
//! Manages node registration, heartbeats, and node-to-node communication
//! in the distributed mesh.

use crate::mesh::mesh_coordinator::{MeshNode, NodeStatus};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Node heartbeat information
#[derive(Debug, Clone)]
pub struct NodeHeartbeat {
    /// Node ID
    pub node_id: Uuid,

    /// Heartbeat timestamp
    pub timestamp: Instant,

    /// Node load information
    pub load_info: NodeLoadInfo,

    /// Active services count
    pub active_services: u32,

    /// Node health status
    pub health_status: NodeHealthStatus,
}

/// Node load information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLoadInfo {
    /// CPU usage percentage (0.0 to 100.0)
    pub cpu_usage: f64,

    /// Memory usage percentage (0.0 to 100.0)
    pub memory_usage: f64,

    /// Network usage in Mbps
    pub network_usage_mbps: f64,

    /// Disk usage percentage (0.0 to 100.0)
    pub disk_usage: f64,

    /// Active connections count
    pub active_connections: u32,

    /// Request queue length
    pub queue_length: u32,
}

/// Node health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeHealthStatus {
    Healthy,
    Warning,
    Critical,
    Unreachable,
}

/// Node registration request
#[derive(Debug, Clone)]
pub struct NodeRegistrationRequest {
    /// Node information
    pub node: MeshNode,

    /// Registration metadata
    pub metadata: RegistrationMetadata,
}

/// Registration metadata
#[derive(Debug, Clone)]
pub struct RegistrationMetadata {
    /// Registration timestamp
    pub registered_at: Instant,

    /// Node version
    pub version: String,

    /// Node tags
    pub tags: Vec<String>,

    /// Additional properties
    pub properties: HashMap<String, String>,
}

/// Node registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRegistryConfig {
    /// Heartbeat interval
    pub heartbeat_interval: Duration,

    /// Node timeout (after missing heartbeats)
    pub node_timeout: Duration,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Maximum nodes allowed
    pub max_nodes: usize,

    /// Enable node authentication
    pub require_authentication: bool,

    /// Node synchronization interval
    pub sync_interval: Duration,
}

/// Node registry statistics
#[derive(Debug, Clone, Default)]
pub struct NodeRegistryStats {
    /// Total registered nodes
    pub total_nodes: usize,

    /// Active nodes
    pub active_nodes: usize,

    /// Inactive nodes
    pub inactive_nodes: usize,

    /// Failed nodes
    pub failed_nodes: usize,

    /// Total heartbeats received
    pub total_heartbeats: u64,

    /// Average heartbeat interval
    pub avg_heartbeat_interval_ms: f64,

    /// Last registry sync
    pub last_sync: Option<Instant>,
}

/// Node registry events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeRegistryEvent {
    NodeRegistered {
        node_id: Uuid,
        node_name: String,
    },
    NodeUnregistered {
        node_id: Uuid,
        reason: String,
    },
    NodeStatusChanged {
        node_id: Uuid,
        old_status: NodeStatus,
        new_status: NodeStatus,
    },
    NodeHealthChanged {
        node_id: Uuid,
        old_health: NodeHealthStatus,
        new_health: NodeHealthStatus,
    },
    HeartbeatReceived {
        node_id: Uuid,
    },
    HeartbeatMissed {
        node_id: Uuid,
        consecutive_misses: u32,
    },
}

/// Node registry for managing mesh nodes
pub struct NodeRegistry {
    /// Configuration
    config: NodeRegistryConfig,

    /// Registered nodes
    nodes: Arc<RwLock<HashMap<Uuid, MeshNode>>>,

    /// Node registration metadata
    node_metadata: Arc<RwLock<HashMap<Uuid, RegistrationMetadata>>>,

    /// Node heartbeat history
    heartbeat_history: Arc<RwLock<HashMap<Uuid, Vec<NodeHeartbeat>>>>,

    /// Registry statistics
    stats: Arc<RwLock<NodeRegistryStats>>,

    /// Event listeners (for notifications)
    event_listeners: Arc<RwLock<Vec<tokio::sync::mpsc::UnboundedSender<NodeRegistryEvent>>>>,
}

impl NodeRegistry {
    /// Create a new node registry
    pub fn new(config: NodeRegistryConfig) -> Self {
        Self {
            config,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            node_metadata: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_history: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(NodeRegistryStats::default())),
            event_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start the node registry
    pub async fn start(&self) -> Result<()> {
        self.start_heartbeat_monitor().await;
        self.start_health_monitor().await;
        self.start_node_synchronization().await;
        self.start_cleanup_task().await;

        tracing::info!("Node registry started");
        Ok(())
    }

    /// Register a new node
    pub async fn register_node(&self, request: NodeRegistrationRequest) -> Result<()> {
        let node_id = request.node.node_id;

        // Check if registry is full
        {
            let nodes = self.nodes.read().await;
            if nodes.len() >= self.config.max_nodes {
                return Err(anyhow!(
                    "Node registry is full (max: {})",
                    self.config.max_nodes
                ));
            }
        }

        // Register the node
        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(node_id, request.node.clone());
        }

        // Store metadata
        {
            let mut metadata = self.node_metadata.write().await;
            metadata.insert(node_id, request.metadata);
        }

        // Initialize heartbeat history
        {
            let mut history = self.heartbeat_history.write().await;
            history.insert(node_id, Vec::new());
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_nodes = self.nodes.read().await.len();
            if request.node.status == NodeStatus::Active {
                stats.active_nodes += 1;
            }
        }

        // Send event
        self.send_event(NodeRegistryEvent::NodeRegistered {
            node_id,
            node_name: request.node.name.clone(),
        })
        .await;

        tracing::info!("Node registered: {} ({})", request.node.name, node_id);
        Ok(())
    }

    /// Unregister a node
    pub async fn unregister_node(&self, node_id: Uuid, reason: String) -> Result<()> {
        let node = {
            let mut nodes = self.nodes.write().await;
            nodes.remove(&node_id)
        };

        if let Some(node) = node {
            // Remove metadata
            {
                let mut metadata = self.node_metadata.write().await;
                metadata.remove(&node_id);
            }

            // Remove heartbeat history
            {
                let mut history = self.heartbeat_history.write().await;
                history.remove(&node_id);
            }

            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_nodes = self.nodes.read().await.len();
                if node.status == NodeStatus::Active {
                    stats.active_nodes = stats.active_nodes.saturating_sub(1);
                }
            }

            // Send event
            self.send_event(NodeRegistryEvent::NodeUnregistered {
                node_id,
                reason: reason.clone(),
            })
            .await;

            tracing::info!("Node unregistered: {} ({}): {}", node.name, node_id, reason);
            Ok(())
        } else {
            Err(anyhow!("Node not found: {}", node_id))
        }
    }

    /// Process node heartbeat
    pub async fn process_heartbeat(&self, heartbeat: NodeHeartbeat) -> Result<()> {
        let node_id = heartbeat.node_id;

        // Update node last seen
        {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(&node_id) {
                node.last_seen = heartbeat.timestamp;

                // Update node health status based on heartbeat
                let old_health = self.get_node_health_status(node).await;
                let new_health = self.calculate_health_status(&heartbeat.load_info);

                if old_health != new_health {
                    self.send_event(NodeRegistryEvent::NodeHealthChanged {
                        node_id,
                        old_health,
                        new_health,
                    })
                    .await;
                }
            } else {
                return Err(anyhow!("Heartbeat received for unknown node: {}", node_id));
            }
        }

        // Store heartbeat in history
        {
            let mut history = self.heartbeat_history.write().await;
            if let Some(node_history) = history.get_mut(&node_id) {
                node_history.push(heartbeat.clone());

                // Keep only last 100 heartbeats
                if node_history.len() > 100 {
                    node_history.remove(0);
                }
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_heartbeats += 1;

            // Calculate average heartbeat interval
            let history_read = self.heartbeat_history.read().await;
            if let Some(node_history) = history_read.get(&node_id) {
                if node_history.len() >= 2 {
                    let intervals: Vec<_> = node_history
                        .windows(2)
                        .map(|w| w[1].timestamp.duration_since(w[0].timestamp).as_millis() as f64)
                        .collect();

                    if !intervals.is_empty() {
                        let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
                        stats.avg_heartbeat_interval_ms = avg_interval;
                    }
                }
            }
        }

        // Send event
        self.send_event(NodeRegistryEvent::HeartbeatReceived { node_id })
            .await;

        tracing::debug!("Heartbeat processed for node: {}", node_id);
        Ok(())
    }

    /// Get node by ID
    pub async fn get_node(&self, node_id: Uuid) -> Option<MeshNode> {
        let nodes = self.nodes.read().await;
        nodes.get(&node_id).cloned()
    }

    /// Get all nodes
    pub async fn get_all_nodes(&self) -> Vec<MeshNode> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// Get nodes by status
    pub async fn get_nodes_by_status(&self, status: NodeStatus) -> Vec<MeshNode> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|node| node.status == status)
            .cloned()
            .collect()
    }

    /// Get node heartbeat history
    pub async fn get_node_heartbeat_history(&self, node_id: Uuid) -> Vec<NodeHeartbeat> {
        let history = self.heartbeat_history.read().await;
        history.get(&node_id).cloned().unwrap_or_default()
    }

    /// Get registry statistics
    pub async fn get_stats(&self) -> NodeRegistryStats {
        let mut stats = self.stats.read().await.clone();

        // Update current counts
        let nodes = self.nodes.read().await;
        stats.total_nodes = nodes.len();
        stats.active_nodes = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Active)
            .count();
        stats.inactive_nodes = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Inactive)
            .count();
        stats.failed_nodes = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Failed)
            .count();

        stats
    }

    /// Add event listener
    pub async fn add_event_listener(
        &self,
        sender: tokio::sync::mpsc::UnboundedSender<NodeRegistryEvent>,
    ) {
        let mut listeners = self.event_listeners.write().await;
        listeners.push(sender);
    }

    /// Update node status
    pub async fn update_node_status(&self, node_id: Uuid, new_status: NodeStatus) -> Result<()> {
        let old_status = {
            let mut nodes = self.nodes.write().await;
            if let Some(node) = nodes.get_mut(&node_id) {
                let old = node.status.clone();
                node.status = new_status.clone();
                old
            } else {
                return Err(anyhow!("Node not found: {}", node_id));
            }
        };

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            match (&old_status, &new_status) {
                (NodeStatus::Active, _) => {
                    stats.active_nodes = stats.active_nodes.saturating_sub(1)
                }
                (_, NodeStatus::Active) => stats.active_nodes += 1,
                _ => {}
            }
        }

        // Send event
        let new_status_for_log = new_status.clone();

        self.send_event(NodeRegistryEvent::NodeStatusChanged {
            node_id,
            old_status,
            new_status,
        })
        .await;

        tracing::info!(
            "Node status updated: {} -> {:?}",
            node_id,
            new_status_for_log
        );
        Ok(())
    }

    /// Get node health status
    async fn get_node_health_status(&self, _node: &MeshNode) -> NodeHealthStatus {
        // In a real implementation, this would check various health metrics
        NodeHealthStatus::Healthy
    }

    /// Calculate health status from load info
    fn calculate_health_status(&self, load_info: &NodeLoadInfo) -> NodeHealthStatus {
        // Simple health calculation based on resource usage
        if load_info.cpu_usage > 90.0 || load_info.memory_usage > 95.0 {
            NodeHealthStatus::Critical
        } else if load_info.cpu_usage > 75.0 || load_info.memory_usage > 85.0 {
            NodeHealthStatus::Warning
        } else {
            NodeHealthStatus::Healthy
        }
    }

    /// Send event to all listeners
    async fn send_event(&self, event: NodeRegistryEvent) {
        let listeners = self.event_listeners.read().await;
        for sender in listeners.iter() {
            let _ = sender.send(event.clone()); // Ignore errors for closed channels
        }
    }

    /// Start heartbeat monitoring task
    async fn start_heartbeat_monitor(&self) {
        let nodes = Arc::clone(&self.nodes);
        let _heartbeat_history = Arc::clone(&self.heartbeat_history);
        let event_listeners = Arc::clone(&self.event_listeners);
        let node_timeout = self.config.node_timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let now = Instant::now();
                let mut failed_nodes = Vec::new();

                // Check for nodes that haven't sent heartbeats
                {
                    let nodes_read = nodes.read().await;
                    for (node_id, node) in nodes_read.iter() {
                        let time_since_last_seen = now.duration_since(node.last_seen);

                        if time_since_last_seen > node_timeout && node.status == NodeStatus::Active
                        {
                            failed_nodes.push(*node_id);
                        }
                    }
                }

                // Mark failed nodes
                if !failed_nodes.is_empty() {
                    let mut nodes_write = nodes.write().await;
                    for node_id in failed_nodes {
                        if let Some(node) = nodes_write.get_mut(&node_id) {
                            let old_status = node.status.clone();
                            node.status = NodeStatus::Failed;

                            // Send event
                            let event = NodeRegistryEvent::NodeStatusChanged {
                                node_id,
                                old_status,
                                new_status: NodeStatus::Failed,
                            };

                            let listeners = event_listeners.read().await;
                            for sender in listeners.iter() {
                                let _ = sender.send(event.clone());
                            }

                            tracing::warn!(
                                "Node marked as failed due to missed heartbeats: {}",
                                node_id
                            );
                        }
                    }
                }
            }
        });
    }

    /// Start health monitoring task
    async fn start_health_monitor(&self) {
        let heartbeat_history = Arc::clone(&self.heartbeat_history);
        let health_check_interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(health_check_interval);

            loop {
                interval.tick().await;

                let history_read = heartbeat_history.read().await;
                for (node_id, heartbeats) in history_read.iter() {
                    if let Some(latest_heartbeat) = heartbeats.last() {
                        let health_status = if latest_heartbeat.load_info.cpu_usage > 90.0
                            || latest_heartbeat.load_info.memory_usage > 95.0
                        {
                            NodeHealthStatus::Critical
                        } else if latest_heartbeat.load_info.cpu_usage > 75.0
                            || latest_heartbeat.load_info.memory_usage > 85.0
                        {
                            NodeHealthStatus::Warning
                        } else {
                            NodeHealthStatus::Healthy
                        };

                        tracing::debug!("Node {} health: {:?}", node_id, health_status);
                    }
                }
            }
        });
    }

    /// Start node synchronization task
    async fn start_node_synchronization(&self) {
        let stats = Arc::clone(&self.stats);
        let sync_interval = self.config.sync_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(sync_interval);

            loop {
                interval.tick().await;

                {
                    let mut stats_write = stats.write().await;
                    stats_write.last_sync = Some(Instant::now());
                }

                tracing::debug!("Node registry synchronization completed");
            }
        });
    }

    /// Start cleanup task
    async fn start_cleanup_task(&self) {
        let heartbeat_history = Arc::clone(&self.heartbeat_history);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                let now = Instant::now();
                let retention_period = Duration::from_secs(3600); // 1 hour

                // Clean up old heartbeat history
                {
                    let mut history_write = heartbeat_history.write().await;
                    for heartbeats in history_write.values_mut() {
                        heartbeats.retain(|hb| now.duration_since(hb.timestamp) < retention_period);
                    }
                }

                tracing::debug!("Node registry cleanup completed");
            }
        });
    }
}
