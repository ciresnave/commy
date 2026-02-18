# Initialize PostgreSQL with test credentials for Commy examples
# Runs psql inside the PostgreSQL Docker container
# Usage: .\init_test_credentials.ps1

param(
    [string]$DockerContainer = "",
    [string]$PostgresUser = "commy2_test",
    [string]$PostgresPassword = "test_password",
    [string]$PostgresDb = "commy2_test"
)

Write-Host "Initializing test credentials in PostgreSQL..." -ForegroundColor Green
Write-Host ""

# Check if Commy server has initialized the database
# The auth-framework creates tables lazily on first use
Write-Host "Note: The Commy server must be started first to initialize the auth-framework schema." -ForegroundColor Yellow
Write-Host "The credentials table is created by auth-framework when the server starts." -ForegroundColor Yellow
Write-Host ""
Write-Host "Process:" -ForegroundColor Cyan
Write-Host "  1. Start the Commy server: .\target\release\commy.exe" -ForegroundColor Cyan
Write-Host "  2. Wait for '[Commy] ✓ Auth-framework initialized' message (20-30 seconds)" -ForegroundColor Cyan
Write-Host "  3. Then run this script to insert test credentials" -ForegroundColor Cyan
Write-Host ""

# Auto-detect container if not specified
if ([string]::IsNullOrEmpty($DockerContainer)) {
    # Look for common PostgreSQL container names used in Commy projects
    $possibleContainers = @("commy-postgres-1", "postgres-1", "commy2-postgres-1")
    
    foreach ($container in $possibleContainers) {
        $output = & docker inspect -f '{{.State.Running}}' $container 2>$null
        if ($LASTEXITCODE -eq 0 -and $output -eq "true") {
            $DockerContainer = $container
            Write-Host "Found running PostgreSQL container: $DockerContainer" -ForegroundColor Cyan
            break
        }
    }
    
    if ([string]::IsNullOrEmpty($DockerContainer)) {
        Write-Host "✗ No running PostgreSQL Docker container found" -ForegroundColor Red
        Write-Host "Available containers:" -ForegroundColor Yellow
        & docker ps -a | grep postgres
        Write-Host ""
        Write-Host "Start a container with: docker start commy-postgres-1" -ForegroundColor Yellow
        exit 1
    }
}

Write-Host "Docker Container: $DockerContainer"
Write-Host "Database: $PostgresDb"
Write-Host ""

# Verify container is running
try {
    $output = & docker inspect -f '{{.State.Running}}' $DockerContainer 2>$null
    if ($LASTEXITCODE -ne 0 -or $output -ne "true") {
        Write-Host "✗ PostgreSQL Docker container '$DockerContainer' is not running" -ForegroundColor Red
        Write-Host "Start it with: docker start $DockerContainer" -ForegroundColor Yellow
        exit 1
    }
} catch {
    Write-Host "✗ Docker not available or container not found" -ForegroundColor Red
    Write-Host "Make sure Docker is installed and the container name is correct" -ForegroundColor Yellow
    exit 1
}

Write-Host "Running credentials initialization inside Docker container..." -ForegroundColor Cyan
Write-Host ""

# Run SQL commands inside container via docker exec
# Set PGPASSWORD to avoid password prompt
$sqlCommands = @"
INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'test_key_123', ARRAY['read', 'write'], true),
  ('api_key', 'admin_key_with_all_perms', ARRAY['admin', 'manage_tenants', 'manage_users'], true),
  ('api_key', 'read_only_key', ARRAY['read'], true),
  ('api_key', 'creator_key', ARRAY['admin', 'create_services', 'manage_variables'], true)
ON CONFLICT DO NOTHING;

SELECT credential_value, scopes FROM api_credentials WHERE credential_type = 'api_key' ORDER BY credential_value;
"@

# Run the SQL script inside the container
$env:PGPASSWORD = $PostgresPassword
& docker exec -i $DockerContainer psql -U $PostgresUser -d $PostgresDb -c $sqlCommands

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "✓ Test credentials initialized successfully" -ForegroundColor Green
    Write-Host ""
    Write-Host "API Keys registered:" -ForegroundColor Cyan
    Write-Host "  - test_key_123 (for basic_client, hybrid_client examples)"
    Write-Host "  - admin_key_with_all_perms (for permissions_example admin user)"
    Write-Host "  - read_only_key (for permissions_example read-only user)"
    Write-Host "  - creator_key (for permissions_example creator user)"
    Write-Host ""
    Write-Host "Examples should now authenticate successfully!" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "✗ Failed to initialize credentials" -ForegroundColor Red
    Write-Host "Try running manually:" -ForegroundColor Yellow
    Write-Host "docker exec -i postgres-1 psql -U postgres -d commy -f init_test_credentials.sql" -ForegroundColor Gray
    exit 1
}

# Clear the password from environment
Remove-Item env:PGPASSWORD -ErrorAction SilentlyContinue
