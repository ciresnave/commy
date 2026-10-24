# Storage backend tests

`storage_backends.rs` tests authentication with the in-memory backend:

- `test_memory_storage_backend`
- `test_multiple_tenants_different_backends`
- `test_token_lifetime_configuration`

Run them with `cargo test --test storage_backends`; CI runs them. They need no external service.

## Real databases: not tested yet

`TenantAuthConfig.storage_backend` is accepted for PostgreSQL, MySQL and Redis, but
`TenantAuthContext::new` ignores it and only `Memory` takes effect (ROADMAP R21). The earlier tests for
those three backends were deleted in 0.22.3: they exercised no database and passed whether or not one
answered. They come back when the backends are wired through PersistAnt, as tests that run against real
service containers in CI and fail when the store is unreachable.

`docker-compose.yml` in the repository root already starts PostgreSQL, MySQL and Redis for that work;
no test uses it today.

## Other test files

- `wss_loopback.rs`: the real TLS and WebSocket server against real clients on a loopback port.
- `restart.rs`: services and grants survive a kill and restart of the binary.
- `windows_second_account.rs`: tenant-directory ACL as a second Windows account (R9, R14); prints
  `UNVERIFIED` and returns unless `COMMY_R9_SPAWN` and `COMMY_R9_PROBE` are set.
- `gen_certs.rs`: the `gen_certs` binary writes a new key each run.
