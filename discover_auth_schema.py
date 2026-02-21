#!/usr/bin/env python3
"""Query PostgreSQL database to discover table schema."""

import subprocess
import json
import sys

def run_psql_command(cmd):
    """Execute a psql command in the Docker container."""
    full_cmd = [
        "docker", "exec",
        "-e", "PGPASSWORD=test_password",
        "commy-postgres-1",
        "psql",
        "-U", "commy_test",
        "-d", "commy_test",
        "-c", cmd,
        "--quiet"
    ]
    
    try:
        result = subprocess.run(full_cmd, capture_output=True, text=True, timeout=10)
        return result.stdout.strip(), result.returncode
    except subprocess.TimeoutExpired:
        return "TIMEOUT", 1
    except Exception as e:
        return f"ERROR: {e}", 1

def main():
    print("🔍 Discovering PostgreSQL Schema created by auth-framework")
    print("=" * 60)
    print()
    
    # Get table list
    print("📋 Querying all tables...")
    output, code = run_psql_command(
        "SELECT table_name FROM information_schema.tables WHERE table_schema='public' ORDER BY table_name;"
    )
    
    if code != 0 or not output.strip():
        print("❌ No tables found or query failed")
        print(f"   Output: {output[:200] if output else '(empty)'}")
        print()
        print("This likely means auth-framework hasn't initialized yet.")
        print("Make sure the Commy server is running with:")
        print("  $env:DATABASE_URL='postgresql://commy_test:test_password@127.0.0.1:5434/commy_test'")
        print("  .\\target\\release\\commy.exe")
        return 1
    
    # Parse table names
    tables = [line.strip() for line in output.split('\n') if line.strip()]
    
    if not tables:
        print("❌ No tables found in public schema")
        return 1
    
    print(f"✅ Found {len(tables)} table(s):")
    print()
    
    for table in tables:
        print(f"  📊 {table}")
    
    print()
    print("=" * 60)
    print("📝 Table Details:")
    print("=" * 60)
    print()
    
    # Get schema for each table
    for table in tables:
        print(f"\nTable: {table}")
        print("-" * 60)
        
        # Get columns
        columns_cmd = f"\\d+ {table}"
        output, code = run_psql_command(columns_cmd)
        
        if code == 0 and output:
            print(output)
        else:
            print(f"  Could not describe table (error: {code})")
    
    print()
    print("=" * 60)
    print("✅ Schema discovery complete!")
    print()
    print("📝 Next steps:")
    print("  1. Note the table names above")
    print("  2. Update init_test_credentials.sql with correct table name")
    print("  3. Map API key fields to the appropriate columns")
    print("  4. Execute the SQL file to insert test credentials")
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
