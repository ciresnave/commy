# Phase 6: Production Hardening & Server Clustering

## Objective
Enable multiple Commy servers to operate in a cluster, with service state replication and client failover capabilities.

## Architecture Overview

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│  Client A       │         │  Client B       │         │  Client C       │
└────────┬────────┘         └────────┬────────┘         └────────┬────────┘
         │                           │                           │
         │ WSS                       │ WSS                       │ WSS
         │                           │                           │
    ┌────▼──────────────────────────▼──────────────────────────▼────┐
    │                                                                 │
    │  ┌─────────────┐  Peer Link  ┌─────────────┐  Peer Link  ┌─────────────┐
    │  │  Server 1   │◄────────────►│  Server 2   │◄────────────►│  Server 3   │
    │  │             │              │             │              │             │
    │  │  tenant_org │              │  tenant_org │              │  tenant_org │
    │  │  ├─ service │              │  ├─ service │              │  ├─ service │
    │  │  └─ state   │              │  └─ state   │              │  └─ state   │
    │  └─────────────┘              └─────────────┘              └─────────────┘
    │
    │  Registry (local or consul)
    └───────────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 6a: Server Registry & Peer Discovery
**Goal**: Servers know about each other and can form a cluster

- [ ] Server registration mechanism
- [ ] Peer discovery (static list or service registry)
- [ ] Heartbeat/health checks between peers
- [ ] Dynamic peer join/leave handling

**Files**:
- `src/server/clustering/registry.rs` (NEW)
- `src/server/clustering/peer.rs` (NEW)
- `src/server/clustering/mod.rs` (NEW)

### Phase 6b: Inter-Server Communication Protocol
**Goal**: Define and implement server-to-server message protocol

- [ ] Server-to-server message types (StateRequest, StateTransfer, etc.)
- [ ] TCP/WebSocket connection between peers
- [ ] Request/response handling
- [ ] Timeout and retry logic

**Files**:
- `src/protocol/server_protocol.rs` (NEW)
- `src/server/clustering/connection.rs` (NEW)
- `src/server/clustering/messages.rs` (NEW)

### Phase 6c: Service State Replication
**Goal**: Transfer service files between servers

- [ ] Fetch service state from peer server
- [ ] Transfer service file (or checkpoint)
- [ ] Verify checksum
- [ ] Handle partial transfers and resume

**Files**:
- `src/server/clustering/replication.rs` (NEW)
- `src/server/clustering/transfer.rs` (NEW)

### Phase 6d: Client Failover Detection
**Goal**: Detect server failures and reconnect clients automatically

- [ ] Monitor connection health
- [ ] Detect server unavailability
- [ ] Queue stall detection (already implemented)
- [ ] Graceful degradation

**Files**:
- `src/server/liveness.rs` (EXPAND)
- `ClientSDKs/rust-sdk/src/client.rs` (UPDATE)

### Phase 6e: Session Persistence Across Servers
**Goal**: Client can reconnect to different server and restore session

- [ ] Store session data in shared state
- [ ] Identify client by session_id
- [ ] Restore permissions on new server
- [ ] Handle concurrent server changes

**Files**:
- `src/server/session_manager.rs` (EXPAND)
- `src/server/state.rs` (EXPAND)

### Phase 6f: Consistency & Conflict Resolution
**Goal**: Handle concurrent writes from multiple servers

- [ ] Vector clocks for ordering
- [ ] Conflict detection
- [ ] Conflict resolution strategy (last-write-wins, custom)
- [ ] Consistency checks

**Files**:
- `src/server/clustering/consistency.rs` (NEW)

### Phase 6g: Testing & Integration
**Goal**: Comprehensive tests for clustering scenarios

- [ ] Unit tests for peer discovery
- [ ] Integration tests for service replication
- [ ] Failover tests
- [ ] Consistency tests
- [ ] Performance tests

**Files**:
- `src/server/clustering/tests.rs` (NEW)
- `tests/clustering_integration_tests.rs` (NEW)

### Phase 6h: Configuration & Deployment
**Goal**: Configure clustering for different deployment scenarios

- [ ] Configuration schema (servers list, registry info)
- [ ] Environment variables for clustering
- [ ] Deployment documentation
- [ ] Docker Compose for multi-node testing

**Files**:
- `src/config.rs` (EXPAND)
- `docker-compose.yml` (NEW)
- `CLUSTERING_GUIDE.md` (NEW)

---

## Detailed Implementation Plan

### Phase 6a: Server Registry & Peer Discovery

#### 1. Peer Registry Structure
```rust
pub struct PeerRegistry {
    local_server_id: String,
    local_address: String,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    heartbeat_interval: Duration,
}

pub struct PeerInfo {
    server_id: String,
    address: String,
    last_seen: Instant,
    is_healthy: bool,
}
```

#### 2. Discovery Methods
- **Static**: Load peers from config file
- **Dynamic**: Consul/etcd integration
- **Multicast**: UDP broadcast (optional)

#### 3. Heartbeat Mechanism
- Ping-pong between peers every 30 seconds
- Mark peer unhealthy after 3 missed pings
- Remove peer after 5 minutes offline

### Phase 6b: Inter-Server Communication

#### 1. Message Types
```rust
pub enum ServerMessage {
    // Cluster management
    Heartbeat { server_id: String },
    PeerDiscovery { servers: Vec<ServerInfo> },
    
    // Service state
    StateRequest { tenant_id: String, service_id: String },
    StateTransfer { tenant_id: String, service_id: String, data: Vec<u8> },
    StateChecksum { tenant_id: String, service_id: String, checksum: u64 },
    
    // Acknowledgements
    Ack { request_id: String },
    Nak { request_id: String, reason: String },
}
```

#### 2. Connection Management
- Maintain long-lived TCP connections between peers
- Reuse connections for multiple requests
- Reconnect on connection failure
- Exponential backoff for reconnection

### Phase 6c: Service State Replication

#### 1. State Transfer Flow
```
Client connects to Server A
Server A hosts service_1
Client connects to Server B (Server A fails)
  → Server B checks if it has service_1
  → If not, request from Server A (or other peers)
  → Transfer service file to Server B
  → Client resumes reading/writing on Server B
```

#### 2. Transfer Protocol
- Checksum verification (CRC32 or SHA256)
- Chunked transfer (1MB chunks)
- Resume capability for interrupted transfers
- Compression option for large files

### Phase 6d: Client Failover

#### 1. Failover Trigger
- Connection timeout (no heartbeat received)
- Explicit server close
- Network error
- Server unavailability detected

#### 2. Failover Logic
```
Client detects server failure
  → Stop sending to failed server
  → Buffer outgoing messages
  → Reconnect to next available server
  → Restore session (permissions, subscriptions)
  → Flush buffered messages
  → Resume normal operation
```

### Phase 6e: Session Persistence

#### 1. Session Storage
- Store session state in shared location (file or database)
- Include: session_id, tenant_id, permissions, subscriptions
- TTL: 24 hours after last activity

#### 2. Session Restoration
- Client reconnects with session_id
- New server loads session from storage
- Verify permissions still valid
- Restore subscriptions

### Phase 6f: Consistency Management

#### 1. Vector Clocks
- Each server maintains logical clock
- Increment on local write
- Update on message from peer
- Used to order concurrent writes

#### 2. Conflict Resolution
- Timestamp-based: newer write wins
- Custom: application-defined rules
- Callback: notify app of conflicts

### Phase 6g: Testing Strategy

#### Test Scenarios
1. **Peer Discovery**: Servers find each other
2. **Heartbeat**: Detect failures and recoveries
3. **State Replication**: Transfer service files
4. **Client Failover**: 
   - Server goes down → client reconnects
   - Client on server A → connects to server B
   - Verify same service accessible
5. **Consistency**:
   - Two clients write simultaneously to different servers
   - Verify no data corruption
   - Verify one write wins (deterministically)
6. **Performance**:
   - 1000 clients across 3 servers
   - Measure failover time
   - Measure state transfer bandwidth

---

## Implementation Order (Recommended)

1. **Phase 6a** (Week 1): Registry & peer discovery
   - Static peer list first
   - Heartbeat mechanism
   - Tests

2. **Phase 6b** (Week 1): Inter-server protocol
   - Message types
   - Connection pool
   - Request/response handling

3. **Phase 6c** (Week 2): Service state replication
   - State transfer
   - Checksum verification
   - Resume capability

4. **Phase 6d** (Week 2): Client failover
   - Failure detection
   - Automatic reconnect
   - Buffer management

5. **Phase 6e** (Week 3): Session persistence
   - Session storage
   - Session restoration
   - TTL management

6. **Phase 6f** (Week 3): Consistency
   - Vector clocks
   - Conflict detection
   - Resolution strategy

7. **Phase 6g** (Week 4): Integration testing
   - Multi-server tests
   - Failover scenarios
   - Performance tests

8. **Phase 6h** (Week 4): Configuration & deployment
   - Config schema
   - Docker Compose
   - Documentation

---

## Success Criteria

✅ **Functional**:
- Servers form cluster and exchange heartbeats
- Service state transfers between servers
- Client can reconnect to different server
- Session persists across server changes
- Concurrent writes handled consistently

✅ **Non-Functional**:
- Failover time < 5 seconds
- State transfer: 100MB/s
- Session restoration: < 100ms
- Zero data loss
- Support 1000+ clients

✅ **Operational**:
- Configuration via YAML/TOML
- Metrics and logging for debugging
- Docker support
- Graceful degradation (partial cluster failure)

---

## Risk Mitigation

| Risk                          | Mitigation                         |
| ----------------------------- | ---------------------------------- |
| Split-brain (servers diverge) | Vector clocks + timestamp ordering |
| Stale session data            | TTL on sessions, verify on restore |
| Network partition             | Detect and go read-only            |
| Large state transfer          | Compression + chunking + resume    |
| Concurrent writes             | Last-write-wins + audit trail      |

---

## Deliverables

- [ ] Clustering module (6 new files, ~2000 LOC)
- [ ] Protocol definitions (1 new file, ~200 LOC)
- [ ] Configuration system (updated, ~100 LOC)
- [ ] Integration tests (3 new files, ~1500 LOC)
- [ ] Documentation (3 guides, ~1000 words)
- [ ] Docker Compose example
- [ ] 50+ new tests, all passing

---

## Ready to Begin?

Starting with **Phase 6a: Server Registry & Peer Discovery** would establish the foundation for the clustering system. This includes:

1. Creating the clustering module structure
2. Implementing peer registry
3. Adding heartbeat mechanism
4. Writing comprehensive tests

Should I proceed with Phase 6a implementation?
