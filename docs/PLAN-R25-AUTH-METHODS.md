# R25: mTLS, Basic and custom credentials (design note)

Status: design only, 2026-11-19. Nothing here is built. Ant-first rule applies: no hand-rolled TLS or
password handling; each part below names the existing crate or ant that does the hard part.

## Where things stand

- `commy-wire` `Credentials` already carries `Basic { username, password }` and `Custom { data }`.
  The server answers both with `VerifyError::UnsupportedMethod` (`FrameworkVerifier`, `StaticApiKeys`).
- There is **no** wire credential for mTLS: the identity of a client certificate comes from the TLS
  handshake, not from a message. `TlsConfiguration` (`src/server/tls.rs`) builds a server-only
  `rustls::ServerConfig` with no client-certificate verifier.
- `CredentialVerifier::verify(VerifyRequest)` has no way to see the connection's peer certificate.

## 1. Basic (username and password)

Per-tenant `auth-framework` already owns users and passwords (`register_user(name, email, password)`).
Add a `Credentials::Basic` arm to `FrameworkVerifier::verify` that asks the framework to check the
password and takes the user id as the subject, exactly as the API-key arm does. No password code in
Commy. Open item to verify in the framework API before building: a password check that returns the
user without issuing a session (we only need an identity), with its own lockout / rate limiting so
guessing is not unlimited. If the framework cannot do that, this is a `[FINDING]` to auth-framework,
not a reason to write it here. Scopes: from the user's roles through `scopes_for_roles`, as for keys.

Tests (red first): right password; wrong password; unknown user; user of another tenant; empty
fields refused by `Authenticate::validate`; secrets absent from `Debug` and from audit lines.

## 2. mTLS

Three pieces.

1. **Handshake.** `rustls::server::WebPkiClientVerifier` (already in our rustls) with a trust store
   of CA files, `client_auth_ca` in the config. Modes: `off` (today), `optional` (a certificate, when
   sent, is verified; a client may still use another method), `required` (no certificate, no
   connection). Nothing hand-rolled; the file is read like the server certificate.
2. **Identity.** After the handshake, read the verified peer certificate and map it to a tenant and a
   client id. Proposal: the subject is the certificate's **SAN URI** `commy://<tenant>/<client_id>`;
   any other certificate is refused. The tenant in the URI must equal the `Authenticate.tenant_id`.
   No mapping table to keep, and a tenant cannot be claimed by someone else's certificate. Parsing
   with `x509-parser` (or whatever `auth-framework` already uses), not by hand.
3. **Wire.** A client still sends `Authenticate`. It needs a credential that says "use my
   certificate": a new `Credentials::Mtls` (no fields), tag `mtls`. **That is a `commy-wire` change
   and must land before the R1 freeze**, in the same release train as R30 (0.5.2). The server then
   passes the peer identity into `VerifyRequest` (new optional field `peer_cert`), and a
   `Credentials::Mtls` with no verified certificate on the connection is `Rejected`.

Revocation: out of scope for the first cut except short-lived certificates and a CRL file given to the
same `WebPkiClientVerifier` (it supports CRLs). Stated plainly in `AUTHENTICATION.md`.

Tests (red first): valid certificate for the tenant; certificate for another tenant; certificate from
an unknown CA; expired certificate; no certificate under `required`; `Credentials::Mtls` without a
certificate; the certificate is **not** accepted when it was only presented for another method under
`optional` unless the client asked for `mtls`.

## 3. Custom

Today `Custom { data }` is meant for "a verifier the server understands". The earlier R26 note
rejected loading verifier code by name (a security surface). Recommendation: **keep Custom refused
in the shipped binary**; embedders who link Commy as a library can already implement
`CredentialVerifier` and install it. Document that, and stop listing Custom as "later work".

## 4. Configuration (R26's second half)

`auth_methods = ["api_key", "jwt", "basic", "mtls"]` in the config file; unknown names fail like
unknown keys; the default is today's `["api_key", "jwt"]`. The development switch stays forbidden in
a file. Restart-only (not a live key), because it changes what the listener accepts.

## Wire and version effects

| Change | Repo | When |
|---|---|---|
| `Credentials::Mtls`, spec + schema + vector | commy-wire | before the R1 freeze; minor |
| `client_auth_ca`, `auth_methods`, `peer_cert` in `VerifyRequest`, Basic arm, mTLS mapping | commy | one PR |
| `[NOTICE]` to each SDK for `Credentials::Mtls` | lane | when the wire minor is cut |

## Questions for the PM / CireSnave

1. Is the SAN-URI identity scheme (`commy://<tenant>/<client_id>`) acceptable, or does CireSnave want
   a mapping table (certificate fingerprint to client id)? **Recommend the URI.**
2. Confirm `Custom` stays refused in the binary. **Recommend yes.**
3. Confirm the wire minor for `Credentials::Mtls` can ride with R30 (0.5.2) or the next one before the
   freeze.

— lane: commy
