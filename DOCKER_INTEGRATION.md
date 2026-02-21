# Commy Server Docker Integration

This guide shows how to integrate the Commy server with the Docker Compose backing services.

## Current Setup

The project includes:
- ✅ **Dockerfile** - Multi-stage build for optimized Commy binary
- ✅ **docker-compose.yml** - PostgreSQL, MySQL, and Redis services
- ✅ **src/main.rs** - Binary entry point with clustering support
- ✅ **Health checks** - Automatic service validation
- ✅ **Port mapping** - Exposed ports for client and inter-server communication

## Dockerfile Overview

### Build Stage
```dockerfile
FROM rust:latest AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release
```

Produces optimized binary at `/build/target/release/commy`.

### Runtime Stage
```dockerfile
FROM debian:bookworm-slim
COPY --from=builder /build/target/release/commy /usr/local/bin/commy
EXPOSE 8000 8001 8002 8003 9000 9001 9002 9003
HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1
```

Creates minimal runtime image with health checking.

## Building the Commy Image

```bash
# Build Commy server image
docker-compose build

# Build specific image
docker build -t commy:latest .

# Build without cache
docker-compose build --no-cache

# View build layers
docker image history commy:latest
```

## Running Commy Server with Docker Compose

### Option 1: Manual Service Dependencies

Add to docker-compose.yml:

```yaml
services:
  # ... existing postgres, mysql, redis services ...

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
      - "8000:8000"  # Client WebSocket
      - "8001:8001"  # Additional client port
      - "8002:8002"  # Additional client port
      - "8003:8003"  # Additional client port
      - "9000:9000"  # Inter-server communication
      - "9001:9001"  # Inter-server communication
      - "9002:9002"  # Inter-server communication
      - "9003:9003"  # Inter-server communication
    networks:
      - commy-net
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 10s
      timeout: 5s
      retries: 3
```

### Option 2: Development Quick Start

For local development without Docker:

```bash
# Terminal 1: Start backing services
docker-compose up postgres mysql redis

# Terminal 2: Run server locally
cargo run --bin commy
```

## Environment Variables

The Commy server respects these environment variables:

| Variable                | Default      | Purpose                                 |
| ----------------------- | ------------ | --------------------------------------- |
| `COMMY_SERVER_ID`       | node-1       | Unique identifier for this server       |
| `COMMY_LISTEN_ADDR`     | 0.0.0.0:8000 | WebSocket listener for clients          |
| `COMMY_BIND_ADDR`       | 0.0.0.0:9000 | Inter-server communication bind address |
| `COMMY_CLUSTER_ENABLED` | true         | Enable clustering mode                  |

## Storage Backend Configuration

Commy supports configurable storage backends. Set via code:

### PostgreSQL Backend
```rust
let config = TenantAuthConfig {
    storage_backend: StorageBackend::PostgreSQL {
        url: "postgresql://commy_test:test_password@commy-postgres-1:5432/commy_test".to_string(),
        max_connections: 100,
    },
    ..Default::default()
};
```

### MySQL Backend
```rust
let config = TenantAuthConfig {
    storage_backend: StorageBackend::MySQL {
        url: "mysql://commy_test:test_password@commy-mysql-1:3306/commy_test".to_string(),
        max_connections: 100,
    },
    ..Default::default()
};
```

### Redis Backend
```rust
let config = TenantAuthConfig {
    storage_backend: StorageBackend::Redis {
        url: "redis://commy-redis-1:6379".to_string(),
    },
    ..Default::default()
};
```

**Note**: Use Docker service names (e.g., `commy-postgres-1`) when running in Docker Compose.

## Multi-Node Clustering

### Run Multiple Commy Nodes

```yaml
services:
  commy-node1:
    build: .
    depends_on: [postgres, mysql, redis]
    environment:
      COMMY_SERVER_ID: node-1
      COMMY_LISTEN_ADDR: 0.0.0.0:8000
      COMMY_BIND_ADDR: 0.0.0.0:9000
      COMMY_CLUSTER_ENABLED: true
    ports:
      - "8000:8000"
      - "9000:9000"
    networks: [commy-net]

  commy-node2:
    build: .
    depends_on: [postgres, mysql, redis]
    environment:
      COMMY_SERVER_ID: node-2
      COMMY_LISTEN_ADDR: 0.0.0.0:8000
      COMMY_BIND_ADDR: 0.0.0.0:9000
      COMMY_CLUSTER_ENABLED: true
    ports:
      - "8001:8000"
      - "9001:9000"
    networks: [commy-net]

  commy-node3:
    build: .
    depends_on: [postgres, mysql, redis]
    environment:
      COMMY_SERVER_ID: node-3
      COMMY_LISTEN_ADDR: 0.0.0.0:8000
      COMMY_BIND_ADDR: 0.0.0.0:9000
      COMMY_CLUSTER_ENABLED: true
    ports:
      - "8002:8000"
      - "9002:9000"
    networks: [commy-net]
```

## Client Connection

Once running, connect to the Commy server:

### Rust Client
```rust
use commy::WssServerConfig;

#[tokio::main]
async fn main() {
    let config = WssServerConfig {
        listen_addr: "127.0.0.1:8000",
        ..Default::default()
    };
    
    // Connect to running server
    let server = commy::WssServer::connect(&config).await.unwrap();
    // Use server...
}
```

### WebSocket (curl)
```bash
# Connect to server
websocat ws://localhost:8000

# Will receive server messages
```

## Testing the Setup

### Test Docker Build
```bash
docker build -t commy:test .
docker run -it commy:test --help
```

### Test Running Server
```bash
# Start all services with Commy
docker-compose up -d

# Check Commy server is running
docker-compose ps commy

# View Commy logs
docker-compose logs -f commy

# Test health endpoint
curl -i http://localhost:8000/health
```

### Test Multi-Node Cluster
```bash
# Start cluster
docker-compose up -d

# Check all nodes running
docker-compose ps | grep commy

# View inter-node communication
docker-compose logs commy-node1 | grep "peer"
docker-compose logs commy-node2 | grep "peer"
```

## Troubleshooting

### Commy Container Won't Start

```bash
# Check build errors
docker-compose build --no-cache 2>&1 | tail -50

# Check container logs
docker-compose logs commy

# Check if port is in use
netstat -ano | findstr :8000
```

### Service Discovery Issues

When Commy connects to databases, use full service names:

```
❌ Wrong:  postgresql://commy_test:test_password@localhost:5434/commy_test
✅ Correct: postgresql://commy_test:test_password@commy-postgres-1:5432/commy_test
```

Note: Docker Compose DNS resolution uses service names, not localhost.

### Memory Issues

If container runs out of memory:

```yaml
services:
  commy:
    deploy:
      resources:
        limits:
          memory: 1G
        reservations:
          memory: 512M
```

### Connection Refused

Ensure `depends_on` uses `condition: service_healthy`:

```yaml
commy:
  depends_on:
    postgres:
      condition: service_healthy  # Wait for health check
    mysql:
      condition: service_healthy
    redis:
      condition: service_healthy
```

## Production Deployment

### Use Published Images
Instead of building from source:

```yaml
services:
  commy:
    image: commy:1.0.0  # Pre-built image
    depends_on: [postgres, mysql, redis]
```

### Configure TLS/HTTPS
```yaml
services:
  commy:
    environment:
      COMMY_TLS_CERT: /etc/commy/cert.pem
      COMMY_TLS_KEY: /etc/commy/key.pem
    volumes:
      - ./certs:/etc/commy:ro
```

### Resource Limits
```yaml
services:
  commy:
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 1G
        reservations:
          cpus: '0.5'
          memory: 512M
```

### Persistent Volumes
```yaml
services:
  commy:
    volumes:
      - commy-data:/var/lib/commy

volumes:
  commy-data:
    driver: local
```

## Performance Optimization

### Build Caching
```bash
# Separate Cargo.toml for better layer caching
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release --locked
COPY src ./src
```

### Multi-Stage Optimizations
- Builder uses `rust:latest` (includes tools)
- Runtime uses `debian:bookworm-slim` (small footprint)
- Final image: ~200MB vs ~2GB builder

### Network Optimization
- Use `--net=host` for local testing (avoid Docker bridge overhead)
- Use internal networks for multi-node clusters
- Enable TCP keepalive for long-lived connections

