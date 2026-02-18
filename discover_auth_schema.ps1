#!/usr/bin/env pwsh
# discover_auth_schema.ps1 - Discover auth-framework's PostgreSQL schema

$Container = "commy-postgres-1"
$PostgresUser = "commy2_test"
$PostgresPassword = "test_password"
$PostgresDb = "commy2_test"

Write-Host "🔍 Discovering Auth-Framework PostgreSQL Schema" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# Check if container exists and is running
$container_check = docker ps --format "{{.Names}}" | Select-String "^${Container}$"
if (-not $container_check) {
    Write-Host "❌ Container '$Container' not found or not running" -ForegroundColor Red
    docker ps
    exit 1
}

Write-Host "📋 All Tables in '$PostgresDb' database:" -ForegroundColor Yellow
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Yellow

$env:PGPASSWORD = $PostgresPassword
$tables_output = docker exec $Container psql -U $PostgresUser -d $PostgresDb -c "\dt" 2>&1
Write-Host $tables_output

Write-Host ""
Write-Host "📊 Table Schemas:" -ForegroundColor Yellow
Write-Host "━━━━━━━━━━━━━━━━" -ForegroundColor Yellow

# Get all table names
$table_list = docker exec $Container psql -U $PostgresUser -d $PostgresDb -t -c `
    "SELECT table_name FROM information_schema.tables WHERE table_schema='public' ORDER BY table_name;" 2>&1

$tables = $table_list | Where-Object { $_ -match '\S' }

if ($tables.Count -eq 0 -or [string]::IsNullOrWhiteSpace($tables[0])) {
    Write-Host "❌ No tables found in public schema" -ForegroundColor Red
    Write-Host ""
    Write-Host "This may mean:" -ForegroundColor Yellow
    Write-Host "  1. Auth-framework hasn't been initialized yet"
    Write-Host "  2. Server is not currently running"
    Write-Host "  3. Tables were created in a different schema"
    Write-Host ""
    Write-Host "🔧 Try starting the server with:" -ForegroundColor Cyan
    Write-Host @"
`$env:DATABASE_URL = 'postgresql://commy2_test:test_password@127.0.0.1:5434/commy2_test'
.\target\release\commy.exe
"@ -ForegroundColor Cyan
    exit 0
}

# For each table, show its columns
foreach ($table in $tables) {
    $table = $table.Trim()
    if ([string]::IsNullOrWhiteSpace($table)) { continue }
    
    Write-Host ""
    Write-Host "Table: $table" -ForegroundColor Green
    Write-Host "─────────────────────────────────────────" -ForegroundColor Green
    $schema_output = docker exec $Container psql -U $PostgresUser -d $PostgresDb -c "\d+ $table" 2>&1
    Write-Host $schema_output
}

Write-Host ""
Write-Host "✅ Schema discovery complete!" -ForegroundColor Green
Write-Host ""
Write-Host "📝 Use this information to:" -ForegroundColor Yellow
Write-Host "  1. Update init_test_credentials.sql with correct table names"
Write-Host "  2. Map test API keys to the appropriate columns"
Write-Host "  3. Seed the database with test credentials"
