#!/bin/bash
# Initialize PostgreSQL with test credentials for Commy examples
# Runs psql inside the PostgreSQL Docker container
# Usage: ./init_test_credentials.sh

DOCKER_CONTAINER=${DOCKER_CONTAINER:-}
POSTGRES_USER=${POSTGRES_USER:-commy2_test}
POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-test_password}
POSTGRES_DB=${POSTGRES_DB:-commy2_test}

echo "Initializing test credentials in PostgreSQL..."
echo ""

# Auto-detect container if not specified
if [ -z "$DOCKER_CONTAINER" ]; then
    # Look for common PostgreSQL container names used in Commy projects
    for container in "commy-postgres-1" "postgres-1" "commy2-postgres-1"; do
        RUNNING=$(docker inspect -f '{{.State.Running}}' "$container" 2>/dev/null)
        if [ "$RUNNING" = "true" ]; then
            DOCKER_CONTAINER="$container"
            echo "Found running PostgreSQL container: $DOCKER_CONTAINER"
            break
        fi
    done
    
    if [ -z "$DOCKER_CONTAINER" ]; then
        echo "✗ No running PostgreSQL Docker container found"
        echo "Available containers:"
        docker ps -a | grep postgres
        echo ""
        echo "Start a container with: docker start commy-postgres-1"
        exit 1
    fi
fi

echo "Docker Container: $DOCKER_CONTAINER"
echo "Database: $POSTGRES_DB"
echo ""

# Verify container is running
RUNNING=$(docker inspect -f '{{.State.Running}}' "$DOCKER_CONTAINER" 2>/dev/null)
if [ "$RUNNING" != "true" ]; then
    echo "✗ PostgreSQL Docker container '$DOCKER_CONTAINER' is not running"
    echo "Start it with: docker start $DOCKER_CONTAINER"
    exit 1
fi

echo "Running credentials initialization inside Docker container..."
echo ""

# Run SQL commands inside container via docker exec
PGPASSWORD=$POSTGRES_PASSWORD docker exec -i "$DOCKER_CONTAINER" psql \
  -U $POSTGRES_USER \
  -d $POSTGRES_DB \
  -c "INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'test_key_123', ARRAY['read', 'write'], true),
  ('api_key', 'admin_key_with_all_perms', ARRAY['admin', 'manage_tenants', 'manage_users'], true),
  ('api_key', 'read_only_key', ARRAY['read'], true),
  ('api_key', 'creator_key', ARRAY['admin', 'create_services', 'manage_variables'], true)
ON CONFLICT DO NOTHING;

SELECT credential_value, scopes FROM api_credentials WHERE credential_type = 'api_key' ORDER BY credential_value;"

if [ $? -eq 0 ]; then
  echo ""
  echo "✓ Test credentials initialized successfully"
  echo ""
  echo "API Keys registered:"
  echo "  - test_key_123 (for basic_client, hybrid_client examples)"
  echo "  - admin_key_with_all_perms (for permissions_example admin user)"
  echo "  - read_only_key (for permissions_example read-only user)"
  echo "  - creator_key (for permissions_example creator user)"
  echo ""
  echo "Examples should now authenticate successfully!"
else
  echo ""
  echo "✗ Failed to initialize credentials"
  echo "Try running manually:"
  echo "docker exec -i postgres-1 psql -U postgres -d commy -f init_test_credentials.sql"
  exit 1
fi
