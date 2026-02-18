# Commy Clustering: Deployment Guide

This guide covers deploying Commy in a multi-server clustered configuration for high availability, fault tolerance, and distributed consistency.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start](#quick-start)
3. [Configuration](#configuration)
4. [Deployment Options](#deployment-options)
5. [Monitoring & Observability](#monitoring--observability)
6. [Troubleshooting](#troubleshooting)
7. [Best Practices](#best-practices)

## Architecture Overview

Commy clustering enables multiple servers to share state through:

- **Vector Clocks**: Logical timestamps tracking causality across servers
- **Conflict Resolution**: Deterministic Last-Write-Wins strategy with server ID tie-breaking
- **Gossip Replication**: Background eventual consistency mechanism
- **Client Failover**: Automatic reconnection to healthy servers
- **Session Persistence**: Session state survives server failures

### Deployment Models

#### Single Server (Development)
```
┌─────────────┐
│  Commy      │
│  Server 1   │
└─────────────┘
```

#### 3-Node Cluster (High Availability)
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Commy      │────▶│  Commy      │────▶│  Commy      │
│  Server 1   │     │  Server 2   │     │  Server 3   │
└─────────────┘     └─────────────┘     └─────────────┘
      ▲                     ▲                     ▲
      └─────────────────────┴─────────────────────┘
           Gossip Replication (bi-directional)
```

#### 5+ Node Cluster (Enterprise)
```
┌─────────────────────────────────────────────────────┐
│         Multi-Region Deployment with PostgreSQL      │
├─────────────────────────────────────────────────────┤
│  Region 1              Region 2         Region 3    │
│  [S1] [S2]      [S3] [S4]       [S5]                │
│    Gossip           Gossip         Sync             │
│  Replication      Replication   Replicas            │
└─────────────────────────────────────────────────────┘
            ▼
        PostgreSQL (Shared Metadata)
```

## Quick Start

### Local 3-Node Cluster (Docker Compose)

1. **Build and start the cluster:**

```bash
cd commy
docker-compose -f docker-compose.cluster.yml up -d
```

2. **Verify cluster health:**

```bash
# Check server 1
curl http://localhost:8001/health

# Check server 2
curl http://localhost:8002/health

# Check server 3
curl http://localhost:8003/health
```

3. **Create a test tenant:**

```bash
curl -X POST http://localhost:8001/api/tenants \
  -H "Content-Type: application/json" \
  -d '{"name": "test-tenant"}'
```

4. **Create a service in the tenant:**

```bash
curl -X POST http://localhost:8001/api/tenants/test-tenant/services \
  -H "Content-Type: application/json" \
  -d '{"name": "shared-state"}'
```

5. **Write data to the service:**

```bash
curl -X POST http://localhost:8001/api/tenants/test-tenant/services/shared-state/variables \
  -H "Content-Type: application/json" \
  -d '{"name": "counter", "value": 42}'
```

6. **Read from another server (automatic replication):**

```bash
curl http://localhost:8002/api/tenants/test-tenant/services/shared-state/variables/counter
# Should return: 42
```

7. **Observe cluster information:**

```bash
curl http://localhost:8001/api/cluster/status
```

## Configuration

### Configuration File Format

Commy uses YAML for cluster configuration. See example files in `examples/`:

- `cluster-3-node-local.yaml` - Local development (3 nodes, in-memory)
- `cluster-5-node-prod.yaml` - Production (5 nodes, PostgreSQL, TLS)

### Core Configuration Options

#### Server Identity

```yaml
server_id: "server-1"  # Unique per server in cluster
```

#### Node List

```yaml
nodes:
  - id: "server-1"
    address: "10.0.1.10:9001"
    max_connections: 20
    tls:
      cert_path: "/etc/commy/certs/server-1.crt"
      key_path: "/etc/commy/certs/server-1.key"
      ca_path: "/etc/commy/certs/ca.crt"
      verify_peer: true
```

#### Replication Settings

```yaml
replication:
  sync_interval_ms: 50        # How often to sync (lower = more consistent)
  batch_size: 2000            # Variables per replication message
  max_concurrent_tasks: 8     # Parallel replication streams
  gossip_enabled: true        # Eventual consistency via gossip
  replication_timeout_ms: 10000  # Timeout for peer responses
  enable_checksums: true      # Verify data integrity
  enable_resume: true         # Resume interrupted transfers
```

#### Consistency Strategy

```yaml
consistency:
  strategy:
    last_write_wins:          # Deterministic conflict resolution
      tie_breaker: null       # null = use alphabetical server order
  enable_vector_clocks: true  # Causality tracking
  quorum_size: 3              # Optional: require N/5 servers (null = no quorum)
  detect_conflicts: true      # Log conflicting writes
  conflict_log_enabled: true
  conflict_log_path: "/var/log/commy/conflicts.log"
```

#### Network & Heartbeat

```yaml
network:
  heartbeat_interval_ms: 500     # How often to ping peers
  heartbeat_timeout_ms: 2000     # Mark peer dead if no response
  max_heartbeat_failures: 2      # Failures before marking dead
  connection_pooling: true       # Reuse connections
  max_queue_size: 50000          # Backpressure threshold
  tcp_keepalive: true
  tcp_keepalive_ms: 10000
```

#### Storage Backend

```yaml
storage:
  backend:
    postgresql:
      url: "postgresql://user:pass@host:5432/db"
      max_connections: 20
  persist_cluster_state: true
  metadata_dir: "/var/lib/commy/cluster"
  enable_snapshots: true
  snapshot_interval_ms: 30000
```

## Deployment Options

### Option 1: Docker Compose (Development/Testing)

**Best for:** Local testing, CI/CD, single-machine clusters

```bash
# 3-node local cluster
docker-compose -f docker-compose.cluster.yml up -d

# View logs
docker-compose -f docker-compose.cluster.yml logs -f commy-1

# Scale down
docker-compose -f docker-compose.cluster.yml down
```

**Ports:**
- `8001-8003`: Client API (WSS)
- `9001-9003`: Inter-server replication
- `5432`: PostgreSQL
- `8080`: Adminer (DB admin)

### Option 2: Kubernetes Deployment

**Best for:** Production, multi-region, auto-scaling

**Example StatefulSet:**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: commy
spec:
  serviceName: commy
  replicas: 3
  selector:
    matchLabels:
      app: commy
  template:
    metadata:
      labels:
        app: commy
    spec:
      containers:
      - name: commy
        image: commy:latest
        ports:
        - containerPort: 8000  # Client API
          name: api
        - containerPort: 9000  # Inter-server
          name: replication
        env:
        - name: COMMY_SERVER_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: COMMY_CLUSTER_ENABLED
          value: "true"
        - name: COMMY_LISTEN_ADDR
          value: "0.0.0.0:8000"
        - name: COMMY_BIND_ADDR
          value: "$(COMMY_SERVER_ID).commy:9000"
        volumeMounts:
        - name: config
          mountPath: /etc/commy
        - name: data
          mountPath: /var/lib/commy
      volumes:
      - name: config
        configMap:
          name: commy-config
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: [ "ReadWriteOnce" ]
      resources:
        requests:
          storage: 10Gi
---
apiVersion: v1
kind: Service
metadata:
  name: commy
spec:
  clusterIP: None  # Headless
  selector:
    app: commy
  ports:
  - port: 9000
    name: replication
```

### Option 3: Manual Deployment (Bare Metal/VMs)

**Best for:** Custom infrastructure, specific performance tuning

1. **Build Commy:**

```bash
cargo build --release
# Binary at: target/release/commy
```

2. **Create configuration files:**

```bash
mkdir -p /etc/commy
cp examples/cluster-5-node-prod.yaml /etc/commy/cluster.yaml
```

3. **Set up TLS certificates:**

```bash
mkdir -p /etc/commy/certs
# Copy your certificates here (or use Let's Encrypt)
```

4. **Create systemd service:**

```ini
[Unit]
Description=Commy Server
After=network.target

[Service]
Type=simple
User=commy
ExecStart=/usr/local/bin/commy --config /etc/commy/cluster.yaml
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

5. **Start services:**

```bash
sudo systemctl enable commy
sudo systemctl start commy
sudo systemctl status commy
```

## Monitoring & Observability

### Health Checks

**Endpoint:** `GET /health`

```bash
curl http://localhost:8001/health
```

**Response:**
```json
{
  "status": "healthy",
  "cluster": {
    "server_id": "server-1",
    "peer_count": 2,
    "healthy_peers": 2,
    "replication_lag_ms": 5
  }
}
```

### Cluster Status

**Endpoint:** `GET /api/cluster/status`

```bash
curl http://localhost:8001/api/cluster/status
```

**Response:**
```json
{
  "server_id": "server-1",
  "status": "healthy",
  "peers": [
    {
      "id": "server-2",
      "status": "healthy",
      "last_heartbeat_ms": 250,
      "replicated_services": 15
    },
    {
      "id": "server-3",
      "status": "healthy",
      "last_heartbeat_ms": 120,
      "replicated_services": 15
    }
  ],
  "replication_metrics": {
    "pending_syncs": 0,
    "active_transfers": 0,
    "total_replicated_bytes": 1048576
  }
}
```

### Metrics & Logging

**Enable detailed logging:**

```bash
export RUST_LOG=commy=debug,commy::clustering=trace
./commy --config cluster.yaml
```

**Log Levels:**
- `error`: Failures and cluster issues
- `warn`: Heartbeat timeouts, replication delays
- `info`: Server startup, peer joins/leaves
- `debug`: Message exchanges, conflict detection
- `trace`: Vector clock operations, gossip merges

### Conflict Logging

When enabled, conflicts are logged to a separate file:

```bash
tail -f /var/log/commy/conflicts.log
```

**Log Format:**
```
timestamp=2026-02-14T10:30:45Z server=server-1 variable=counter tenant=org-a service=metrics
  local: value=100 clock={server-1:5, server-2:3} timestamp=1707899445000
  remote: value=101 clock={server-1:4, server-2:4} timestamp=1707899444999
  resolved: value=100 (local wins, server-1 > server-2 lexicographically)
```

## Troubleshooting

### Symptom: Servers Not Communicating

**Check:** Heartbeat connectivity

```bash
# On server-1, check if it can reach server-2
curl -v telnet://server-2:9001

# Check firewall rules
sudo ufw status
sudo ufw allow 9001:9003/tcp  # Allow inter-server ports
```

**Check:** Configuration

```bash
# Verify node addresses match actual deployments
cat /etc/commy/cluster.yaml | grep address:
```

**Fix:** 
1. Ensure all nodes are reachable from each other
2. Verify TLS certificates if enabled
3. Check DNS resolution for hostnames

### Symptom: High Replication Lag

**Check:** Network bandwidth

```bash
iperf3 -c server-2 -t 10  # Test bandwidth
```

**Check:** Replication settings in config

```yaml
replication:
  sync_interval_ms: 50   # Reduce if lag is acceptable
  batch_size: 2000       # Increase for faster bulk transfers
  max_concurrent_tasks: 8 # Increase if CPU allows
```

**Fix:**
1. Increase `sync_interval_ms` for more frequent syncs
2. Increase `batch_size` for larger messages
3. Verify database performance if using PostgreSQL backend
4. Check network MTU: `ip link | grep mtu`

### Symptom: Split-Brain (Servers Isolated)

**Detection:**
- Health check shows only self
- Cluster status shows `status: "degraded"`

**Recovery:**
1. Check network connectivity between nodes
2. Review firewall/security group rules
3. Once reconnected, nodes automatically sync via gossip protocol
4. Monitor `replication_metrics.pending_syncs` to completion

**Prevention:**
1. Use quorum-based writes (set `quorum_size: 3` for 5-node cluster)
2. Monitor network connectivity continuously
3. Use watchdog timers for automatic failover

### Symptom: Service Not Replicating

**Check:** Service configuration

```bash
curl http://localhost:8001/api/tenants/{tenant}/services/{service}
```

**Check:** Replication enabled

```yaml
replication:
  gossip_enabled: true  # Ensure true
  sync_interval_ms: 100 # Should be reasonable
```

**Check:** Peer connectivity

```bash
curl http://localhost:8001/api/cluster/status
```

**Fix:**
1. Create service on a node with healthy peers
2. Verify service is allocated before expecting replication
3. Check conflict logs for errors

### Symptom: Out of Memory

**Check:** Allocation limits

```bash
# Query allocation info per service
curl http://localhost:8001/api/tenants/{tenant}/services/{service}/stats
```

**Check:** Configuration limits

```yaml
storage:
  backend:
    postgresql:
      max_connections: 20  # Reduce if too many
```

**Fix:**
1. Monitor allocation usage via metrics
2. Implement allocation limits per service
3. Archive old data to separate storage
4. Increase available memory or reduce batch_size

## Best Practices

### 1. Cluster Sizing

| Deployment        | Nodes | Use Case     | Failure Tolerance |
| ----------------- | ----- | ------------ | ----------------- |
| Development       | 1     | Testing      | None              |
| High Availability | 3     | Production   | 1 server          |
| Enterprise        | 5+    | Multi-region | 2+ servers        |

### 2. Configuration Tuning

**For Consistency (Lower Latency):**
```yaml
replication:
  sync_interval_ms: 50     # Sync every 50ms
  batch_size: 2000         # Larger batches
  max_concurrent_tasks: 8  # More parallelism

consistency:
  quorum_size: 3           # 3-of-5 write quorum (slower but safer)
```

**For Availability (Higher Throughput):**
```yaml
replication:
  sync_interval_ms: 200    # Sync every 200ms (eventual consistency)
  batch_size: 5000         # Even larger batches
  
consistency:
  quorum_size: null        # No quorum (fastest writes)
```

**For Durability:**
```yaml
consistency:
  detect_conflicts: true
  conflict_log_enabled: true
  conflict_log_path: "/var/log/commy/conflicts.log"

storage:
  persist_cluster_state: true
  enable_snapshots: true
  snapshot_interval_ms: 30000
```

### 3. Monitoring Checklist

- [ ] Health endpoint responding on all nodes
- [ ] Cluster status shows all peers as "healthy"
- [ ] Replication lag < 1 second
- [ ] Memory usage stable (not growing)
- [ ] No conflicts in conflict log (or expected application-level conflicts)
- [ ] All peers have same view of cluster membership

### 4. Deployment Checklist

- [ ] All servers have unique `server_id`
- [ ] All servers can reach each other on replication ports
- [ ] PostgreSQL (or other backend) is accessible
- [ ] TLS certificates are valid (if enabled)
- [ ] Sufficient disk space for metadata and snapshots
- [ ] Logging is configured for troubleshooting
- [ ] Automated backups of PostgreSQL configured
- [ ] Monitoring/alerting connected to health endpoints

### 5. Upgrade Procedure

1. **Prepare:** Test upgrade in staging cluster first
2. **Rolling upgrade:** Upgrade nodes one at a time:
   ```bash
   # Stop node 1
   systemctl stop commy
   
   # Deploy new binary
   cp /tmp/commy-new /usr/local/bin/commy
   
   # Start node 1
   systemctl start commy
   
   # Wait for rejoin and sync
   curl http://localhost:8001/api/cluster/status
   
   # Repeat for nodes 2, 3, ...
   ```
3. **Verify:** Cluster should remain operational during upgrade

### 6. Backup & Recovery

**Backup cluster state:**
```bash
# PostgreSQL backup
pg_dump -U commy_user -d commy_cluster > backup.sql

# Metadata directory
tar -czf commy_metadata.tar.gz /var/lib/commy/cluster
```

**Recovery:**
```bash
# Stop all nodes
systemctl stop commy

# Restore PostgreSQL
psql -U commy_user -d commy_cluster < backup.sql

# Restore metadata (optional)
tar -xzf commy_metadata.tar.gz -C /var/lib/commy

# Start nodes
systemctl start commy
```

## Summary

Commy clustering provides production-grade distributed state management with:

- ✅ Automatic failover
- ✅ Eventual consistency via gossip protocol
- ✅ Deterministic conflict resolution
- ✅ Vector clock causality tracking
- ✅ Flexible deployment (Docker, Kubernetes, bare metal)
- ✅ Comprehensive monitoring
- ✅ Multiple storage backends

For more information, see:
- `ARCHITECTURE.md` - Technical design details
- `src/server/clustering/config.rs` - Configuration types
- `examples/` - Example configurations
