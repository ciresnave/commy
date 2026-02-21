#!/usr/bin/env python3
"""
Seed test API credentials in PostgreSQL for Commy examples.

This script discovers the auth-framework credential table schema and inserts
test API keys that examples can use for authentication.
"""

import psycopg2
import sys
from typing import Dict, List, Tuple

# Database connection parameters
DB_CONFIG = {
    'host': '127.0.0.1',
    'port': 5434,  # commy-postgres-1 container mapped port
    'user': 'commy_test',
    'password': 'test_password',
    'database': 'commy_test',
}

# Test API keys to insert
TEST_KEYS = {
    'test_key_123': {
        'scopes': ['read', 'write'],
        'active': True,
    },
    'admin_key_with_all_perms': {
        'scopes': ['admin', 'manage_tenants', 'manage_users'],
        'active': True,
    },
    'read_only_key': {
        'scopes': ['read'],
        'active': True,
    },
    'creator_key': {
        'scopes': ['admin', 'create_services', 'manage_variables'],
        'active': True,
    },
}

def connect_db():
    """Connect to the PostgreSQL database."""
    try:
        conn = psycopg2.connect(**DB_CONFIG)
        return conn
    except psycopg2.Error as e:
        print(f"❌ Failed to connect to database: {e}")
        sys.exit(1)

def discover_credential_table(conn) -> Dict[str, any]:
    """Discover the auth-framework credential table schema."""
    cursor = conn.cursor()
    
    try:
        # Query information_schema to find credential tables
        cursor.execute("""
            SELECT table_name, column_name, data_type
            FROM information_schema.columns
            WHERE table_schema = 'public'
            AND (table_name LIKE '%credential%' OR table_name LIKE '%auth%' OR table_name LIKE '%api%')
            ORDER BY table_name, ordinal_position
        """)
        
        results = cursor.fetchall()
        
        if not results:
            print("❌ No auth-framework tables found in the database")
            print("\nTroubleshooting:")
            print("  1. Make sure the Commy server has been started once to initialize auth-framework")
            print("  2. Check that DATABASE_URL is set correctly")
            print("  3. Verify PostgreSQL is running and accessible")
            return None
        
        # Group by table name
        tables = {}
        for table_name, column_name, data_type in results:
            if table_name not in tables:
                tables[table_name] = []
            tables[table_name].append({
                'name': column_name,
                'type': data_type,
            })
        
        return tables
    
    finally:
        cursor.close()

def insert_test_credentials(conn, table_info: Dict[str, List]) -> bool:
    """Insert test credentials into the database."""
    cursor = conn.cursor()
    
    try:
        # Find the credential table (likely named 'credentials', 'api_credentials', etc.)
        credential_table = None
        for table_name in table_info.keys():
            if 'credential' in table_name.lower() or 'api' in table_name.lower():
                credential_table = table_name
                break
        
        if not credential_table:
            print("❌ Could not identify credential table")
            print(f"\n Available tables: {list(table_info.keys())}")
            return False
        
        print(f"📝 Using table: {credential_table}")
        
        # Get column names for this table
        columns = {col['name']: col['type'] for col in table_info[credential_table]}
        print(f"   Columns: {list(columns.keys())}")
        
        # Determine which columns to use for API keys
        key_col = None
        scopes_col = None
        active_col = None
        
        for col_name in columns.keys():
            if 'key' in col_name.lower() or 'credential' in col_name.lower():
                key_col = col_name
            elif 'scope' in col_name.lower() or 'permission' in col_name.lower():
                scopes_col = col_name
            elif 'active' in col_name.lower():
                active_col = col_name
        
        if not key_col:
            print("❌ Could not identify key/credential column")
            return False
        
        print(f"\n   Key column: {key_col}")
        if scopes_col:
            print(f"   Scopes column: {scopes_col}")
        if active_col:
            print(f"   Active column: {active_col}")
        
        # Insert test keys
        inserted = 0
        for key_name, config in TEST_KEYS.items():
            try:
                # Build INSERT statement
                col_names = [key_col]
                values = [key_name]
                
                if scopes_col:
                    col_names.append(scopes_col)
                    # Convert scopes list to JSON or comma-separated string
                    scopes_str = ','.join(config['scopes'])
                    values.append(scopes_str)
                
                if active_col:
                    col_names.append(active_col)
                    values.append(config['active'])
                
                cols_str = ', '.join(col_names)
                placeholders = ', '.join(['%s'] * len(values))
                sql = f"INSERT INTO {credential_table} ({cols_str}) VALUES ({placeholders})"
                
                cursor.execute(sql, values)
                inserted += 1
                print(f"   ✅ Inserted: {key_name}")
            
            except psycopg2.Error as e:
                print(f"   ⚠️  Skipped {key_name}: {e}")
        
        conn.commit()
        print(f"\n✅ Successfully inserted {inserted}/{len(TEST_KEYS)} test keys")
        return True
    
    except Exception as e:
        conn.rollback()
        print(f"❌ Error inserting credentials: {e}")
        return False
    
    finally:
        cursor.close()

def main():
    print("🔑 Seeding Test API Credentials")
    print("=" * 50)
    print()
    
    # Connect to database
    print(f"📌 Connecting to {DB_CONFIG['host']}:{DB_CONFIG['port']}/{DB_CONFIG['database']}...")
    conn = connect_db()
    print("   ✅ Connected\n")
    
    # Discover schema
    print("🔍 Discovering auth-framework schema...")
    tables = discover_credential_table(conn)
    
    if not tables:
        conn.close()
        return 1
    
    print(f"   Found {len(tables)} auth-framework table(s):")
    for table_name, columns in tables.items():
        print(f"     • {table_name}")
        for col in columns:
            print(f"       - {col['name']} ({col['type']})")
    print()
    
    # Insert credentials
    print("📝 Inserting test credentials...")
    success = insert_test_credentials(conn, tables)
    
    conn.close()
    
    if success:
        print("\n" + "=" * 50)
        print("✅ Test credentials seeded successfully!")
        print("\n Examples can now authenticate with:")
        print("   • test_key_123 (read, write)")
        print("   • admin_key_with_all_perms (admin)")  
        print("   • read_only_key (read)")
        print("   • creator_key (admin, create)")
        return 0
    else:
        print("\n" + "=" * 50)
        print("❌ Failed to seed credentials")
        return 1

if __name__ == '__main__':
    sys.exit(main())
