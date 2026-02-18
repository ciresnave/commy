# Commy Server - Authentication Setup (Session 2 Update)

## ✅ What's Working

- **PostgreSQL**: Running in Docker (`commy-postgres-1` on port 5434)
- **Credentials**: User `commy2_test` / Password `test_password` / Database `commy2_test`
- **Commy Server**: Builds successfully and initializes auth-framework
- **TLS**: Certificates configured (`dev-cert.pem` / `dev-key.pem`)
- **Auth-Framework**: Successfully integrates with PostgreSQL backend

## 🔴 Blocking Issue

**Clustering prevents the WSS listener from starting:**

```
[Commy] Cluster nodes: node-1:9000, node-2:9000, node-3:9000
Error: Os { code: 11001, kind: Uncategorized, message: "No such host is known." }
```

### Quick Fix (Choose One)

#### Fix A: Disable Clustering in Code
Edit `src/main.rs` and comment out cluster initialization for development

#### Fix B: Set Clustering to Single Node
Update cluster configuration to use `127.0.0.1:9000` instead of `node-1:9000`

#### Fix C: Set Environment Variable
(Assuming clustering can be disabled via env var)
```powershell
$env:COMMY_CLUSTERING="disabled"
```

## Current Infrastructure

```
PostgreSQL: commy-postgres-1 (port 5434)
├─ User: commy2_test
├─ Password: test_password
└─ Database: commy2_test

Commy Server: (When fixed)
├─ Listen: wss://127.0.0.1:8443
├─ TLS: dev-cert.pem / dev-key.pem
└─ Tenant: my_tenant

Auth-Framework: PostgreSQL Backend
├─ Database: postgresql://commy2_test:test_password@127.0.0.1:5434/commy2_test
├─ Status: Ready to create tables on first access
└─ Tables: Lazy-created (will be created when first authentication attempt is made)
```

## Next Steps

### 1. Fix Clustering (Critical)
The server cannot initialize WSS listener while clustering configuration fails

### 2. Start Server
```powershell
$env:DATABASE_URL = "postgresql://commy2_test:test_password@127.0.0.1:5434/commy2_test"
$env:COMMY_TLS_CERT_PATH = "dev-cert.pem"
$env:COMMY_TLS_KEY_PATH = "dev-key.pem"
.\target\release\commy.exe
```

### 3. Trigger Auth-Framework Table Creation
Run an example that attempts authentication - this will force auth-framework to create credential tables:
```bash
cargo run --release --example basic_client -- --api-key test_key_123
```

### 4. Discover Table Schema
Once tables are created, inspect them:
```bash
docker exec -e PGPASSWORD=test_password commy-postgres-1 psql -U commy2_test -d commy2_test -c "\dt"
```

### 5. Insert Test Credentials
Use the discovered schema to insert API keys for examples

You should see 4 rows with the test API keys.

### 4. Run Examples

Start the GUI server in another terminal:
```powershell
cd commy-examples
cargo run --release --bin gui_runner -- --port 8080
```

Then open `http://127.0.0.1:8080` in your browser and run the examples.

## Test API Keys

The following API keys are registered for testing:

| Key                        | Permissions                              | Used By                              |
| -------------------------- | ---------------------------------------- | ------------------------------------ |
| `test_key_123`             | read, write                              | basic_client, hybrid_client examples |
| `admin_key_with_all_perms` | admin, manage_tenants, manage_users      | permissions_example (admin user)     |
| `read_only_key`            | read                                     | permissions_example (read-only user) |
| `creator_key`              | admin, create_services, manage_variables | permissions_example (creator user)   |

## Understanding the Authentication Flow

1. **Client connects** to Commy server via WSS (WebSocket Secure)
2. **Client sends** authentication request with:
   - Tenant name: `my_tenant`
   - Auth method: `api_key`
   - Credentials: API key value
3. **Server forwards** request to auth-framework
4. **Auth-framework validates** against PostgreSQL
5. **Server grants** permissions based on scopes
6. **Client can now** access services with those permissions

## Architecture

```
PostgreSQL (5432)
    ↑
    │ (Stores credentials & tokens)
    │
Auth-Framework (in Commy server)
    ↑
    │
WSS Server (8443)
    ↑
    │ (Encrypted WebSocket)
    │
Examples (basic_client, hybrid_client, permissions_example)
```

## Troubleshooting

### "Operation timeout" during authentication
- **Cause**: API keys not registered in PostgreSQL
- **Solution**: Run `init_test_credentials.ps1` (or equivalent for your OS)
- **Verify**: Check PostgreSQL has the credentials with the psql command above

### "Permission denied: Not authenticated"
- **Cause**: API key exists but doesn't have required permissions for the operation
- **Solution**: Ensure you're using the correct API key for the example
- **Example**: 
  - Use `test_key_123` for basic_client (has read, write)
  - Use `admin_key_with_all_perms` for permissions_example (has admin)

### PostgreSQL connection error
- **Cause**: PostgreSQL not running or not accessible
- **Solution**: 
  - Verify Docker container is running: `docker ps | grep postgres`
  - Check port: `netstat -an | find "5432"`
  - Restart container if needed: `docker restart postgres-1`

### "Table not found" in PostgreSQL
- **Cause**: Auth-framework hasn't initialized the schema yet
- **Solution**: The schema is created automatically on first server start
- **Verify**: Run the psql credential check command - it will wait for the schema

## Complete Manual Setup (if scripts don't work)

```bash
# 1. Connect to PostgreSQL
psql -h 127.0.0.1 -U postgres -d commy

# 2. Insert test credentials manually
INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'test_key_123', ARRAY['read', 'write'], true),
  ('api_key', 'admin_key_with_all_perms', ARRAY['admin', 'manage_tenants', 'manage_users'], true),
  ('api_key', 'read_only_key', ARRAY['read'], true),
  ('api_key', 'creator_key', ARRAY['admin', 'create_services', 'manage_variables'], true);

# 3. Verify
SELECT credential_type, credential_value, scopes FROM api_credentials WHERE credential_type = 'api_key';
```

## Environment Variables

The following environment variables are used:

```
COMMY_LISTEN_ADDR=127.0.0.1
COMMY_LISTEN_PORT=8443
COMMY_CLUSTER_ENABLED=false
COMMY_TLS_CERT_PATH=dev-cert.pem
COMMY_TLS_KEY_PATH=dev-key.pem
DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:5432/commy
ENVIRONMENT=production  (set automatically for PostgreSQL backend)
```

## Notes

- All examples use WSS (WebSocket Secure) - TLS is required
- The self-signed certificates (dev-cert.pem, dev-key.pem) are for development only
- Test API keys are plaintext for development - use proper secrets in production
- Auth-framework manages token lifecycle, expiration, and validation
- The `my_tenant` is automatically created on first authentication attempt
