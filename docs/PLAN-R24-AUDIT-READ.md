# R24: a wire message to read the audit log

**Status: option A accepted by the PM (2026-10-28); implementation waits on the `commy-wire` PR.** CireSnave ruled R24 GO on 2026-10-28 (read-only, admin-only, before the `commy-wire` freeze). Ant-first applies: the audit log stays commy's own durable log on PersistAnt (R6, R13); this adds a reader, not a new audit tool.

## What exists

`Server::audit_history(after, limit)` returns `(seq, AuditEvent{ts_ms, line, prev, hash})`, oldest first. `Server::audit_head()` gives the newest `(seq, hash)`; `persist::audit_hash(seq, ts_ms, prev, line)` is a pure function, so a client can re-check the chain itself. Operators today read files or call the Rust API; no wire message exists.

## The one real question: who is "admin"?

The log is **server-wide and free text**. Its lines mix tenants (`grant GRANTED ... by=<client>`) with server events (`state:`, `config:`). A tenant admin (`TenantAdmin`) is admin of one tenant only, so handing them the whole log would leak other tenants' client ids and service ids. Commy has no server-operator role today.

| Option | What | Cost |
|---|---|---|
| **A (recommended)** | Tag each event with the tenant it concerns (`AuditEvent.tenant: Option<String>`, `#[serde(default)]`, SCHEMA stays readable); a `TenantAdmin` reads only events tagged with **their own** tenant. Untagged (server-wide) events are not served over the wire. | The ~6 `audit(...)` call sites that concern a tenant pass the tenant; the hash input is unchanged for old events. Chain verification over a tenant-filtered view is not possible (gaps), see "Verification". |
| B | A server-operator session: a session authenticated into one configured operator tenant reads everything. | New config key in `src/config.rs` and a new role concept; more hand-rolled config and authorisation. Ant-first says avoid. |
| C | Dev-auth admin only. | Dev auth is refused on a reachable server; useless in production. |

Decision: **A** (PM, 2026-10-28). There is no server-wide read over the wire. **Events written before tagging carry no tenant and return no rows to tenant admins: there is no backfill and no guessing a tenant from the line text.** Recommendation was: **A**. It needs no new role, leaks nothing across tenants, and "admin-only" holds (`TenantAdmin` of that tenant). Operators who want the whole log keep the Rust API and the files.

## Messages (all JSON text frames, `commy-wire`)

- `ReadAudit{request_id, after?: u64, limit?: u32}` client to server. `limit` defaults to 100 and is capped at 1000 (a larger value is clamped, not refused).
- `AuditEvents{request_id, entries: [{seq, ts_ms, line, prev, hash}], head?: {seq, hash}, more: bool}` server to client. `entries` oldest first, only events tagged with the caller's tenant and with `seq > after`. `head` is the newest chained event of the whole log (`seq`, `hash`) so a client can anchor it elsewhere (R13); it carries no tenant data. `more` is true if the limit cut the list.

Errors (existing codes only, no new `ErrorCode`): `UNAUTHENTICATED` before `Authenticate`; `PERMISSION_DENIED` without `TenantAdmin`; `INVALID_REQUEST` for unknown fields; `RATE_LIMITED` past a per-session ceiling (reuse the existing session rate-limit mechanism). Without persistence the reply is an empty `entries` and no `head`.

## Verification

A tenant-filtered view has gaps, so the client **cannot** re-run the whole chain; each entry's `hash` is still recomputable from `(seq, ts_ms, prev, line)`, which proves an entry was not edited, and `head` plus an off-machine anchor (R13) proves the log was not truncated. Full-chain verification stays the operator's (`Server::verify_audit`). The spec says so plainly.

## What each repo changes

1. **commy-wire** (next minor, 0.6.0; the layout is still unfrozen): `ReadAudit`, `AuditEvents`, `AuditEntry`, `AuditHead` in `src/lib.rs`; `spec/audit.md`; `schema/control.schema.json`; golden vectors (a request, a reply with `more`, an empty reply). The enum is `#[non_exhaustive]`, so this is additive in Rust; an old server answers `INVALID_REQUEST`. Publish before commy depends on it (crates.io only, portfolio rule).
2. **commy** (minor, the PM allocates the number): the tenant tag on tenant-scoped audit calls, the `ws_handler` arm, tests (admin sees own tenant only; **tenant A's admin cannot read tenant B's events, a test with two tenants and one event each**; an untagged pre-tagging event is returned to nobody; non-admin refused; other tenant's events absent; `more`/`after` paging; clamp; no persistence; no secret in any returned line), README and `docs/AUTHENTICATION.md` lines.
3. **SDKs**: a follow-up row each, none required to ship the server side.

## Order and gates

1. This note, ack from the PM. 2. `commy-wire` PR and publish (owner's call, before the R1 freeze). 3. commy PR on the published crate. R1's freeze and R28 follow after.

## Not in this row

Writing, deleting or truncating the log over the wire; streaming/subscribing to new events; a server-operator role (option B), reopen only if someone needs the whole log remotely.
