# First coordinated release (R1)

One release number for Commy and every SDK, so a developer knows which versions go together
(owner rule: one version number per project; pre-1.0 the *second* number is the breaking one). The
portfolio PM allocates the number. `commy-wire` takes the same number as the Commy release it
belongs to, because it is Commy's contract.

State below is as read on 2026-10-20; the SDK and dirty-ranges columns are those lanes' to correct.

## What is in place

| Piece | Version read | Where |
|-------|--------------|-------|
| Commy server | 0.12.0 on `main` (crates.io had 0.4.1 when last searched; the PM publishes) | this repo |
| `commy-wire` | 0.3.2 published | crates.io |
| `dirty-ranges` | 0.1.2 published | crates.io |
| `role-system` | 1.3.0 (used for grants) | crates.io |
| `commy-sdk-rust` | 0.10.0 on `main` against `commy-wire` 0.3.1 and `dirty-ranges =0.1.2`; crates.io shows 0.1.0 | SDK repo |

Server rows of the plan (C1 to C6, C8 to C10) are merged. R3's administration message is merged.

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
4. **No open layout question.** Today's known ones: whether `ServiceFilePath` should carry the
   generation (currently the new `file_id` is the handle), the maximum frame and range-list sizes
   (1 MiB and 1024), and the error-code list. Each is either answered in `spec/` or listed as a decision for
   the owner.
5. **Write detection per OS** (R2): the README of the SDK / `dirty-ranges` names every OS and architecture
   as TESTED or UNVERIFIED, and trapping is the default only where TESTED. Commy itself carries bytes and
   does not depend on this, but the release notes must say it.

## Sequence

1. Settle the open layout questions; PM allocates the release number *n*.
2. Publish `commy-wire` 0.*n*.0 with the layout marked frozen (spec, schema, vectors, README).
3. Commy 0.*n*.0 on that wire (pin the exact wire version).
4. SDK(s) 0.*n*.0 on the same wire (the Rust SDK's `dirty-ranges` pin follows its own policy).
5. Announce with the "known gaps" list below.

Every step is its own PR with a version line; none merges before the previous one is published.

## Known gaps to state in the release notes

- Authentication is development-only (`COMMY_ALLOW_DEV_AUTH=1`); a client names itself. (R4)
- Grants are in memory and lost on restart. (R5)
- The audit trail is in memory and on stderr. (R6)
- Limits and timeouts are environment variables, not a configuration file. (R7)
- Service files carry no per-file permissions; the unlisted tenant directory and a random name are the
  barrier on the server's machine. (R8)
- The tenant-directory ACL was not checked with a second real Windows account. (R9)
- The clustering code predates the redesign. (R10)

## Not in scope

Persistence of service contents, a layer that gives names to offsets (a future crate builds its
bookkeeping inside the file), and non-Rust SDKs beyond running the vectors.
