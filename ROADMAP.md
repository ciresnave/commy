# Roadmap

Authoritative portfolio priorities live in the portfolio roadmap; this file records what is open in
Commy itself. State is `main` at 0.11.x unless a row says otherwise.

## Done (server side of plan v11)

C1 service file path and same-machine proof, C2 whole-file resync, C3 range forwarding, C4 rotation
(resize with prepare and commit, revocation), C5 required service size, C6/C9 named variables,
allocator, containers and file header removed (stable Rust), C8 tenant directories, C10 service-scoped
grants on role-system with the revocation rotation they trigger.

## Open

| Row | Item | State | Notes |
|-----|------|-------|-------|
| R1 | First coordinated release with the SDK(s) and the `commy-wire` layout freeze | open | Plan: `docs/RELEASE-PLAN.md`. Needs dirty-ranges 0.1.0 published and the Rust SDK's local mapping, trapping and rotation work merged. The wire layout stays unfrozen until then; any layout change is the next minor of `commy-wire`. |
| R2 | Non-Linux write-trap tests | open (SDK / dirty-ranges lane) | The trapping default is gated per OS on the plan's uncovered-cases list; the README must name each OS and architecture TESTED or UNVERIFIED. Commy itself only carries the bytes. |
| R3 | Local zero-copy sharing end to end | open | The server side exists (nonce handshake, `set_trusted_local`). Still missing: an administration message to open a service to local mapping (today only `Server::set_trusted_local`), and an SDK end-to-end run against the real binary. |
| R4 | Real authentication | in progress (0.15.0: verifier trait + wire types; next: auth-framework API key + JWT) | `CredentialVerifier`, identity from the credential subject, `commy-wire` 0.4.0 `Authenticate` / `AuthenticationResult`. The binary still installs no production verifier. mTLS later. Client provisioning is the framework's admin tooling, outside Commy. |
| R12 | Remove the MessagePack protocol (`WssMessage`) | after the wire freeze | A second, older protocol with its own `Authenticate` (development-only, refused unless `COMMY_ALLOW_DEV_AUTH=1`). |
| R5 | Persistent grants and service registry | done (0.13.0) | On PersistAnt: services as records, grants as an append-only operation log replayed at start-up; write-through and fail closed (a revoke or a close that cannot be recorded is refused and not applied); tested with a real kill and restart of the binary. Not stored: sessions, nonces, rotations in flight. The operation log is not compacted yet. |
| R6 | Durable audit log | done (0.14.0) | On PersistAnt, in a store of its own under the state directory; every `Server::audit` event (rotations, grants, local mapping, deletions, start-up) is appended with a timestamp; retention `COMMY_AUDIT_MAX_EVENTS`; best effort with a readable failure counter. Not tamper-evident (no signing or hash chain) and there is no wire message to read it yet: operators read the files or `Server::audit_history`. |
| R7 | Configuration file | open | The maximum service size, drain and fill timeouts are environment variables (`COMMY_MAX_SERVICE_SIZE`, `COMMY_DRAIN_TIMEOUT_MS`, `COMMY_FILL_TIMEOUT_MS`); the plan asks for a configuration file. |
| R8 | Per-file permissions (Q9) and read-only local access (Q10) | deferred | A service file has no permissions of its own; the unlisted tenant directory and a long random name are the barrier. Anyone who learns a path on the server's machine can open the file read-write. |
| R9 | Windows second-account check of the tenant directory ACL (C8) | open | Verified on Linux with a second account; on Windows only the ACL text and the owner's access were checked, not another real account and not antivirus. |
| R10 | `[[bin]]`/docs hygiene | open | `CLUSTERING_DEPLOYMENT.md` and the Docker guides predate the redesign (they carry a banner); they need rewriting against the bare-bytes server. `src/server/clustering` replicates the old variable model and has not been reworked for range forwarding. |
| R11 | Rust SDK parity and other SDKs | open (SDK lanes) | Every non-Rust SDK runs the `commy-wire` golden vectors in its own CI. |
