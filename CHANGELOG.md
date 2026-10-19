# Changelog

## 0.8.1
- Fix: the crate did not compile on Linux and macOS (`E0432: unresolved import libc`): `src/allocator.rs` imported `libc` for an unused helper, while `libc` was an optional dependency no feature enabled. The dead helper (`process_is_alive`), the import and the optional `libc` dependency (and its implicit `libc` feature) are removed. CI now also runs `cargo test --all` on Linux and macOS (README and Cargo `description` now say the crate requires nightly Rust (`allocator_api`, `slice_ptr_get`); on stable 1.99 only 22 errors remain, all in `allocator.rs`, `containers.rs` and the `Service` allocator calls in `lib.rs`, which plan items C6/C9 remove). The full suite passes on Linux: 543 lib tests and every integration target, run in `rust:bookworm` with nightly).

## 0.8.0 (breaking)
- Rotation (C4) on `commy-wire` 0.2.4 (`spec/rotation.md`). `RotateRequest` with `RESIZE` runs a drain barrier: every member of the old file gets `RotationPending{deadline_ms}`, flushes and answers `RotationDrained`; at the deadline (default 5 s, `COMMY_DRAIN_TIMEOUT_MS`) the server builds the new file at a new random name (old bytes that fit, zero padded), switches the service to it, and sends `RotationCommit` to every member that drained. A member that did not gets `Loss{ROTATION_STRAGGLER}`. `REVOCATION` has no barrier: same size and bytes, `RotationCommit` to everyone but the revoked client, who gets `Loss{REVOKED}`.
- New permission `ServiceResize` (`service:resize`); resize needs it, `ServiceAdmin` or `TenantAdmin`; revocation needs `ServiceAdmin` or `TenantAdmin`. The new size must respect the server maximum.
- Limits: one rotation per service at a time, at least one second between rotations of a service, at most six rotations per requester per minute (`RATE_LIMITED`).
- After a commit, frames and `Resync` for the old `file_id` are refused (`INVALID_REQUEST`); `GetServiceFilePath` returns the new `file_id` with a fresh nonce. Each service's `seq` restarts at 0 and `generation` increases by one.
- Interim audit trail (`Server::audit_events`, also on stderr): every rotation request, grant, refusal and commit. A durable audit log is a later item.
- **Known gap:** a revocation only rotates the file and removes the client from it. It does not revoke the client's grants, so the client can fetch the new file again until its permissions are revoked (service-scoped grants, C10). Do not treat it as access revocation on its own.
- `ServiceRecord` gains `generation`; `Server` gains `rotations`, `retired_files`, `drain_timeout`, `rotation_min_interval`; `Forwarding::join` takes the client id.

## 0.7.1
- Fix: the server panicked at TLS start-up ("Could not automatically determine the process-level CryptoProvider") because the dependency tree enables both aws-lc-rs and ring. `TlsConfiguration::from_files` now builds its `ServerConfig` with an explicit aws-lc-rs provider.
- New `tests/wss_loopback.rs`: a self-signed certificate, the real `WssServer` accept loop and connection loop, and real TLS/WebSocket clients, covering TLS start-up, the outbound-queue select and the binary range-frame dispatch (closes the 0.7.0 known gap). Mutation-checked: dispatching on a wrong frame kind, or starving the outbound queue, fails the test.
- README documents the server's environment variables, including the `DATABASE_URL` default.

## 0.7.0 (breaking)
- Range forwarding (C3) on `commy-wire` 0.2.4. A session joins a file as a local mapper (after `ConfirmLocal`) or a remote mirror (after `Resync`). A local mapper reports changes with `RangesChanged` (no data); a remote sender writes range frames (binary, first byte 1) and ends the flush with `FlushEnd`. The server writes remote bytes into the file at once, gives each flush one `seq`, answers `FlushAck`, and forwards to every other member: local members get `RangesUpdated` (ranges only), remote members get range frames with the bytes read at forwarding time. A sender never gets its own ranges back. Refusals (`INVALID_REQUEST`, `NOT_FOUND` for another tenant's file, `PERMISSION_DENIED` without a write grant) forward nothing and do not spend a `seq`.
- The connection loop now has an outbound queue per connection and selects between the socket and that queue. Closed queues are pruned; `Disconnect` and connection end remove the session from every file.
- `ClientSession` gains `outbound` and `pending_flush`; `Server` gains `forwarder`. The `commy-wire` requirement is 0.2.4.
- Known gaps: the TLS/WebSocket loop (outbound-queue select, binary dispatch) has no automated test.

## 0.6.1
- `Resync` (C2): a client with a read grant on a file gets the whole file as `commy-wire` range frames (at most 1 MiB each, offset order, all carrying the service's current `seq`), then `ResyncDone{seq, size}`. The connection loop sends queued binary frames before the text reply. Unknown file `NOT_FOUND`; another tenant's file or no read grant `PERMISSION_DENIED`. `ServiceRecord` gains `seq`; `ClientSession` gains `outbound_frames`.

## 0.6.0 (breaking)
- `GetServiceFilePath` is real (C1) and now uses the `commy-wire` 0.2.1 messages: it needs a `request_id` and returns `ServiceFilePath` with the service's `file_id`. `path` and `nonce_path` are present only when the service has been opened to local mapping (`Server::set_trusted_local`, default off) and the caller holds a write grant (`ServiceWrite`, `ServiceAdmin` or `TenantAdmin`).
- `ConfirmLocal` checks the per-connection nonce (random, in a 0600 side file in the tenant directory), deletes it on the first attempt, and answers `ConfirmLocalAck` or `PERMISSION_DENIED`. At most 16 unconfirmed nonces per session (`RATE_LIMITED`); unconfirmed nonce files are removed on `Disconnect` and swept after 60 s.
- New replies use the `commy-wire` error spelling (`NOT_FOUND`, `PERMISSION_DENIED`, `INVALID_REQUEST`, `RATE_LIMITED`); the rest of the server still uses the old spellings until the spelling sweep.
- `ServiceRecord` gains `file_id` and `trusted_local`; `ClientSession` gains `pending_nonces` and `local_files`; `Server` gains `next_file_id`.

## 0.5.0 (breaking)
- `CreateService` requires `size` (bytes, a positive integer). The file is rounded up to whole 4 KiB pages and may not exceed the server maximum (default 10 MiB; `COMMY_MAX_SERVICE_SIZE` until config-file support exists). A missing, zero, negative or non-integer size is refused as `InvalidRequest`. The `Service` reply carries the final `size`.
- `Tenant::register_service` takes a `size` argument. `Server::max_service_size`, `Server::with_max_service_size`, `service_file_size`, `ServiceSizeError`, `PAGE_SIZE`, `DEFAULT_MAX_SERVICE_SIZE` are new.

## 0.4.1
- Untrack committed example build output (fixes "filename too long" on Windows clones). This is the release that ships the 0.4.0 security fix.

## 0.4.0 (breaking, security)
- Every operation except `Heartbeat`/`Disconnect` needs an authenticated session; `Authenticate` is refused unless `COMMY_ALLOW_DEV_AUTH=1`; tenant ids are validated (`[A-Za-z0-9_-]{1,64}`) before use in paths; `DeleteTenant` removes the tenant; services are checked against the session's tenant. GHSA-5q28-5hrh-w432, GHSA-rwg3-grc8-336p.
