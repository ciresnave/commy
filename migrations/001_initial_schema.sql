-- Initial database schema for Commy Shared File Manager
-- This migration creates the core tables for file management, authentication, and auditing

-- Table to store shared file information
CREATE TABLE shared_files (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    file_path TEXT NOT NULL UNIQUE,
    directionality TEXT NOT NULL,
    topology TEXT NOT NULL,
    serialization_format TEXT NOT NULL,
    connection_side TEXT NOT NULL,
    existence_policy TEXT NOT NULL,
    creation_policy TEXT NOT NULL,
    transport_preference TEXT NOT NULL,
    max_size_bytes INTEGER,
    ttl_seconds INTEGER,
    max_connections INTEGER,
    encryption_required BOOLEAN NOT NULL DEFAULT 0,
    auto_cleanup BOOLEAN NOT NULL DEFAULT 1,
    persist_after_disconnect BOOLEAN NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    metadata_json TEXT -- JSON blob for additional metadata
);

-- Table to track client connections to shared files
CREATE TABLE client_connections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id TEXT NOT NULL,
    file_id INTEGER NOT NULL,
    process_id INTEGER,
    network_address TEXT,
    connection_type TEXT NOT NULL,
    auth_token_hash TEXT NOT NULL,
    identity TEXT NOT NULL,
    permissions_json TEXT, -- JSON array of permissions
    connected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_heartbeat DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    disconnected_at DATETIME,
    disconnect_reason TEXT,

    FOREIGN KEY (file_id) REFERENCES shared_files (id) ON DELETE CASCADE
);

-- Table for performance metrics and statistics
CREATE TABLE file_statistics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read_count INTEGER NOT NULL DEFAULT 0,
    write_count INTEGER NOT NULL DEFAULT 0,
    bytes_read INTEGER NOT NULL DEFAULT 0,
    bytes_written INTEGER NOT NULL DEFAULT 0,
    avg_latency_us REAL NOT NULL DEFAULT 0.0,
    concurrent_connections INTEGER NOT NULL DEFAULT 0,
    peak_connections INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY (file_id) REFERENCES shared_files (id) ON DELETE CASCADE
);

-- Table for client performance metrics
CREATE TABLE client_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id INTEGER NOT NULL,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    messages_sent INTEGER NOT NULL DEFAULT 0,
    messages_received INTEGER NOT NULL DEFAULT 0,
    bytes_sent INTEGER NOT NULL DEFAULT 0,
    bytes_received INTEGER NOT NULL DEFAULT 0,
    avg_latency_us REAL NOT NULL DEFAULT 0.0,
    uptime_seconds INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY (connection_id) REFERENCES client_connections (id) ON DELETE CASCADE
);

-- Table for audit logging
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    event_type TEXT NOT NULL,
    client_id TEXT,
    file_id INTEGER,
    identity TEXT,
    source_ip TEXT,
    event_data_json TEXT, -- JSON blob for event-specific data
    success BOOLEAN NOT NULL,
    error_message TEXT,

    FOREIGN KEY (file_id) REFERENCES shared_files (id) ON DELETE SET NULL
);

-- Table for authentication tokens and sessions
CREATE TABLE auth_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL UNIQUE,
    client_id TEXT NOT NULL,
    identity TEXT NOT NULL,
    auth_token_hash TEXT NOT NULL,
    permissions_json TEXT, -- JSON array of permissions
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    last_access DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    source_ip TEXT,
    user_agent TEXT,
    revoked BOOLEAN NOT NULL DEFAULT 0,
    revoked_at DATETIME,
    revoked_reason TEXT
);

-- Table for manager configuration
CREATE TABLE manager_config (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL, -- 'string', 'integer', 'boolean', 'json'
    description TEXT,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by TEXT
);

-- Table for reusable file IDs
CREATE TABLE reusable_file_ids (
    id INTEGER PRIMARY KEY NOT NULL,
    released_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_shared_files_name ON shared_files (name);
CREATE INDEX idx_shared_files_status ON shared_files (status);
CREATE INDEX idx_shared_files_created_by ON shared_files (created_by);
CREATE INDEX idx_shared_files_created_at ON shared_files (created_at);

CREATE INDEX idx_client_connections_client_id ON client_connections (client_id);
CREATE INDEX idx_client_connections_file_id ON client_connections (file_id);
CREATE INDEX idx_client_connections_connected_at ON client_connections (connected_at);
CREATE INDEX idx_client_connections_last_heartbeat ON client_connections (last_heartbeat);

CREATE INDEX idx_file_statistics_file_id ON file_statistics (file_id);
CREATE INDEX idx_file_statistics_timestamp ON file_statistics (timestamp);

CREATE INDEX idx_client_metrics_connection_id ON client_metrics (connection_id);
CREATE INDEX idx_client_metrics_timestamp ON client_metrics (timestamp);

CREATE INDEX idx_audit_log_timestamp ON audit_log (timestamp);
CREATE INDEX idx_audit_log_event_type ON audit_log (event_type);
CREATE INDEX idx_audit_log_client_id ON audit_log (client_id);
CREATE INDEX idx_audit_log_file_id ON audit_log (file_id);

CREATE INDEX idx_auth_sessions_session_id ON auth_sessions (session_id);
CREATE INDEX idx_auth_sessions_client_id ON auth_sessions (client_id);
CREATE INDEX idx_auth_sessions_expires_at ON auth_sessions (expires_at);
CREATE INDEX idx_auth_sessions_revoked ON auth_sessions (revoked);

-- Insert default configuration values
INSERT INTO manager_config (key, value, value_type, description) VALUES
    ('listen_port', '8080', 'integer', 'Port for the manager to listen on'),
    ('bind_address', '127.0.0.1', 'string', 'Address for the manager to bind to'),
    ('max_files', '1000', 'integer', 'Maximum number of concurrent shared files'),
    ('max_file_size', '1073741824', 'integer', 'Maximum file size in bytes (1GB)'),
    ('default_ttl_seconds', '3600', 'integer', 'Default TTL for files in seconds'),
    ('heartbeat_timeout_seconds', '30', 'integer', 'Client heartbeat timeout'),
    ('cleanup_interval_seconds', '60', 'integer', 'Cleanup task interval'),
    ('require_tls', 'false', 'boolean', 'Whether to require TLS for all connections'),
    ('require_auth', 'true', 'boolean', 'Whether to require authentication'),
    ('max_auth_failures', '5', 'integer', 'Maximum authentication failures before lockout'),
    ('auth_lockout_seconds', '300', 'integer', 'Authentication lockout duration'),
    ('audit_logging', 'true', 'boolean', 'Enable audit logging'),
    ('threat_detection', 'true', 'boolean', 'Enable threat detection'),
    ('performance_monitoring', 'true', 'boolean', 'Enable performance monitoring'),
    ('collection_interval_seconds', '10', 'integer', 'Performance metrics collection interval'),
    ('history_retention_days', '30', 'integer', 'Performance history retention period');