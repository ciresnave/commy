# Commy

Commy shares **bare bytes** between programs. A *service* is a memory-mapped file of the size its
creator asked for, zero filled, with nothing written into it by Commy: no header, no allocator, no
bookkeeping. What the bytes mean is up to the programs that use them. Programs on the server's
machine map the file directly; programs elsewhere keep a mirror and exchange only the byte ranges
that changed, over WebSocket Secure.

Builds on **stable Rust** (no nightly features).

## Pieces

| Piece | Where |
|-------|-------|
| Server (`commy` binary and library) | this repository |
| Wire contract: messages, JSON Schema, golden vectors, Rust types | [`commy-wire`](https://crates.io/crates/commy-wire) |
| Write detection (page-fault trapping, scan fallback) | [`dirty-ranges`](https://crates.io/crates/dirty-ranges) |
| Rust SDK | `commy-sdk-rust` |

## What the server does

- **Tenants and services.** `CreateService` takes a required `size` (rounded up to whole 4 KiB pages,
  at most the server maximum, default 10 MiB).
- **Local mapping.** For a service an administrator has opened to it, a client on the same machine
  with a write grant is offered the file's path plus a per-connection nonce file; echoing the nonce
  over the authenticated connection proves it shares the filesystem. Local clients write straight
  into the mapping and report the changed ranges.
- **Remote mirrors.** Other clients receive the file once (`Resync`), then every flush as range
  frames carrying the bytes. Order is the server's: one sequence number per flush.
- **Rotation.** A service's file can be replaced: *resize* (drain barrier, then the requester fills
  the new file, then commit or abort) and *revocation* (the file moves; the revoked client is dropped).
- **Access control.** Grants are per client, tenant and service (a tenant-wide grant is shorthand for
  every service), checked on every request that names a service; `Grant`, `Revoke` and `ListGrants`
  administer them, and revoking a client from a service rotates that service's file. The engine is
  [`role-system`](https://crates.io/crates/role-system).

The protocol is specified in `commy-wire` (`spec/`), including the flush, rotation and grant rules.
It is **not frozen** until the first coordinated release.

## Running the server

| Variable | Meaning |
|----------|---------|
| `COMMY_TLS_CERT_PATH`, `COMMY_TLS_KEY_PATH` | PEM certificate and PKCS#8 key; both are needed for WSS. |
| `COMMY_LISTEN_ADDR`, `COMMY_LISTEN_PORT` | Where to listen (see the startup log for the defaults). |
| `COMMY_ALLOW_DEV_AUTH=1` | Accept any credential as admin. Development only; never on a reachable server. Without it nobody can authenticate yet: no real credential verifier is wired in. |
| `COMMY_MAX_SERVICE_SIZE` | Largest service file in bytes a client may request (default 10 MiB). |
| `COMMY_DRAIN_TIMEOUT_MS` | How long a resize waits for every client to drain (default 5000). |
| `COMMY_FILL_TIMEOUT_MS` | How long a resize requester has to fill the new file (default 30000). |
| `DATABASE_URL` | Backing store for tenant authentication data. **If unset the binary sets it to `postgresql://postgres:postgres@127.0.0.1:5432/commy`**, so a bare run expects a PostgreSQL there. Any value that is not `postgresql://` or `mysql://` (for example `memory`) selects in-memory storage, which needs no database. |

Service files live under `tenant_<id>/` in the server's working directory.

## Known limits

- Authentication is development-only (`COMMY_ALLOW_DEV_AUTH`); grants are in memory and are lost on
  restart.
- Service files carry no per-file operating-system permissions; a long random name inside a tenant
  directory is the only barrier on the server's machine.
- The audit trail is in memory (and on stderr); a durable log is not built yet.
- Many of the Markdown files in this repository predate the redesign and describe named variables,
  the shared allocator and the containers, which were removed in 0.11.0. Trust this README and
  `CHANGELOG.md`.

## Building and testing

```
cargo build
cargo test
```

Tests that need an external database are in `tests/storage_backends.rs` (see `tests/README.md`).
`tests/wss_loopback.rs` runs the real TLS and WebSocket server against real clients on a loopback port.

Licensed under MIT.
