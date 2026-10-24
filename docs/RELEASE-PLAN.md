# First coordinated release (R1)

One release number for Commy and every SDK, so a developer knows which versions go together
(owner rule: one version number per project; pre-1.0 the *second* number is the breaking one). The
portfolio PM allocates the number. `commy-wire` takes the same number as the Commy release it
belongs to, because it is Commy's contract.

State below is as read on 2026-10-20 at Commy `main` 0.18.2 (this release: 0.19.0); the SDK and dirty-ranges rows are those lanes' to correct.

## What is in place

| Piece | Version read | Where |
|-------|--------------|-------|
| Commy server | 0.18.2 on `main` (crates.io had 0.4.1 when last searched; the PM publishes) | this repo |
| `commy-wire` | 0.4.0 published; 0.5.0 (drops `RotationCommit.generation`) is what this release speaks, published by the PM first | crates.io |
| `dirty-ranges` | 0.1.2 published | crates.io |
| `role-system` | 1.3.0 (used for grants) | crates.io |
| `commy-sdk-rust` | 0.13.0 on `main` against `commy-wire` 0.4.0 and `dirty-ranges =0.1.2`, both from crates.io; crates.io shows 0.1.0 (SDK lane, 2026-10-20) | SDK repo |

Server rows of the plan (C1 to C6, C8 to C10) are merged, with R3's administration message (`SetLocalMapping`), R4 authentication, R5 durable state and R6 the durable audit log.

## Gate: what must be true before the wire layout is frozen

1. **Every message in `commy-wire` is used by both sides.** Server and Rust SDK both implement and test
   range frames, `RangesChanged` / `RangesUpdated` / `FlushEnd`, `Resync`, the local-mapping handshake
   and `SetLocalMapping`, rotation (`RotateRequest` to `RotationCommit` / `RotationAborted`), grants and
   `Loss`. A message nobody uses is removed rather than frozen.
2. **Conformance.** Every SDK runs the golden vectors (`vectors/`) in its own CI; the Rust crate already
   does. A second SDK, when it exists, runs them too before it ships.
3. **An end-to-end run against the real binary** covers: remote mirror, local mapping with the nonce,
   a resize with a remote fill and with a local fill, a straggler, a revocation by `Revoke`, and an
   abort. The server's `tests/wss_loopback.rs` is the server-side start; the SDK's e2e job is the other half.
4. **No open layout question: closed, board 235.** `ServiceFilePath` carries no generation (the new
   `file_id` is the handle; which format a service uses is the service's to say, not Commy's), and the
   maximum frame (1 MiB) and range-list (1024) sizes stay. The owner also ruled that Commy does not track a
   rotation count at all, so `RotationCommit.generation` is removed (`commy-wire` 0.5.0).
5. **Write detection per OS** (R2): the README of the SDK / `dirty-ranges` names every OS and architecture
   as TESTED or UNVERIFIED, and trapping is the default only where TESTED. Commy itself carries bytes and
   does not depend on this, but the release notes must say it.

## Gate status (read 2026-10-23)

Refs: Commy `main` 956bb5e, `commy-sdk-rust` `main` 8838377, `dirty-ranges` `main` 303ed42, `commy-wire` 0.5.0 from crates.io. Read from the repos and crates.io, not run again by the Commy lane except where it says so.

1. **Met, as name presence only.** All 30 `ControlMessage` variants of `commy-wire` 0.5.0 appear in Commy's `src` and in the SDK's `src` (a `git grep` of each variant name, both repos). That shows nobody ships a message the other side lacks; it does not show each one is exercised, which gate 3 does by behaviour.
2. **Met for the Rust SDK.** `tests/wire_vectors.rs` is in its tree and its `test (ubuntu, windows, macos)` jobs passed at 8838377 (CI run of 2026-10-21). No second SDK exists yet.
3. **Met.** The SDK's `e2e (real commy server)` job builds the real `commy` binary at commit bcbf961 (0.19.0) and passed at 8838377. `tests/e2e.rs` covers the remote mirror, local mapping, a resize filled remotely and one filled locally, the straggler, an abort, grants and revoke, and a restart.
4. **Closed**, board 235 (above).
5. **Met.** The `dirty-ranges` README at 303ed42 lists every OS and architecture as TESTED or UNVERIFIED, with trapping default only where TESTED. Its UNVERIFIED rows (Linux aarch64 musl, 64 KiB pages, 32-bit) and the macOS blocked-signal case must go in the release notes, as step 5 says.

What is left is step 2 of the sequence: `commy-wire` still says "layout unfrozen" in its 0.5.0 README, and Commy's README says "not frozen until the first coordinated release". Marking it frozen is the owner's decision.

## Sequence

1. Settle the open layout questions; PM allocates the release number *n*.
2. Publish `commy-wire` 0.*n*.0 with the layout marked frozen (spec, schema, vectors, README).
3. Commy 0.*n*.0 on that wire (pin the exact wire version).
4. SDK(s) 0.*n*.0 on the same wire (the Rust SDK's `dirty-ranges` pin follows its own policy).
5. Announce with the "known gaps" list below.

Every step is its own PR with a version line; none merges before the previous one is published.

## Known gaps to state in the release notes

- Authentication is API key or JWT checked by `auth-framework`, one database per tenant; creating tenants, users and keys is that framework's tooling, not Commy's (`docs/AUTHENTICATION.md`). Basic and custom credentials are refused, mTLS is later. The old MessagePack protocol was removed in 0.21.0 (R12).
- The grant operation log and the audit log are durable but not compacted. The audit log is hash-chained: it detects modification, insertion and reordering, not truncation (a cut tail or front is found only against a head recorded elsewhere).
- Settings come from environment variables or a TOML file (`COMMY_CONFIG`, `commy.example.toml`); no hot reload, and the verifier and rotation interval are not file settings. (R7, R26)
- Service files carry no per-file permissions; the unlisted tenant directory and a random name are the
  barrier on the server's machine. (R8)
- The tenant-directory ACL was checked with a second real Windows account on 2026-10-23 (R9; `tests/windows_second_account.rs`, needs AntSpawn's account setup, so GitHub CI does not run it); antivirus was not checked.
- The clustering code predates the redesign. (R10)

## Not in scope

Persistence of service contents, a layer that gives names to offsets (a future crate builds its
bookkeeping inside the file), and non-Rust SDKs beyond running the vectors.
