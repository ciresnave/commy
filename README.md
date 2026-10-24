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

`docs/DEPLOYMENT.md` has the settings (environment variables or a TOML file), a minimal production
start and Docker. In short: with `COMMY_ALLOW_DEV_AUTH` unset the server refuses to start without
`COMMY_AUTH_MASTER_SECRET`, and clients authenticate with an API key or JWT issued by
`auth-framework` for their tenant (`docs/AUTHENTICATION.md`). Service files live under `tenant_<id>/`
in the working directory.

## Known limits

- One server only: the clustering code is experimental, unused and compiled only with the Cargo feature `clustering` (`docs/CLUSTERING.md`).
- Service files carry no per-file operating-system permissions; a long random name inside an
  unlisted tenant directory is the only barrier on the server's machine.
- The audit log is durable but best effort, and hash-chained: `Server::verify_audit` detects modification, insertion and reordering of
  retained events, but not truncation: a log cut at the front or the tail verifies unless you recorded
  its head elsewhere (`Server::audit_head`). Events are appended under
  `<state dir>/audit/`; a write that fails is counted in `Server::audit_failures` and never blocks the
  operation it describes). The grant operation log is not compacted.
- Basic and custom credentials are refused; mTLS is later work.
- Markdown files outside `README.md`, `ROADMAP.md`, `CHANGELOG.md` and `docs/` may still describe the
  removed variable model (`tests/README.md` is about the storage-backend tests). Trust this README and
  `CHANGELOG.md`.

## Building and testing

```
cargo build
cargo test
```

`tests/storage_backends.rs` runs authentication against the in-memory backend only; tests against real databases wait on ROADMAP R21 (`tests/README.md`).
`tests/wss_loopback.rs` runs the real TLS and WebSocket server against real clients on a loopback port.

Licensed under MIT.
