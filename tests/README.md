# Storage Backend Integration Tests

This directory contains integration tests for Commy's authentication storage backends.

## Test Organization

### `storage_backends.rs`
Tests authentication with different storage backends:
- **Memory Storage** (in-memory, development only)
- **PostgreSQL** (production database)
- **MySQL** (production database)  
- **Redis** (high-performance cache)

## Running Tests

### Quick Tests (Memory Storage Only)

Run tests that don't require external services:

```powershell
cargo test --test storage_backends
```

These tests use in-memory storage and run automatically in CI/CD.

### Full Integration Tests (Requires Databases)

To test against real databases, set up the services and run ignored tests:

```powershell
cargo test --test storage_backends -- --ignored --nocapture
```

## Database Setup

### PostgreSQL

1. **Install PostgreSQL** (if not already installed)
2. **Create test database**:
   ```sql
   CREATE DATABASE commy_test;
   CREATE USER commy_test WITH PASSWORD 'test_password';
   GRANT ALL PRIVILEGES ON DATABASE commy_test TO commy_test;
   ```

3. **Set environment variable**:
   ```powershell
   $env:DATABASE_URL = "postgresql://commy_test:test_password@localhost:5432/commy_test"
   ```

4. **Run PostgreSQL test**:
   ```powershell
   cargo test --test storage_backends test_postgresql_storage_backend -- --ignored --nocapture
   ```

### MySQL

1. **Install MySQL** (if not already installed)
2. **Create test database**:
   ```sql
   CREATE DATABASE commy_test;
   CREATE USER 'commy_test'@'localhost' IDENTIFIED BY 'test_password';
   GRANT ALL PRIVILEGES ON commy_test.* TO 'commy_test'@'localhost';
   FLUSH PRIVILEGES;
   ```

3. **Set environment variable**:
   ```powershell
   $env:MYSQL_URL = "mysql://commy_test:test_password@localhost:3306/commy_test"
   ```

4. **Run MySQL test**:
   ```powershell
   cargo test --test storage_backends test_mysql_storage_backend -- --ignored --nocapture
   ```

### Redis

1. **Install Redis** (if not already installed)
   - Windows: Use [Redis for Windows](https://github.com/microsoftarchive/redis/releases) or WSL
   - Or use Docker: `docker run -d -p 6379:6379 redis`

2. **Set environment variable** (if not localhost:6379):
   ```powershell
   $env:REDIS_URL = "redis://localhost:6379"
   ```

3. **Run Redis test**:
   ```powershell
   cargo test --test storage_backends test_redis_storage_backend -- --ignored --nocapture
   ```

## Docker Compose Setup (Recommended)

For easy testing, use Docker Compose to start all databases:

```yaml
# docker-compose.yml
version: '3.8'
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: commy_test
      POSTGRES_USER: commy_test
      POSTGRES_PASSWORD: test_password
    ports:
      - "5432:5432"

  mysql:
    image: mysql:8
    environment:
      MYSQL_DATABASE: commy_test
      MYSQL_USER: commy_test
      MYSQL_PASSWORD: test_password
      MYSQL_ROOT_PASSWORD: root_password
    ports:
      - "3306:3306"

  redis:
    image: redis:7
    ports:
      - "6379:6379"
```

Start all services:
```powershell
docker-compose up -d
```

Set environment variables:
```powershell
$env:DATABASE_URL = "postgresql://commy_test:test_password@localhost:5432/commy_test"
$env:MYSQL_URL = "mysql://commy_test:test_password@localhost:3306/commy_test"
$env:REDIS_URL = "redis://localhost:6379"
```

Run all integration tests:
```powershell
cargo test --test storage_backends -- --ignored --nocapture
```

Stop services when done:
```powershell
docker-compose down
```

## CI/CD Configuration

For CI/CD pipelines:

1. **Memory tests** run automatically (no external dependencies)
2. **Database tests** are ignored by default
3. Enable in CI by:
   - Starting database services (Docker containers)
   - Setting environment variables
   - Running: `cargo test --test storage_backends -- --ignored`

## Test Coverage

- ✅ Storage backend configuration
- ✅ Token creation with different backends
- ✅ Token validation across backends
- ✅ Multi-tenant with different backends
- ✅ Token lifetime configuration
- ⏳ Token expiration enforcement (depends on auth-framework behavior)

## Troubleshooting

### "Database connection failed"
- Verify database service is running
- Check connection URL environment variable
- Verify credentials and permissions

### "Connection refused"
- Check port mappings (5432 for PostgreSQL, 3306 for MySQL, 6379 for Redis)
- Verify firewall rules
- Ensure services are listening on correct interface

### "Schema/table not found"
- auth-framework creates tables automatically on first connection
- Ensure database user has CREATE TABLE permissions
- Check auth-framework initialization logs
