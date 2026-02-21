# Docker Deployment Guide for Commy

This guide provides comprehensive instructions for building, deploying, and managing Commy using Docker and Docker Compose.

## Prerequisites

- **Docker**: Version 20.10 or higher
- **Docker Compose**: Version 1.29 or higher (or Docker with compose v2)
- **Git**: For cloning the repository
- **At least 2GB RAM** for running all services

## Quick Start

### 1. Clone and Navigate to Project
```bash
git clone <repository-url>
cd commy
```

### 2. Build and Start All Services
```bash
# Build images
docker-compose build

# Start services in background
docker-compose up -d

# Check service status
docker-compose ps
```

### 3. Verify Services Are Healthy
```bash
# All services should show status "Up (healthy)"
docker-compose ps

# Test individual services:
docker exec commy-postgres-1 psql -U commy_test -d commy_test -c "SELECT 1"
docker exec commy-redis-1 redis-cli ping
docker exec commy-mysql-1 mysql -u commy_test -ptest_password -e "SELECT 1"
```

### 4. Stop Services
```bash
docker-compose down
```

## Services Overview

### PostgreSQL (Port 5434)
- **Image**: postgres:15
- **Container Name**: commy-postgres-1
- **Database**: commy_test
- **User**: commy_test
- **Password**: test_password
- **Health Check**: Every 5 seconds
- **Use Case**: Production-ready relational database for Commy storage backend

**Connection String**:
```
postgresql://commy_test:test_password@localhost:5434/commy_test
```

**Test Connection**:
```bash
psql postgresql://commy_test:test_password@localhost:5434/commy_test
```

### MySQL (Port 3306)
- **Image**: mysql:8
- **Container Name**: commy-mysql-1
- **Database**: commy_test
- **User**: commy_test
- **Password**: test_password
- **Root Password**: root_password
- **Health Check**: Every 5 seconds via mysqladmin ping
- **Use Case**: Alternative relational database backend for Commy

**Connection String**:
```
mysql://commy_test:test_password@localhost:3306/commy_test
```

**Test Connection**:
```bash
mysql -h localhost -u commy_test -ptest_password commy_test
```

### Redis (Port 6379)
- **Image**: redis:7
- **Container Name**: commy-redis-1
- **Health Check**: Every 5 seconds via redis-cli ping
- **Use Case**: High-performance cache backend for Commy

**Test Connection**:
```bash
redis-cli -h localhost -p 6379 ping
```

## Docker Compose File Structure

```yaml
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: commy_test
      POSTGRES_USER: commy_test
      POSTGRES_PASSWORD: test_password
    ports:
      - "5434:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U commy_test"]
      interval: 5s
      timeout: 5s
      retries: 5

  mysql:
    image: mysql:8
    environment:
      MYSQL_DATABASE: commy_test
      MYSQL_USER: commy_test
      MYSQL_PASSWORD: test_password
      MYSQL_ROOT_PASSWORD: root_password
    ports:
      - "3306:3306"
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 5s
      retries: 5
```

## Dockerfile Structure

The Dockerfile uses a **two-stage build** pattern:

### Stage 1: Builder
- Compiles Rust code in `rust:latest` image
- Installs build dependencies (pkg-config, libssl-dev)
- Produces optimized binary via `cargo build --release`
- Result: Single binary at `/build/target/release/commy`

### Stage 2: Runtime
- Uses minimal `debian:bookworm-slim` base image
- Copies binary from builder stage
- Creates required directories `/var/lib/commy` and `/etc/commy`
- Exposes ports 8000-8003 (client) and 9000-9003 (inter-server)
- Includes health check via HTTP endpoint
- **Image size**: ~200MB (optimized from ~2GB builder)

## Common Docker Commands

### Build the Image
```bash
# Build all images
docker-compose build

# Build specific service
docker-compose build commy

# Build without cache
docker-compose build --no-cache

# Build with progress output
docker-compose build --progress=plain
```

### Run Services
```bash
# Start in background
docker-compose up -d

# Start in foreground (see logs)
docker-compose up

# Start specific service
docker-compose up -d postgres

# Start and rebuild images
docker-compose up -d --build
```

### View Logs
```bash
# All services
docker-compose logs

# Specific service
docker-compose logs postgres

# Follow logs (tail)
docker-compose logs -f postgres

# Last 50 lines
docker-compose logs --tail=50 postgres
```

### Access Containers
```bash
# Open bash shell in container
docker-compose exec postgres bash

# Execute command in container
docker-compose exec postgres psql -U commy_test -d commy_test -c "SELECT 1"

# Interactive MySQL
docker-compose exec mysql bash -c "mysql -u commy_test -ptest_password commy_test"
```

### Stop and Clean Up
```bash
# Stop services (keep volumes)
docker-compose stop

# Stop and remove containers
docker-compose down

# Remove everything including volumes
docker-compose down -v

# Remove images too
docker-compose down --rmi all
```

### Inspect Services
```bash
# Show service status
docker-compose ps

# Show running services
docker-compose ps --services

# Show service statistics
docker stats commy-postgres-1

# Inspect service configuration
docker inspect commy-postgres-1
```

## Environment Variables Configuration

The Commy server supports these environment variables (from src/main.rs):

```bash
# Server identification
COMMY_SERVER_ID=node-1

# Client connection address (WebSocket listener)
COMMY_LISTEN_ADDR=0.0.0.0:8000

# Inter-server communication address
COMMY_BIND_ADDR=0.0.0.0:9000

# Enable clustering
COMMY_CLUSTER_ENABLED=true
```

## Storage Backend Configuration

Commy supports multiple storage backends. Configure via environment:

### PostgreSQL Backend
```rust
StorageBackend::PostgreSQL {
    url: "postgresql://commy_test:test_password@localhost:5434/commy_test",
    max_connections: 100,
}
```

### MySQL Backend
```rust
StorageBackend::MySQL {
    url: "mysql://commy_test:test_password@localhost:3306/commy_test",
    max_connections: 100,
}
```

### Redis Backend
```rust
StorageBackend::Redis {
    url: "redis://localhost:6379",
}
```

### In-Memory Backend (Development Only)
```bash
ENVIRONMENT=development  # Enables memory storage
```

## Troubleshooting

### Services Not Starting
```bash
# Check Docker daemon
docker ps

# Check Docker Compose syntax
docker-compose config

# View error logs
docker-compose logs --tail=100
```

### Connection Refused Errors
```bash
# Verify services are running
docker-compose ps

# Check if ports are already in use
netstat -ano | findstr :5434  # PostgreSQL
netstat -ano | findstr :3306  # MySQL
netstat -ano | findstr :6379  # Redis
```

### Database Connection Issues
```bash
# Test PostgreSQL
docker-compose exec postgres psql -U commy_test -d commy_test -c "\dt"

# Test MySQL
docker-compose exec mysql mysql -u commy_test -ptest_password -e "SHOW DATABASES;"

# Test Redis
docker-compose exec redis redis-cli info
```

### High Memory Usage
```bash
# Check container resource usage
docker stats

# Limit memory for a service (in docker-compose.yml)
services:
  postgres:
    deploy:
      resources:
        limits:
          memory: 512M
```

### Persistent Data

By default, database data is stored in Docker volumes and persists across container restarts:

```bash
# List volumes
docker volume ls | grep commy

# Inspect volume
docker volume inspect commy_postgres_data

# Remove volumes (WARNING: deletes data)
docker-compose down -v
```

## Performance Optimization

### For Development
- Use in-memory storage when possible
- Mount source code as volume for hot reloading
- Enable detailed logging

### For Production
- Use PostgreSQL or MySQL (Redis alone insufficient for persistence)
- Configure appropriate max_connections for your workload
- Use health checks to detect failures
- Set resource limits to prevent container OOM
- Use dedicated volumes for data persistence
- Consider using managed database services instead of Docker

## Security Best Practices

### Never use default passwords in production
```yaml
# BAD ❌
environment:
  POSTGRES_PASSWORD: test_password

# GOOD ✅
environment:
  POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
```

### Use environment files
```bash
# Create .env file (add to .gitignore)
POSTGRES_PASSWORD=<secure-password>
MYSQL_PASSWORD=<secure-password>

# Reference in docker-compose.yml
env_file:
  - .env
```

### Restrict port access
```yaml
# BAD ❌
ports:
  - "5434:5432"  # Exposed to all interfaces

# GOOD ✅
ports:
  - "127.0.0.1:5434:5432"  # Localhost only
```

### Network isolation
```yaml
# Create isolated network
networks:
  commy-net:
    driver: bridge

services:
  postgres:
    networks:
      - commy-net
```

## Integration with Commy Server

To run the Commy server with these backing services:

```bash
# Add to docker-compose.yml
services:
  commy:
    build: .
    depends_on:
      postgres:
        condition: service_healthy
      mysql:
        condition: service_healthy
      redis:
        condition: service_healthy
    environment:
      COMMY_SERVER_ID: node-1
      COMMY_LISTEN_ADDR: 0.0.0.0:8000
      COMMY_BIND_ADDR: 0.0.0.0:9000
      COMMY_CLUSTER_ENABLED: true
    ports:
      - "8000:8000"
      - "9000:9000"
    networks:
      - commy-net
```

## Monitoring and Logs

### Enable detailed logging
```bash
# Stream logs from all services
docker-compose logs -f

# Stream logs from specific service
docker-compose logs -f postgres

# Last 100 lines
docker-compose logs --tail=100
```

### Health check status
```bash
# Watch health status
docker ps --format "table {{.Names}}\t{{.Status}}"
```

### Resource monitoring
```bash
# Real-time stats
docker stats --no-stream

# Continuous monitoring
docker stats
```

## Advanced Configuration

### Custom docker-compose override
```bash
# Create docker-compose.override.yml for local customizations
# (automatically loaded by docker-compose)

services:
  postgres:
    environment:
      POSTGRES_PASSWORD: my-custom-password
    volumes:
      - ./backups:/backups
```

### Multi-stage deployment
```bash
# Staging environment
docker-compose -f docker-compose.yml -f docker-compose.staging.yml up -d

# Production environment
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

## References

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [PostgreSQL Docker Image](https://hub.docker.com/_/postgres)
- [MySQL Docker Image](https://hub.docker.com/_/mysql)
- [Redis Docker Image](https://hub.docker.com/_/redis)
- [Rust Docker Image](https://hub.docker.com/_/rust)

