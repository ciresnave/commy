# R12: remove the MessagePack protocol (`WssMessage`)

**Status: executed in 0.21.0 (PR #53).** The PM approved options A1, B-now, C-direct, D-none, on two conditions, both met: a crates.io reverse-dependency check (none) and a CHANGELOG naming the replacements. The new loopback tests were run against `main` as a known-bad control and fail there (comment on #53). This file is kept as the record of the inventory and the decisions; the text below is the plan as it was evaluated, read at Commy `main` 956bb5e (0.20.0) and `commy-sdk-rust` 8838377.

## What it is

`src/protocol.rs` defines `WssMessage`, 18 variants sent as MessagePack in WebSocket **binary** frames. It is the pre-redesign protocol. The real protocol is JSON text frames (SDK messages) plus binary range frames starting with `commy_wire::KIND_RANGE`; `commy-wire` does not contain `WssMessage`, so removing it does not touch the wire layout or the freeze.

## Does it lose us anything?

Variant by variant, what runs today and what takes its place. "Production" means reachable by a client on a running server with default settings.

| Variant(s) | Reachable in production? | Real replacement | Lost |
|---|---|---|---|
| `Authenticate`, `AuthenticationResponse` | Only with `COMMY_ALLOW_DEV_AUTH=1`; otherwise answered with the refusal text and nothing else. Hands out an admin session and a `dev-token-<millis>` string. | JSON `Authenticate` (API key or JWT through `auth-framework`; dev auth also works there, the e2e job uses it). | Nothing. |
| `Heartbeat`, `HeartbeatAck` | Yes (any session). | JSON `Heartbeat`. | **One side effect**: the MessagePack heartbeat sets `session.last_heartbeat_ack`; the JSON one does not. The only readers are `liveness.rs` and `ClientSession::is_healthy`, and `liveness` is not used by the server (no reference outside its own file). Add the one-line set to the JSON heartbeat so the field keeps meaning what it says. |
| `Logout`, `LogoutResponse` | Yes, after a dev-auth login only in practice. Clears token, permissions, tenant, client id and subscriptions; keeps the socket. | JSON `Disconnect` leaves the shared files and ends the session. | A "log out but keep the socket open" step. Nothing in the repo or the Rust SDK calls it. Not replaced (see open question 3). |
| `RefreshToken`, `TokenRefreshResponse` | Yes, but always answers `success: false, "requires re-authentication"`. | Authenticate again with a fresh credential. | Nothing: it never succeeded. A test pins that it fails. |
| `Error` | Server-bound `Error` is logged and dropped. | JSON `Error` replies. | Nothing. |
| `Ack` | No handler. | None needed. | Nothing. |
| `PermissionRevoked`, `FileMigration`, `MigrationAck`, `CheckPermission`, `PermissionResponse` | **No sender.** Built only by `RevocationManager::create_revocation_message` / `create_migration_message`, which nothing but tests call. | Revocation reaches a connected client as `commy_wire::Loss` with `LossKind::Revoked`, and the file move as the rotation messages (`RotationPending` ... `RotationCommit`). Both are tested in `ws_handler.rs`. | Nothing in behaviour. The two constructors go. |
| `TokenSync`, `ClusterPing`, `ClusterPingResponse` | Only in `src/clustering.rs`, behind the default-off `clustering` feature (experimental, unused). | None. | Nothing in a default build. Under the feature, clustering needs its own two message types (see Decision A). |

Other things that disappear with it, none of them used by the real protocol: `MessageRouter` and `RoutingDecision` (`src/server/message_router.rs`, 291 lines, exported from `lib.rs`; no production code calls `route`), `WssServer::broadcast_to_tenant` (takes a message, prints a line, sends nothing), the binary-frame MessagePack branch and the "invalid JSON" reply (which answers a text frame with a MessagePack `Error`) in `ws_handler.rs`, and the `rmp-serde` dependency.

**The Rust SDK does not use it**: no `WssMessage` or `rmp_serde::to_vec/from_slice` call in `commy-sdk-rust` `src` or `tests` (the crate keeps `rmp-serde` only for two error-conversion variants in `error.rs`). No other SDK exists. So no consumer lane is affected by the protocol itself.

## What gets added to keep the behaviour

1. JSON `Heartbeat` sets `session.last_heartbeat_ack` (one line, with a test).
2. Binary frames that are not range frames get an explicit **JSON text** `Error` reply (`INVALID_REQUEST`, "binary frames must be range frames") instead of a MessagePack parse attempt. Today a non-range binary frame is parsed as `WssMessage`; after removal it must still be refused loudly, and the same for text that is not JSON (also replied in JSON, not MessagePack).
3. Nothing else. In particular no replacement for `Logout`, `RefreshToken`, `PermissionRevoked`, `FileMigration` (see above).

## Steps

One PR, `feat!`, because the pieces do not compile apart. Version **0.21.0** (pre-1.0 breaking is the second number): `commy::protocol::WssMessage`, `MessageRouter` and `RoutingDecision` are public.

1. `src/protocol.rs`: delete `WssMessage` and its `impl`; keep `ClientSessionId`, `ClientState`, `ClientSession` and their tests.
2. `src/server/ws_handler.rs`: delete `handle_message` and the `Message::Binary` MessagePack branch; add the explicit refusals above; delete the tests of the removed variants (10 test functions in this file mention `WssMessage::`, counted by the `#[test]` attribute above each mention: heartbeat, logout, refresh, MessagePack `Authenticate`, and the two near lines 5850 and 6970; the tests of `WssMessage` in `src/protocol.rs` from line 259 go with step 1) and port the ones that protect a real property:
   * dev auth refused by default: the JSON path has an equivalent (`dev_auth_off_tests`); confirm it covers both refusals the MessagePack tests covered (tenant id syntax, client id syntax) and add what is missing.
   * the JSON `Heartbeat` test gains the `last_heartbeat_ack` assertion.
3. Delete `src/server/message_router.rs`, the `lib.rs` re-export, `WssServer::broadcast_to_tenant` (and its test), and the two constructors in `src/revocation.rs` with their tests.
4. `src/clustering.rs` (feature `clustering`): see Decision A.
5. `Cargo.toml`: drop `rmp-serde`; `Cargo.lock` loses `rmp` and `rmp-serde` if nothing else needs them (check with `cargo tree -i rmp-serde`).
6. Docs: `docs/AUTHENTICATION.md` (the MessagePack paragraph), `docs/RELEASE-PLAN.md` known-gaps line, `ROADMAP.md` R12 -> done, `src/server/mod.rs` header comment, CHANGELOG "Breaking: ...".

Size: not counted. `message_router.rs` is 291 lines, `WssMessage` and its tests are most of `protocol.rs` (483 lines, 48 mentions), plus the MessagePack branch, `handle_message` and 10 tests in `ws_handler.rs`; the additions are small (a line in the heartbeat, two refusal replies, tests).

## Verification

* `cargo test --locked` (lib + `restart`, `storage_backends`, `wss_loopback`) and `cargo build --locked --features clustering` both pass.
* The SDK's `e2e` job: it builds Commy at a pinned commit, so it cannot see this change until it re-pins. The Commy lane runs the SDK's `tests/e2e.rs` against the new binary locally (`COMMY_BIN`) before the PR is marked ready, and says so.
* Known-bad control: with `COMMY_ALLOW_DEV_AUTH` unset, a binary non-range frame and a non-JSON text frame must each get a JSON `Error`, and neither may authenticate anyone. A test that sends the old MessagePack `Authenticate` bytes must get that refusal (this fails on `main`, which answers with an `AuthenticationResponse`).

## Decisions I need

* **A. Clustering.** `src/clustering.rs` uses `ClusterPing` and `TokenSync`. The standing decision is that clustering is not reworked while the FAM-to-Synapse merge is open. Options: (1) move those two variants unchanged into a private `ClusterMessage` enum inside `src/clustering.rs`, which keeps the feature compiling and is not a rework (recommended); (2) delete those two functions and their tests, which is a clustering change. Recommend (1).
* **B. Timing.** `ROADMAP.md` says R12 goes "after the wire freeze". `WssMessage` is not in `commy-wire`, so nothing here depends on the freeze; it can go now. The owner has said he is fine with the removal. Recommend now, as 0.21.0, unless the freeze release is about to be cut and you want one release note.
* **C. A deprecation release first?** The protocol is development-only and refused by default, no consumer uses it, so I recommend removing it directly.
* **D. `Logout`.** If you want a "log out without closing the socket" message in the JSON protocol, it is a new `commy-wire` message (a layout change, so it must precede the freeze). I recommend not adding it: nothing uses it.

## Risks

* The new refusal paths for non-range binary frames and non-JSON text are new code on the connection loop; the tests above cover them. A client that still sends MessagePack gets a JSON error and stays connected, as it does today.
* `MessageRouter`/`RoutingDecision` are public items; a downstream crate using them stops compiling. Nothing known does (the SDK does not; the other consumers use the binary).
