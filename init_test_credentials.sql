-- Initialize test API credentials for Commy examples
-- This script populates the PostgreSQL database with test API keys
-- Run this after the Commy server creates the auth-framework schema

-- Insert test API keys for examples
-- Note: These are for development/testing only
-- The scopes field stores the permissions for each API key

-- Basic API key for basic_client and hybrid_client examples
INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'test_key_123', ARRAY['read', 'write'], true)
ON CONFLICT DO NOTHING;

-- Admin API key for permissions_example
INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'admin_key_with_all_perms', ARRAY['admin', 'manage_tenants', 'manage_users'], true)
ON CONFLICT DO NOTHING;

-- Read-only API key for permissions_example
INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'read_only_key', ARRAY['read'], true)
ON CONFLICT DO NOTHING;

-- Creator API key for permissions_example
INSERT INTO api_credentials (credential_type, credential_value, scopes, active)
VALUES 
  ('api_key', 'creator_key', ARRAY['admin', 'create_services', 'manage_variables'], true)
ON CONFLICT DO NOTHING;

-- Display the inserted credentials
-- SELECT * FROM api_credentials WHERE credential_type = 'api_key';
