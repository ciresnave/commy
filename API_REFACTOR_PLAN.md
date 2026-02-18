# Commy Client-Server API Refactor: Explicit CRUD Operations

## Objective
Replace implicit create-on-read semantics with explicit CRUD operations at each hierarchy level (Server → Tenant → Service → Variable), enabling:
- Proper permission checking (create ≠ read)
- Clear error differentiation (NotFound vs PermissionDenied vs Unauthorized)
- Auditability (who created what resources)
- No side effects from read operations

## Completed Changes

### 1. Message Protocol (`ClientSDKs/rust-sdk/src/message.rs`)

#### New ClientMessage Variants
```rust
// Tenant-level CRUD
CreateTenant { tenant_id: String, tenant_name: String }
DeleteTenant { tenant_id: String }

// Service-level CRUD (separated from implicit read)
CreateService { tenant_id: String, service_name: String }
GetService { tenant_id: String, service_name: String }  // Error if not found
DeleteService { tenant_id: String, service_name: String }
```

#### New ServerMessage Variants
```rust
// Tenant confirmation
Tenant { tenant_id: String, tenant_name: String }

// New error structure with explicit error codes
Error { code: ErrorCode, message: String }
```

#### New ErrorCode Enum
```rust
pub enum ErrorCode {
    NotFound,              // Service/Tenant doesn't exist
    PermissionDenied,      // No permission for operation
    Unauthorized,          // Invalid authentication
    AlreadyExists,         // Resource already created
    InvalidRequest,        // Bad parameters
    InternalError,         // Server-side error
    ConnectionLost,        // WSS connection failed
    Timeout,               // Operation timed out
}
```

## Pending Changes

### 2. Server Implementation Changes

#### For Server Core (`src/server.rs`)
**Methods to add:**
```rust
// Tenant management
async fn create_tenant(&mut self, name: &str, auth_config: TenantAuthConfig) -> Result<String>
async fn get_tenant(&self, name: &str) -> Result<&Tenant>
async fn delete_tenant(&mut self, name: &str) -> Result<()>

// Permission enforcement
fn check_permission(client: &Client, tenant: &str, permission: Permission) -> Result<()>
```

#### For Tenant (`src/containers/tenant.rs`)
**Methods to add:**
```rust
async fn create_service(&mut self, name: &str) -> Result<Service>
async fn get_service(&self, name: &str) -> Result<Arc<Service>>  // Error if not found
async fn delete_service(&mut self, name: &str) -> Result<()>
```

#### Message Handling (`src/server/handler.rs` or similar)
**Client messages to handle:**
```rust
ClientMessage::CreateTenant { tenant_id, tenant_name } => {
    // Verify client has admin permission
    // Create new tenant
    // Return ServerMessage::Tenant
}

ClientMessage::CreateService { tenant_id, service_name } => {
    // Verify client has create_service permission for tenant
    // Create service
    // Return ServerMessage::Service
}

ClientMessage::GetService { tenant_id, service_name } => {
    // Verify client has read permission for tenant
    // Check if service exists
    // Return ServerMessage::Service or Error { NotFound }
}

ClientMessage::DeleteService { tenant_id, service_name } => {
    // Verify client has admin permission for tenant
    // Delete service
    // Return Result { success: true } or Error { PermissionDenied }
}
```

### 3. Client SDK Changes

#### Client API (`ClientSDKs/rust-sdk/src/client.rs`)
**New methods to add:**
```rust
// Tenant operations (admin)
async fn create_tenant(&self, name: &str) -> Result<String>
async fn delete_tenant(&self, name: &str) -> Result<()>

// Service operations
async fn create_service(&self, tenant_id: &str, name: &str) -> Result<String>
async fn get_service(&self, tenant_id: &str, name: &str) -> Result<Service>
async fn delete_service(&self, tenant_id: &str, name: &str) -> Result<()>
```

#### Error Mapping
Update `CommyError` to handle explicit error codes:
```rust
pub enum CommyError {
    NotFound(String),           // Resource not found
    PermissionDenied(String),   // No permission
    Unauthorized(String),       // Auth failed
    AlreadyExists(String),      // Resource exists
    InvalidRequest(String),     // Bad request
    InternalError(String),      // Server error
    ConnectionLost(String),     // Connection failed
    Timeout(String),            // Operation timeout
}
```

### 4. Examples/Documentation Updates

#### Update Examples
- `examples/basic.rs` - Show explicit create calls before using services
- `examples/permissions.rs` - Demonstrate permission-aware API
- Document permission model in README

#### API Documentation
**Before (implicit create):**
```rust
let service = client.get_service("org_a", "config").await?;
// Side effect: creates service if it doesn't exist
```

**After (explicit):**
```rust
// Create must be explicit
client.create_service("org_a", "config").await?;

// Read is safe - no side effects
let service = client.get_service("org_a", "config").await?;

// Delete is explicit
client.delete_service("org_a", "config").await?;
```

## Implementation Order

1. ✅ **Phase 1: Protocol** (COMPLETED)
   - Update message.rs with new variants and ErrorCode enum
   - Verify SDK compiles

2. **Phase 2: Server Handler** (NEXT)
   - Implement message handlers for CreateTenant/DeleteTenant
   - Implement handlers for CreateService/GetService/DeleteService
   - Add permission checks

3. **Phase 3: Client SDK**
   - Implement new client methods
   - Update error handling to use ErrorCode
   - Update examples

4. **Phase 4: Testing**
   - Update unit tests for new operations
   - Test permission enforcement
   - Test error cases (NotFound, PermissionDenied, etc.)

5. **Phase 5: Documentation**
   - Update API docs
   - Update QUICKSTART.md
   - Update USER_GUIDE.md

## Breaking Changes

This refactor introduces **breaking changes** to the client API:
- `client.get_service()` no longer creates services
- New required `client.create_service()` calls before using services
- Error types now use `ErrorCode` enum instead of string codes
- New tenant CRUD operations added

## Benefits

- ✅ **Permissions**: Separate create/read permissions enforced
- ✅ **Clarity**: No hidden side effects from read operations
- ✅ **Auditability**: Track who creates/deletes resources
- ✅ **Error Handling**: Clear distinction between error types
- ✅ **Principle of Least Privilege**: Clients get minimal required permissions

## Example Usage (Post-Refactor)

```rust
let client = CommyClient::new("ws://localhost:8080");

// Authenticate to tenant
client.authenticate("org_a", credentials).await?;

// Create a service (must be explicit)
client.create_service("org_a", "config").await?;

// Get existing service (safe - no side effects)
match client.get_service("org_a", "config").await {
    Ok(service) => { /* use service */ }
    Err(CommyError::NotFound(_)) => { /* create it first */ }
    Err(CommyError::PermissionDenied(_)) => { /* insufficient permissions */ }
}

// Use service
client.allocate_variable("config", "setting1", vec![]).await?;

// Delete when done
client.delete_service("org_a", "config").await?;
```
