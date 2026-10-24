# Commy: notes for AI assistants and contributors

Read `README.md` (what the server is and does), `ROADMAP.md` (what is open) and `CHANGELOG.md` (what
changed and why). The wire contract is the `commy-wire` crate (`spec/`, JSON Schema, golden vectors);
change the contract there first, never here alone.

## The design in five lines

- A service is a memory-mapped file of **bare bytes**, zero filled, requested size, rounded up to 4 KiB
  pages. Commy writes nothing into it: no header, no allocator, no names.
- Programs on the server's machine map the file; others keep a mirror and exchange changed byte
  ranges over WSS. The server's order wins: one sequence number per flush, per `file_id`.
- Files rotate (resize: drain barrier, the requester fills a prepared file, commit or abort; revocation).
- Access is per client, tenant and service (`server::grants`, on role-system); tenant-wide is shorthand.
- Authentication is API key or JWT through `auth-framework` (`docs/AUTHENTICATION.md`); `COMMY_ALLOW_DEV_AUTH=1` is the development switch.

## Rules for changes

- Stable Rust. No `#![feature]`.
- Every pushed change bumps the version (`Cargo.toml`, `Cargo.lock`) and adds a `CHANGELOG.md` entry; a
  breaking change before 1.0 bumps the second number. The portfolio PM allocates the number.
- Dependencies on other projects are crates.io versions, never paths or git, and are kept current.
- New tests come with a mutation check: break the behaviour and show a named test failing.
- New code paths that check access go through `session_allows` (service scope) in
  `src/server/ws_handler.rs`; the guard there refuses any message naming a service the client cannot reach.
- Do not put secrets, keys or logs in the repository (`*.pem`, `*.pfx`, `*.log` are ignored).

## Layout

`src/server/ws_handler.rs` handlers and the connection loop · `forwarding.rs` who is on which file ·
`rotation.rs` rotation state · `grants.rs` the grant store · `local_mapping.rs` the nonce handshake ·
`src/private_dir.rs` tenant directory permissions · `tests/wss_loopback.rs` real TLS + WebSocket clients.
