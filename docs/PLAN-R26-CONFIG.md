# R26: hot reload, and the verifier as a file setting

**Status: proposal, nothing built.** Read at Commy 0.27.1 (`src/config.rs`, `src/main.rs`, `Server::configure`). The rotation interval, the third item of R26, is done in 0.27.1.

## Today

Settings are read once at start-up: `Settings` (environment, then `commy.toml`) feeds `Server::configure`, which copies the values into public fields of `Server`, which lives in an `Arc<RwLock<Server>>`. The authentication verifier is built in `main.rs` from `FrameworkVerifier::from_env` (or none with development authentication, which the file may not switch on).

## Hot reload

**Which settings could change live.** Safe, because the server reads the field on each use: `fill_timeout_ms`, `drain_timeout_ms`, `rotation_min_interval_ms`, `max_service_size`, `audit_max_events`, `grant_compact_after`. Not live: `listen_addr`, `listen_port`, `state_dir`, `auth_dir` (they are opened once), `cluster_*` (ignored). TLS certificate and key paths are the one worth having live (certificate renewal), but swapping them is a separate change to `WssServer` and is not part of this note. Environment variables never change after start, so a reload re-reads the file only, and the environment still wins.

**How a reload is triggered.**

| Option | Cost | Note |
|---|---|---|
| A. Poll the file's modified time every few seconds | none; no new dependency; same on Windows and Unix | latency of one interval |
| B. SIGHUP | Unix only; this project runs on Windows too | would need A anyway |
| C. A file-watch crate | new dependency, platform backends | faster, but nothing here needs seconds-fast |
| D. An admin wire message | a `commy-wire` change; it must precede the R1 freeze (R24 has the same constraint) | also needs a permission and an audit design |
| E. Do not build it | a restart applies everything; state is durable (R5) and clients reconnect | costs dropped connections per change |

**Recommendation: A.** Poll the mtime and length, parse with the existing `Settings` code, and apply the live-safe set under the write lock. A file that does not parse, or that holds a key refused by the file rules, is rejected whole and the old values stay (fail closed, as R5). Each applied change, and each rejected reload, is one audit event naming the key and the old and new value, never a secret. A changed not-live key is logged as "needs a restart" and ignored. Test: write a file, change it, assert the field changes; write a bad file, assert nothing changes and the audit says why; real-process test with the binary. Cost: small, one new module and `Server::configure` split into "apply live settings" and "start-up only".

## The verifier as a file setting

Today there is one verifier, so a setting such as `auth_verifier = "framework"` has one legal value and adds a key that only fails when it is wrong. The development switch is refused in a file on purpose. A real choice appears only when R25 adds methods (mTLS, Basic, custom credentials), and R25 is held for a CireSnave decision.

**Options.** (1) A single-valued key now: no benefit. (2) After R25: `auth_methods = ["api_key", "jwt", "mtls"]`, a list of accepted credential methods, each one a built-in verifier; unknown names fail like unknown keys. (3) A pluggable verifier named in the file: loads code by name, which is a security surface this project should not add.

**Recommendation: defer to R25 and then take (2).** Nothing to build now.

## Needs CireSnave

Nothing new. R26's second half waits on R25 (already asked of CireSnave via the PM). Hot reload by option A needs no decision outside the lane unless the PM wants it ahead of other rows (it is marked low).
