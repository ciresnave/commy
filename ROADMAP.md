# Roadmap

Authoritative portfolio priorities live in the portfolio roadmap; this file records what is open in
Commy itself. State is `main` at 0.16.x unless a row says otherwise.

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
| R3 | Local zero-copy sharing end to end | server side done (0.12.0) | The nonce handshake and `SetLocalMapping` exist; the SDK lane confirmed the flow end to end. The SDK has an end-to-end CI job against the real binary (`tests/e2e.rs`, pinned to a Commy commit it repins at each release), which is gate 3 of the release plan. |
| R4 | Real authentication | done in 0.16.0 (verifier trait + wire types in 0.15.0; auth-framework API key + JWT in 0.16.0) | `CredentialVerifier`, identity from the credential subject, `commy-wire` 0.4.0 `Authenticate` / `AuthenticationResult`. The binary installs `FrameworkVerifier` unless dev auth is on; see `docs/AUTHENTICATION.md`. mTLS later. Client provisioning is the framework's admin tooling, outside Commy. |
| R12 | Remove the MessagePack protocol (`WssMessage`) | after the wire freeze | A second, older protocol with its own `Authenticate` (development-only, refused unless `COMMY_ALLOW_DEV_AUTH=1`). |
| R5 | Persistent grants and service registry | done (0.13.0) | On PersistAnt: services as records, grants as an append-only operation log replayed at start-up; write-through and fail closed (a revoke or a close that cannot be recorded is refused and not applied); tested with a real kill and restart of the binary. Not stored: sessions, nonces, rotations in flight. The operation log is not compacted yet. |
| R6 | Durable audit log | done (0.14.0) | On PersistAnt, in a store of its own under the state directory; every `Server::audit` event (rotations, grants, local mapping, deletions, start-up) is appended with a timestamp; retention `COMMY_AUDIT_MAX_EVENTS`; best effort with a readable failure counter. Hash-chained since 0.19.2: it detects modification, insertion and reordering of retained events, but not truncation (a cut tail or front verifies unless `audit_head` was recorded off the machine; R13 below). There is no wire message to read it yet: operators read the files or `Server::audit_history`. |
| R7 | Configuration file | done (0.17.0) | TOML, `COMMY_CONFIG` or `./commy.toml`; environment beats file beats default; unknown keys fail; secrets and the dev-auth switch are refused in a file. See `commy.example.toml`. Not yet: hot reload, and `FrameworkVerifier`/rotation interval are not file settings. |
| R8 | Per-file permissions (Q9) and read-only local access (Q10) | deferred | A service file has no permissions of its own; the unlisted tenant directory and a long random name are the barrier. Anyone who learns a path on the server's machine can open the file read-write. |
| R9 | Windows second-account check of the tenant directory ACL (C8) | open | Verified on Linux with a second account; on Windows only the ACL text and the owner's access were checked, not another real account and not antivirus. |
| R13 | Anchor the audit head off the machine | open | An API to export `audit_head` to the operator's store (or a sink they name) so a cut tail or front is detectable. |
| R10 | Docs hygiene and clustering | done: docs (0.17.1), feature gate (0.18.0) | The stale deployment, Docker and authentication guides are replaced by `docs/DEPLOYMENT.md`, `docs/CLUSTERING.md` and `docs/AUTHENTICATION.md`. `src/server/clustering` replicates the old variable model, is unused by the server and is not to be reworked while the FAM to Synapse merge is open; it is behind the default-off Cargo feature `clustering`. |
| R11 | Rust SDK parity and other SDKs | open (SDK lanes) | Every non-Rust SDK runs the `commy-wire` golden vectors in its own CI. |
