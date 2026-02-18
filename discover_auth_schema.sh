#!/usr/bin/env bash
# discover_auth_schema.sh - Discover auth-framework's PostgreSQL schema

set -e

CONTAINER="commy-postgres-1"
USER="commy2_test"
PASSWORD="test_password"
DB="commy2_test"

echo "🔍 Discovering Auth-Framework PostgreSQL Schema"
echo "================================================"
echo ""

# Check if container exists and is running
if ! docker ps --format "{{.Names}}" | grep -q "^${CONTAINER}$"; then
    echo "❌ Container '$CONTAINER' not found or not running"
    docker ps
    exit 1
fi

echo "📋 All Tables in '$DB' database:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
docker exec -e PGPASSWORD="$PASSWORD" "$CONTAINER" \
    psql -U "$USER" -d "$DB" -c "\dt" || echo "No tables found"

echo ""
echo "📊 Table Schemas:"
echo "━━━━━━━━━━━━━━━━"

# Get all table names
TABLES=$(docker exec -e PGPASSWORD="$PASSWORD" "$CONTAINER" \
    psql -U "$USER" -d "$DB" -t -c \
    "SELECT table_name FROM information_schema.tables WHERE table_schema='public' ORDER BY table_name;")

if [ -z "$TABLES" ]; then
    echo "❌ No tables found in public schema"
    echo ""
    echo "This may mean:"
    echo "  1. Auth-framework hasn't been initialized yet"
    echo "  2. Server is not currently running"
    echo "  3. Tables were created in a different schema"
    echo ""
    echo "🔧 Try starting the server with:"
    echo "   \$env:DATABASE_URL='postgresql://commy2_test:test_password@127.0.0.1:5434/commy2_test'"
    echo "   .\\target\\release\\commy.exe"
    exit 0
fi

# For each table, show its columns
for table in $TABLES; do
    echo ""
    echo "Table: $table"
    echo "─────────────────────────────────────────"
    docker exec -e PGPASSWORD="$PASSWORD" "$CONTAINER" \
        psql -U "$USER" -d "$DB" -c "\d+ $table" || echo "Could not describe $table"
done

echo ""
echo "✅ Schema discovery complete!"
echo ""
echo "📝 Use this information to:"
echo "  1. Update init_test_credentials.sql with correct table names"
echo "  2. Map test API keys to the appropriate columns"
echo "  3. Seed the database with test credentials"
