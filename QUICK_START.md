# Quick Start Guide - Commy Server & Examples

## Status

✅ **Server Infrastructure**: Successfully configured and running
- Commy server: WSS on 127.0.0.1:8443 with TLS
- PostgreSQL: Running on commy-postgres-1 container (port 5434)
- Auth-framework: Initialized with PostgreSQL backend

❌ **Test Credentials**: Need to be registered in PostgreSQL

## What Works Right Now

1. **Commy server starts successfully** with auth-framework
2. **Database connection** is working (PostgreSQL commy2_test)
3. **TLS certificates** are in place and valid
4. **Examples can connect** to the server via WSS

## What Needs to Happen

The examples need API keys registered in the database to authenticate. Due to auth-framework's schema complexity, here's the simplest approach:

### Option 1: Use auth-framework's Direct API (Recommended)

The auth-framework likely has a REST endpoint to manage credentials. To add test API keys:

```bash
curl -X POST http://127.0.0.1:8443/auth/api-keys \
  -H "Content-Type: application/json" \
  -d '{
    "key": "test_key_123",
    "scopes": ["read", "write"]
  }'
```

(Adjust port/path based on auth-framework's actual REST API)

### Option 2: Manual Database Entry

Once we identify the correct table schema, manually insert:

```sql
-- Connect to the database
docker exec -i commy-postgres-1 psql -U commy2_test -d commy2_test

-- Insert test API key (schema may vary)
INSERT INTO auth_credentials (...) VALUES (...);
```

### Option 3: Check Auth-Framework Documentation

Visit the auth-framework library documentation to understand:
- How to programmatically register API keys
- The expected database schema
- Best practices for test credentials

## Current Database State

The Commy server successfully created auth-framework tables in PostgreSQL:

```
Database: commy2_test
User: commy2_test
Host: commy-postgres-1:5432
```

To inspect the schema:
```bash
docker exec -e PGPASSWORD=test_password commy-postgres-1 \
  psql -U commy2_test -d commy2_test -c "\dt"
```

## Next Steps

1. **Identify the auth-framework credential table**
   - Run the inspect command above
   - Look for tables like: `credentials`, `api_keys`, `auth_tokens`, etc.

2. **Insert test keys with correct schema**
   - Once table name is known, insert the 4 test keys
   - Ensure scopes are properly mapped

3. **Test authentication**
   ```bash
   cd commy-sdk-rust
   cargo run --release --example basic_client -- \
     --server-url wss://127.0.0.1:8443 \
     --api-key test_key_123
   ```

## Test API Keys Expected

| Key                      | Scopes        | Usage                         |
| ------------------------ | ------------- | ----------------------------- |
| test_key_123             | read, write   | basic_client, hybrid_client   |
| admin_key_with_all_perms | admin         | permissions_example (admin)   |
| read_only_key            | read          | permissions_example (read)    |
| creator_key              | admin, create | permissions_example (creator) |

## Troubleshooting

**Error: "Role 'postgres' does not exist"**
- Use `commy2_test` user instead of `postgres`
- Container has: POSTGRES_USER=commy2_test, PASSWORD=test_password

**Error: "Table not found"**
- Auth-framework creates tables lazily  
- The 'INSERT INTO api_credentials' command will show the actual table name in error message

**Server not starting**
- Ensure PostgreSQL is running: `docker ps | grep postgres`
- Check DATABASE_URL environment variable matches container credentials

## Architecture

```
Examples (basic_client, hybrid_client, permissions_example)
    ↓ (WSS with TLS)
Commy Server (127.0.0.1:8443)
    ↓ (SQL queries)
Auth-Framework (in-process)
    ↓ (verify credentials)
PostgreSQL (commy-postgres-1:5434)
```

## Files Created

- `dev-cert.pem` - TLS certificate
- `dev-key.pem` - TLS private key
- `init_test_credentials.ps1` - PowerShell helper script
- `init_test_credentials.sh` - Bash helper script
- `init_test_credentials.sql` - SQL-based initialization
- `AUTHENTICATION_SETUP.md` - Detailed auth configuration
- `QUICK_START.md` - This file

## Key Technical Details

- **Port 8443**: Commy WSS server
- **Port 5434**: PostgreSQL (mapped from 5432 inside container)
- **Database**: commy2_test
- **Auth Backend**: PostgreSQL (not memory-based)
- **TLS**: Required for WSS connections
- **Auth Framework Version**: v0.4.2

---

**Status**: Infrastructure complete, ready for credential seeding

Once API keys are registered, all examples should run successfully! ✨
