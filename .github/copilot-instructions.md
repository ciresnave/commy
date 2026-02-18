# Commy Copilot Instructions

This document provides architectural and design guidance for AI assistants and developers working on the Commy project.

## System Architecture Overview

Commy is a hierarchical, multi-tenant shared memory coordination system for Windows. It enables secure, efficient data sharing between multiple processes and remote clients through a Server-Tenant-Service architecture with optional direct memory-mapping for local clients.

### Hierarchical Structure (Bottom to Top)

```
Layer 1: Service (Foundation)
  ├─ Contains: Shared memory variables, watchers, change detection
  ├─ Responsibility: Manage data within a single shared memory domain
  ├─ File: Random name assigned by Server (e.g., service_[uuid].mem)
  └─ Access: Direct variable allocation/deallocation via SharedData trait

Layer 2: Tenant (Organization)
  ├─ Contains: Multiple Service instances
  ├─ Directory: Server maintains tenant_[name]/ directory
  ├─ Responsibility: Organize services by tenant/domain
  ├─ Authentication: Receives credentials, grants per-Client permissions
  └─ Access: Get/create services by name

Layer 3: Server (Entry Point)
  ├─ Contains: Multiple Tenant instances
  ├─ Protocol: Accepts Client connections via WSS (Secure WebSocket)
  ├─ Responsibility: Host tenants, manage client connections, enforce permissions
  ├─ Knowledge: Maintains private (tenant, service) → filename mapping
  └─ Access: Get/create tenants by name

Layer 4: Client (Consumer)
  ├─ Connects to: Server instance (NEVER directly to Tenant/Service)
  ├─ Authentication: Authenticates to one or more Tenants through Server
  ├─ Access patterns: Remote (WSS) or Local (direct memory-mapping)
  └─ Permissions: Per-Tenant authorization (can differ across tenants)
```

### Filesystem Organization

```
Server Root Directory/
├── tenant_org_a/
│   ├── service_shared_config_[uuid].mem
│   ├── service_task_queue_[uuid].mem
│   └── service_metrics_[uuid].mem
├── tenant_org_b/
│   ├── service_app_state_[uuid].mem
│   └── service_user_cache_[uuid].mem
└── [Service files deleted when last Client disconnects]
```

**Key Properties**:
- Each Tenant has isolated filesystem directory
- Service files have random, cryptographically secure names
- Only Server knows the mapping: (tenant_name, service_name) → filename
- This prevents unauthorized Client file access even if they discover the directory
- Files created on first Client connection, deleted on last Client disconnect

### Connection & Authentication Model

**WSS Connection Phase** (Steps 1-2):
1. Client initiates WSS connection to Server (not to Tenant/Service directly)
2. Server creates Client session, connection remains persistent

**Authentication Phase** (Steps 3-7):
3. Client sends authentication request to named Tenant through Server
4. Tenant applies its pre-configured authentication protocol (API key, mTLS, custom)
5. Tenant validates credentials and decides grant/deny
6. Tenant updates permission set for this (Client, Tenant) pair
7. Server receives updated permissions and enforces them

**Key Point**: Permissions are PER-TENANT, so same Client can have:
- Read-only access to Tenant A
- Admin access to Tenant B
- No access to Tenant C

### Authentication Ownership Model

Commy supports **three authentication modes** that Tenants can choose:

**ServerManaged** (Centralized)
- Server validates credentials using Tenant's stored configuration
- Best for: Small tenants, simple deployments
- Security: Server stores credentials (encrypted), validates directly

**TenantManaged** (Delegated)
- Server forwards auth requests to Tenant's callback endpoint
- Best for: Enterprise tenants with custom auth infrastructure
- Security: Tenant maintains full control over credentials and validation logic

**Hybrid** (Defense-in-Depth)
- Server pre-validates basics, Tenant adds sophisticated verification
- Best for: High-security deployments, multi-factor authentication
- Security: Layered approach with Server baseline + Tenant context-aware checks

**Implementation**:
- `TenantAuthConfig` stores mode per Tenant
- Server routes authentication based on Tenant's chosen mode
- All modes return same result: token + permissions
- SDK clients unaware of mode (transparent to end users)

**Storage Backends**:
- **Memory**: In-memory storage (development only, requires `ENVIRONMENT=development`)
- **PostgreSQL**: Production database storage (`StorageBackend::PostgreSQL { url, max_connections }`)
- **MySQL**: Production database storage (`StorageBackend::MySQL { url, max_connections }`)
- **Redis**: High-performance cache (`StorageBackend::Redis { url }`)

**Critical Rule**: Authentication is ALWAYS initiated through Server, never direct Tenant access

### Client Access Patterns

**Pattern 1: Remote Access via WSS**
- Client connects remotely (different machine)
- Server provides in-memory buffer representation of Service variables
- Client writes: Changes sent as commands over WSS → Server applies via virtual local-Client
- Server broadcasts: Changes from other Clients forwarded to this Client over WSS
- SDK role: Handle buffer synchronization, apply remote changes to local buffer
- Use case: Remote clients, untrusted networks, centralized monitoring

**Pattern 2: Direct Memory-Mapping (Local Only)**
- Client on same machine requests filename for Service from Server
- Server validates permissions and returns filename
- Client directly memory-maps the .mem file
- Zero-copy reads/writes: Data accessed directly from mapped memory
- Use case: Local processes, highest performance, tight integration
- Limitation: Only works for processes on same physical machine

**Pattern 3: Hybrid (Recommended for SDKs)**
- Auto-detect if Client is local or remote
- Use direct mapping if local (best performance)
- Fall back to WSS if remote (only option available)
- Present unified API to application developers

### Service File Lifecycle

**Creation**: When first Client connects to a Service in a Tenant
- Server generates cryptographically random filename
- Creates `tenant_directory/service_[uuid].mem`
- File persists as long as ANY Client is connected

**Deletion**: When last Client disconnects
- Server deletes the file from filesystem
- Exception: Services can keep archival Clients connected to maintain persistence

**Resurrection** (if file was deleted):
- New Client requests Service that previously had a file
- Server queries other Servers in cluster: "Do you have this Service's latest data?"
- If found: Transfer from peer Server and create new file
- If not found: Start fresh (data lost) with new file

### Client Liveness Detection

**Mechanism 1: WSS Connection Drop**
- If WSS connection closes: Client is immediately dead
- Server cleanup: Remove permissions, release resources

**Mechanism 2: Outbound Queue Stall**
- Server monitors outbound message queue size for each Client
- If queue grows but does not shrink in ~30 seconds:
  - Indicates Client dead or network stall
  - Server treats as dead, initiates cleanup
  - Prevents memory exhaustion from queue buildup

## Concurrent Access Model

### Philosophy: Services Own Concurrency

Commy deliberately does NOT enforce per-variable locking:

- **No built-in locks**: No mandatory per-variable mutexes
- **Services choose model**: Each Service decides how to handle concurrent access
- **SDK provides tools**: Client SDKs include optional concurrency helpers
- **Preserve zero-copy**: Avoid forcing locking overhead on all use cases

### Concurrency Options for Services

1. **Eventual consistency** (simplest): Last-write-wins, no coordination
2. **Versioning** (medium): Service tracks versions, SDK resolves conflicts
3. **Distributed locks** (medium): SDK lock manager coordinates across Clients
4. **Transactional batching** (medium): Group variables, atomic updates
5. **CAS primitives** (medium): Service implements Compare-And-Swap
6. **Event log** (complex): All mutations logged, replay for state reconstruction
7. **Custom** (most complex): Service implements domain-specific concurrency

### SDK Concurrency Helpers

Each language SDK provides (optionally):
- Atomic operations (CAS, increment, etc.)
- Versioning utilities
- Distributed lock manager
- Event log/audit trail
- Transactional batchers
- Conflict resolution strategies

**Key**: These are HELPERS, not enforced. Services use what they need.

## Implementation Principles

### Do NOT violate layer boundaries

❌ **WRONG**: Accessing Service directly without going through Tenant
```rust
// This bypasses tenant-level authorization
let service = Service::new("config");
service.allocate_variable(...);
```

✅ **CORRECT**: Access through proper hierarchy
```rust
let mut server = Server::new();
let tenant = server.get_tenant("organization_a");
let service = tenant.get_service("config");
service.allocate_variable(...);
```

### Do NOT mix authentication contexts

❌ **WRONG**: Assuming Client has permission across all Tenants
```rust
// Permission context is per-Tenant, not global
let client_permissions = get_global_perms(client);
```

✅ **CORRECT**: Check permissions per Tenant
```rust
let tenant_a_perms = get_tenant_perms(client, "tenant_a");
let tenant_b_perms = get_tenant_perms(client, "tenant_b");
// These might be different!
```

### Do NOT store raw pointers or Services outside their owner

❌ **WRONG**: Storing Service references globally
```rust
static GLOBAL_SERVICE: LazyLock<Service> = LazyLock::new(|| Service::new(...));
```

✅ **CORRECT**: Access through Server → Tenant → Service chain
```rust
fn get_variable(server: &mut Server, tenant_name: &str, service_name: &str, var_name: &str) {
    let tenant = server.get_tenant(tenant_name);
    let service = tenant.get_service(service_name);
    service.get_variable(var_name)
}
```

### Do NOT assume zero-copy on remote Clients

❌ **WRONG**: Remote Client using same optimization as local
```rust
// Remote clients can't memory-map files!
let mapped_file = map_file(&service_path); // Fails for remote
```

✅ **CORRECT**: Use access pattern appropriate to Client type
```rust
if client_is_local {
    let mapped_file = map_file(&service_path);
} else {
    let buffer = server.request_service_buffer(&service_path);
}
```

## When Adding Features

### Adding a new variable type to a Service

1. Ensure the type implements `SharedData` trait (Copy types only)
2. Add it through `Service::allocate_variable()`
3. Use `detect_changes()` for change notifications
4. Register watchers through `Service::register_watcher()`

### Adding multi-tenant features

1. Think about per-Tenant state requirements
2. Store in Tenant struct, not Server or globally
3. Ensure permissions are checked at Tenant boundary
4. Consider replication strategy if multi-server

### Adding Client authentication

**CRITICAL**: Commy uses **auth-framework** (v0.4+) for all authentication. Do NOT implement custom authentication.

1. Authentication always happens through auth-framework's `AuthFramework`
2. Each Tenant has a `TenantAuthContext` wrapping auth-framework instance
3. Use `auth_framework.authenticate(Credential)` for validation
4. Convert auth-framework scopes to Commy `PermissionSet` via `from_scopes()`
5. Configure storage backend via `TenantAuthConfig.storage_backend`

**Example Authentication Flow**:
```rust
// Create credential from client data
let credential = Credential::jwt(token_string);

// Authenticate via auth-framework
let auth_fw = tenant_auth_context.auth().read().await;
let result = auth_fw.authenticate(credential).await;

match result {
    AuthResult::Success(auth_token) => {
        // Convert scopes to permissions
        let permissions = PermissionSet::from_scopes(&auth_token.scopes);
        // Update session...
    }
    AuthResult::Failure(reason) => {
        // Handle authentication failure
    }
}
```

**Storage Backend Configuration**:
```rust
let config = TenantAuthConfig {
    tenant_id: "my_tenant".to_string(),
    storage_backend: StorageBackend::PostgreSQL {
        url: "postgresql://user:pass@localhost:5432/commy".to_string(),
        max_connections: 100,
    },
    ..Default::default()
};
```

### Working with auth-framework

**Do NOT**:
- ❌ Implement custom token storage (auth-framework manages this)
- ❌ Call `auth_framework.storage()` directly (internal API)
- ❌ Create custom token validation logic
- ❌ Manage token expiration manually

**Do**:
- ✅ Use `auth_framework.authenticate(Credential)` for validation
- ✅ Use `auth_framework.create_auth_token()` for token generation
- ✅ Use `auth_framework.validate_token()` for token checks
- ✅ Configure via `AuthConfig` and `TenantAuthConfig`
- ✅ Let auth-framework handle token lifecycle

**Testing with auth-framework**:
```rust
#[tokio::test]
async fn test_authentication() {
    // Set development environment for memory storage
    unsafe {
        std::env::set_var("ENVIRONMENT", "development");
    }

    let config = AuthConfig::new()
        .token_lifetime(Duration::from_secs(3600))
        .secret("test-secret-at-least-32-chars-long".to_string());

    let mut auth = AuthFramework::new(config);
    let jwt_method = JwtMethod::new()
        .secret_key("test-secret-at-least-32-chars-long")
        .issuer("commy-test");

    auth.register_method("jwt", AuthMethodEnum::Jwt(jwt_method));
    auth.initialize().await.unwrap();

    // Test authentication...
}
```

### Adding concurrency features to SDKs

1. Implement as optional SDK helper (not core library requirement)
2. Services can use or ignore based on needs
3. Do not modify core Commy locking mechanisms
4. Provide examples for common patterns (optimistic versioning, distributed locks)

### Adding Client connection/revocation features

1. Implement WSS message handling in Server
2. Use heartbeat/queue stall detection for liveness
3. On revocation: Create new file, copy data, notify other Clients
4. Keep old file as honeypot (optional)

## Memory Management

### SharedMemory files per Service

- Each Service maintains its own memory-mapped file
- File contains MmapHeader + FreeListAllocator + Variables
- Offset-based pointers enable cross-process access
- Heartbeat mechanism detects dead processes

### Allocation Failure Handling

When allocations fail:
1. Check current file size with `allocator.size()`
2. Check allocation limit with `allocator.allocation_limit()`
3. Call `allocator.resize_file()` with appropriate new size
4. Retry allocation

## Testing Guidelines

### Service Tests
- Test variable allocation/deallocation
- Test change detection
- Test watcher notifications
- Use single Service in isolation

### Tenant Tests
- Test multiple Services within one Tenant
- Test Service creation/lookup by name
- Verify Tenant isolation

### Server Tests
- Test multiple Tenants within one Server
- Test Client authentication to multiple Tenants
- Test per-Tenant authorization

### Client Access Pattern Tests
- Test remote Client via WSS synchronization
- Test local Client direct memory-mapping
- Test hybrid mode auto-detection
- Test permission restrictions prevent unauthorized access

### Integration Tests
- Test full flow: Server → Tenant → Service → Variables
- Test multi-process access patterns
- Test heartbeat/timeout mechanisms
- Test concurrent access with chosen Service concurrency model

### Permission Revocation Tests
- Test file copy on Client removal
- Test all other Clients notified and switched to new file
- Test kicked Client unable to access new file
- Test honeypot detection (optional)

## Critical Design Decisions

### Why offset-based pointers?

Process-specific raw pointers cannot be shared. Offsets are process-agnostic:
- Process A: offset 4096 → address 0x7f1000
- Process B: offset 4096 → address 0x7f3000 (same data!)

### Why separate files per Service?

Independent memory management per Service enables:
- Isolation: Service failures do not affect others
- Scaling: Large services do not impact small ones
- Replication: Services can be replicated independently
- Revocation: Can move Client to new file independently

### Why Tenant layer?

Multi-tenancy requires:
- Permission isolation: Client A cannot see Tenant B's data
- Administrative isolation: Tenant managers cannot interfere
- Billing/quotas: Per-tenant resource limits
- Authentication flexibility: Each Tenant chooses auth protocol

### Why Server as entry point?

Server provides:
- Single authentication point (simpler security model)
- Tenant routing (no client-side routing logic)
- Connection management (handle client disconnects gracefully)
- File name obscurity (only Server knows mapping)

### Why Services own concurrency?

- **Zero-copy preserved**: No mandatory locking overhead
- **Flexibility**: Services with complex needs implement custom solutions
- **Performance**: Simple services do not pay for concurrency infrastructure
- **Simplicity**: Core library stays small and maintainable
- **SDK evolution**: Concurrency features added to SDKs without core changes

### Why WSS instead of raw TCP?

- **Secure**: Built-in encryption and certificate validation
- **Standard**: Wide library support across languages
- **Firewall-friendly**: HTTP upgrade path works through most firewalls
- **Message framing**: Built-in message boundaries

## Common Pitfalls to Avoid

1. **Storing Service references outside their Tenant context**
   - Services must be accessed through tenant.get_service()
   - Do not cache Service pointers across requests

2. **Mixing process-specific pointers with offset calculations**
   - All pointers must be recalculated in each process
   - Use allocator.offset_to_ptr() for reliable conversion

3. **Assuming single-tenant behavior**
   - Always design for multi-tenant scenarios
   - Permissions must be checked at Tenant level

4. **Ignoring heartbeat/timeout mechanisms**
   - Call update_heartbeat() periodically
   - Handle timeout scenarios gracefully

5. **Not validating Tenant/Service names**
   - Treat names as untrusted user input
   - Validate length, characters, and patterns

6. **Assuming remote Clients can memory-map files**
   - Remote Clients must use WSS buffer synchronization
   - Always check Client locality before offering direct mapping

7. **Not implementing concurrency strategy**
   - Document which concurrency model each Service uses
   - Provide SDK helpers for chosen model
   - Do not force one model on all Services

## Performance Expectations

- Single allocation: 35.3 microseconds
- Multi-process throughput: 6,922 ops/sec (8 processes)
- Allocation failure typically means file is full (not a bug)
- WSS overhead: ~1-10ms latency (network dependent)
- Direct memory-mapping: Sub-microsecond access

## Contact & Questions

For architectural questions or clarifications, refer to:
- ARCHITECTURE.md - Full technical design
- USER_GUIDE.md - API reference
- tests/ - Working examples of correct usage
