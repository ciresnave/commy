# Changelog

## 0.13.3
- **One error-code spelling on the SDK protocol.** The legacy handlers sent `NotFound`, `Unauthorized`, `InvalidRequest` and `InternalError` (and the MessagePack path `PARSE_ERROR` / `INVALID_JSON`), which `commy-wire` does not allow and clients read as unknown. They now send `NOT_FOUND`, `INVALID_REQUEST`, `INTERNAL_ERROR`; "not authenticated" is `UNAUTHENTICATED`; a session that is authenticated but for another tenant, or lacks access, is `PERMISSION_DENIED` (the old `Unauthorized` covered both); a malformed MessagePack or JSON frame is `INVALID_REQUEST`.
- An `Error` reply to an SDK-protocol request that carried a `request_id` echoes it.
- Breaking for a client that matched the old CamelCase codes (the Rust SDK reads them as unknown and is adding aliases meanwhile).

## 0.13.2
- **`DeleteService` is real.** With `service:admin` on the service it removes the service from the registry and the tenant, retires its `file_id` (frames and `Resync` for it are refused), drops its service-scoped grants (tenant-wide grants stay), removes the file, tells every member of the file with `Loss{REVOKED}` and answers `Result{success:true}`. It is refused (`RATE_LIMITED`) while a rotation of the service is running, and `NOT_FOUND` for an unknown or already deleted name.
- **Durable and fail closed.** With persistence, a deletion is recorded first as a tombstone (`deleted/<id>`, an atomic write) and refused (`INTERNAL_ERROR`) if it cannot be recorded; the stored service row is then removed, and a leftover row is ignored because the tombstone wins. A restart does not bring a deleted service back.
- `Tenant::remove_service`, `Server::forget_row`, `persist::Tombstone`; `Persistence::delete_service` now takes the tenant and name.
- Known: the file is removed best effort (Windows refuses while a local client still has it mapped; audited); the grant log still grows.

## 0.13.1
- **`DeleteService` no longer lies.** It answered `{"type":"Result","success":true,"message":"Service deleted successfully"}` while deleting nothing. It now resolves the service, needs `service:admin` on it (`PERMISSION_DENIED` otherwise, `NOT_FOUND` for an unknown name) and answers `NOT_IMPLEMENTED` ("the service was not deleted"). The old tenant-level `ServiceDelete` permission check is gone: service-scoped admin replaces it. The real deletion follows.
- `GetService` (by name) only returns the id of a service the client can read; one it cannot reach looks like one that does not exist (it used to check only a tenant-level `ServiceRead`, so a client denied on one service could still learn its id).

## 0.13.0
- **R5: durable state** on PersistAnt 0.8.21. The service registry (service id, tenant, name, file path, `file_id`, generation, whether it is open to local mapping) is stored as one record per service; the grants as an append-only log of operations replayed at start-up; the creator's `service:admin` grant, `Grant`, `Revoke`, `SetLocalMapping` and every rotation commit are recorded. `COMMY_STATE_DIR` names the directory (default `./commy_state`, created private to the server and administrators; `off` disables it). A restart finds the same service ids, files, `file_id`s and grants. Sessions, nonces, rotations in flight and per-file sequence numbers are not stored (clients resynchronise).
- **Fail closed.** The change is recorded first and refused (`INTERNAL_ERROR`, audited) if it cannot be: a revoke that was not recorded is not applied, so a restart cannot give the access back; closing local mapping, creating a service and a rotation commit follow the same rule.
- New: `Server::enable_persistence`, `Server::use_persistence` (tests use PersistAnt's memory and fault-injecting fakes), `Server::grant` / `Server::revoke` (record-then-apply), `Server::set_trusted_local_recorded`, `StateReport`, `persist::{Persistence, ServiceRow, GrantOp}`, `private_dir::create_secret`. Handlers now use `Server::grant` / `revoke`.
- Tests include a real kill of the `commy` process and a restart on the same state directory (`tests/restart.rs`).
- Known: the operation log grows with every grant change and is not compacted; a stored service whose file is gone is skipped and counted, not deleted; `DeleteService` is still a stub that deletes nothing.

## 0.12.1
- Docs only: `docs/RELEASE-PLAN.md`, the plan for the first coordinated release with the SDK(s) and the `commy-wire` freeze (gate, sequence, known gaps); `ROADMAP.md` R1 points to it.

## 0.12.0
- R3: an administrator opens a service to local mapping over the wire. New `SetLocalMapping{request_id, service_id, enabled}` (answered by `LocalMappingAck`) on `commy-wire` 0.3.2; needs `service:admin` on the service, is audited, and a refused request changes nothing. A service stays closed until opened, as before, so the binary now offers `path` and `nonce_path` (to a client with a write grant) without a code call to `Server::set_trusted_local`. Closing stops future offers.
- New real-WSS test: open, get the path and nonce file, confirm, over TLS and WebSocket against the real server.

## 0.11.4
- Documentation clean-up. Deleted 42 root-level Markdown files that described the removed design (named variables, the shared allocator, the containers, the nightly toolchain) or were status reports, phase reports, tutorials and WebSocket notes from earlier iterations; they remain in git history. Kept the Docker, clustering and authentication guides with a banner saying they predate 0.11.0. `.github/copilot-instructions.md` rewritten for the current design. New `ROADMAP.md`. No code changes.

## 0.11.3
- Tenant directories (C8): a newly created `tenant_<id>` directory can be traversed by other accounts but listed only by the Commy server and administrators. Unix: mode `0711`. Windows: inheritance removed; SYSTEM, Administrators and the running account get full control (inherited); `Users` get traverse on the directory and modify on the files inside (inherit-only), via `icacls`. This keeps the plan's barrier (a long random file name in a directory others cannot list; a local client opens the file by its full path) without per-file permissions (Q9/Q10, deferred). An existing directory is left as the operator set it; if the restriction cannot be applied the directory is not used.
- The local-mapping nonce file is now owner-only on Windows too (it used to inherit the directory's grant to other accounts); Unix was already `0600`.
- Verified: ACL text and mode in tests on Windows and Linux; on Linux also by hand with a second account (`nobody` could open a file by its full path and could not list the directory). **Not verified on Windows with a second real account** (the plan's spike already noted this), nor with antivirus.

## 0.11.2
- Fix (dev authentication identity): `Authenticate` never set `client_id`, so grants were filed under the session id, which no client ever learned: a grant could not target a client over the wire and a client holding no grant could not exist. Now `Authenticate` honours `client_id` (1 to 128 characters of letters, digits and `_ - . @ :`; anything else is refused), falls back to the session id when none is given, and returns the id in `AuthenticationResult.client_id`. The MessagePack `Authenticate` sets it too. Development authentication still trusts the claim, so any dev client can claim any id; it is development only.
- New, development authentication only: `dev_scopes` in `Authenticate` (a list of scopes such as `["read"]`, or `[]` for none) gives exactly those permissions instead of administrator, so a test client can be a client that holds no grant. Absent means administrator, as before.

## 0.11.1
- Security hygiene: `dev-key.pem`, `dev-key-temp.pem`, `dev-cert.pem`, `dev-cert-temp.pem`, `dev-cert.pfx` and four log/output files (`commy_server.log`, `commy_test.log`, `server_debug.log`, `commy_output.txt`) were tracked in git and shipped inside every published crate from 0.1.0 to 0.11.0. They are untracked and ignored (`*.pem`, `*.pfx`, `*.log`). The key is a throwaway self-signed development key (subject `O=Commy Dev, CN=127.0.0.1`, valid Feb 2026 to Feb 2027), referenced only by the test scripts that generate their own; it protects nothing real, but treat it as public. The logs contained the test database URL with the test password of a local test container. They remain in git history and in the already-published crates.

## 0.11.0 (breaking)
- **Named variables are gone (C6).** Removed: the SDK-protocol handlers `AllocateVariable`, `ReadVariable`, `WriteVariable`, `DeallocateVariable`, `Subscribe`, `Unsubscribe`, `ReportVariableChanges`; the MessagePack-protocol `GetVariables`, `SetVariables`, `Subscribe` handlers and the `WssMessage` variants `GetVariables`, `VariablesData`, `SetVariables`, `VariablesUpdated`, `VariableChanged`, `Subscribe`, `SubscriptionAck` (and their routing); the by-name methods on `Service` (`allocate_variable`, `deallocate_variable`, `get_variable[_mut]`, `get`, `get_mut`, `detect_changes`, `handle_changes`, `register_watcher`), `SharedData`, `find_changed_variables`, `VariableMetadata`, `ServiceWatcherRegistry`, `VariableWatcher`. A service that needs names builds its own table of names, types, offsets and lengths on top of Commy. The old message types are now unknown types.
- **Bare files (C9).** `FreeListAllocator`, the `Shared*` containers and the 4 KiB file header are removed (`allocator.rs`, `containers.rs`, the `allocator_api` / `slice_ptr_get` features). A `Service` is a plain memory mapping (`len`, `as_slice`, `as_mut_slice`, `flush`, `path`); Commy writes nothing into a service file, which is the requested number of zero bytes from offset 0. Removed with them: the two example programs that used named variables (financial ticker, chat), the allocator stress examples, `tests/comprehensive_tests.rs`, `tests/integration_test.rs` and the Markdown guides under `examples/` that covered them.
- **Stable Rust.** The crate no longer needs nightly: `rust-toolchain.toml` is gone, the crate description no longer says nightly, CI builds and tests on stable (Windows, Linux, macOS), and the README says stable. Verified with `cargo +stable test` (431 lib tests and all integration targets).
- README rewritten for the bare-bytes design. The many older Markdown files in the repository root still describe the removed design; they are stale and a separate clean-up.

## 0.10.1
- Service-scoped access control, part 2, on `commy-wire` 0.3.1: `Grant`, `Revoke` and `ListGrants` (+ `GrantAck`, `Grants`). Granting or revoking at service scope needs `service:admin` on that service; tenant-wide changes and handing out `tenant:admin` need tenant admin. Unknown permission names, empty lists and unknown services are refused. `ListGrants` shows a tenant admin everything, a service admin that service, anyone else their own entries. Every grant, revoke and refusal is audited.
- **Revocation now closes the C4 gap.** When a `Revoke` removes a client's last access to a service (including a tenant-wide revoke, for each service lost), the server starts a revocation rotation of that service: the file moves to a new random name, the remaining members get `RotationCommit`, and the revoked client gets `Loss{REVOKED}` and, holding no grant, cannot fetch the new file. It happens even if the client never joined the file (it may have been offered the path). Revocation rotations are exempt from the spacing and per-requester limits (`Rotations::begin_forced`) but still wait for a running rotation.

## 0.10.0 (breaking)
- Service-scoped access control (C10, part 1: enforcement), on role-system 1.3.0 (owner decision: reuse it). Grants are per client, tenant and service; a tenant-wide grant is shorthand for all services; `service:admin` implies write, read and resize; `service:write` implies read; revoking one service denies it even for a tenant-wide grant or tenant admin (role-system deny).
- Every handler that names a service or a file of one now checks the session against it: `GetServiceFilePath` (read; the path only with write), `Resync` (read), range frames, `RangesChanged`, `FlushEnd` (write), `RotateRequest` (resize needs `service:resize`, revocation `service:admin`), and the guard refuses any message naming a service the client has no access to (`PERMISSION_DENIED`), which also closes the older variable handlers.
- A session's authentication permissions are imported as tenant-wide grants the first time they are checked; the creator of a service is granted `service:admin` on it. Previously a tenant-level permission reached every service and nothing was checked per service.
- New: `server::grants` (`GrantStore`, `RoleSystemGrants`, `Action`, `Spec`), `Server::grants`. Storage is in memory; persistence is a later item. The grant/revoke/list messages and the revocation rotation they trigger follow in part 2 (needs `commy-wire` 0.3.1).

## 0.9.0 (breaking)
- Resize is now **prepare and commit** (plan Q11(b)), on `commy-wire` 0.3.0. After the drain barrier the server creates the new file (zero filled, new random name, not live) and hands it to the **requester only** (`RotationPrepared`; a local requester gets `path`, a remote one sends range frames for `new_file_id` and `FlushEnd`). The requester fills it from the old file in the new shape and answers `RotationFilled`; the server then makes it live and sends `RotationCommit`. If the requester sends `RotationAbort` or does nothing within `fill_timeout` (default 30 s, `COMMY_FILL_TIMEOUT_MS`), the prepared file is discarded, the old file stays live and every member hears `RotationAborted` (no one is dropped). Stragglers are dropped with `Loss{ROTATION_STRAGGLER}` only at a commit. `RotationPending` carries `fill_ms`.
- **What 0.8.0 did instead (do not rely on it):** a resize in 0.8.0 had the server copy the first `min(old, new)` bytes into the new file itself, with no prepare step and no requester fill. That was a divergence from the approved design and is gone. Code written against 0.8.0 resize must now fill the new file itself. REVOCATION is unchanged (the server keeps the old bytes).
- New: `Server::prepared`, `Server::fill_timeout` / `with_fill_timeout`, `PreparedFile`, `rotation::{Phase, FillResult}`; a prepared file is visible to the requester session alone (everyone else gets `NOT_FOUND`).

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
