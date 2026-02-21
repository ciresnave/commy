# Docker Quick Reference for Commy

## Essential Commands

### Start Services
```bash
docker-compose up -d                 # Start in background
docker-compose up                    # Start in foreground (see logs)
docker-compose up -d --build         # Rebuild and start
```

### Stop Services
```bash
docker-compose down                  # Stop and remove containers
docker-compose stop                  # Stop containers (keep volumes)
docker-compose down -v               # Remove everything including volumes
```

### View Status
```bash
docker-compose ps                    # Show all services
docker logs <container-name>         # View container logs
docker-compose logs -f <service>     # Follow logs in real-time
```

### Access Containers
```bash
# PostgreSQL
docker-compose exec postgres psql -U commy_test -d commy_test

# MySQL
docker-compose exec mysql mysql -u commy_test -ptest_password commy_test

# Redis
docker-compose exec redis redis-cli
```

## Service Connection Details

| Service    | Port | User        | Password      | Database    |
| ---------- | ---- | ----------- | ------------- | ----------- |
| PostgreSQL | 5434 | commy_test | test_password | commy_test |
| MySQL      | 3306 | commy_test | test_password | commy_test |
| Redis      | 6379 | -           | -             | -           |

## Connection Strings

```
PostgreSQL: postgresql://commy_test:test_password@localhost:5434/commy_test
MySQL:      mysql://commy_test:test_password@localhost:3306/commy_test
Redis:      redis://localhost:6379
```

## Quick Tests

```bash
# Test all services
docker-compose ps

# Test PostgreSQL
docker exec commy-postgres-1 psql -U commy_test -d commy_test -c "SELECT 1"

# Test MySQL  
docker exec commy-mysql-1 mysql -u commy_test -ptest_password -e "SELECT 1"

# Test Redis
docker exec commy-redis-1 redis-cli ping
```

## Troubleshooting

```bash
# Check syntax
docker-compose config

# View all logs
docker-compose logs

# View specific service logs
docker-compose logs postgres

# Rebuild everything
docker-compose down -v && docker-compose up -d --build
```

## Health Status

All services include health checks that run every 5 seconds. Status should be:
- ✓ `(healthy)` - Service is ready
- ⏳ `(health: starting)` - Service is initializing
- ✗ `(unhealthy)` - Service failed

