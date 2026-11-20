# Authentication

A client proves who it is with an `Authenticate` message (the `commy-wire` type): a `tenant_id` and
credentials: an **API key** or a **JWT** (the default), a **user name and password** (`basic`) or the
**TLS client certificate** of the connection (`mtls`), the last two when `auth_methods` allows them.
`Authenticate` names exactly one method and carries only that method's fields; a request with a
second method's secret or any unknown field is refused at parse time. The server never believes a `client_id` the client
supplies; the identity is the credential's subject (the `auth-framework` user id, or the JWT `sub`).

## How the server verifies

`FrameworkVerifier` (`src/server/framework_verifier.rs`) runs one `auth-framework` instance **per
tenant**:

- Each tenant has its own SQLite database, `<COMMY_AUTH_DIR>/<tenant>.sqlite` (default
  `commy_state/auth/`). A login attempt **never creates** one: a tenant without a database is simply
  refused.
- Each tenant has its own JWT secret, derived from the master secret with HKDF-SHA256 (salt
  `commy-auth-framework-v1`, `info` = the tenant id). A token issued for tenant A fails signature
  checks for tenant B.
- `COMMY_AUTH_MASTER_SECRET` (at least 32 bytes) **must be set**; with development authentication off,
  the server exits at start-up without it. There is no default. Keep it out of logs and version control;
  changing it invalidates every outstanding JWT (API keys live in the database and are unaffected).

`COMMY_ALLOW_DEV_AUTH=1` replaces all of this with "accept anything" for development. Never set it in
production.

## What a credential grants

| Credential | Identity | Tenant-level scopes |
|---|---|---|
| API key, role `admin` | the key's user id | `admin`, `service:create` |
| API key, role `user` | the key's user id | `service:create` |
| API key, role `api_user` | the key's user id | none |
| JWT | `sub` | the whitespace-separated `scope` claim |

Everything finer (read, write or admin on one service) is a Commy **grant** on that identity
(`Grant` / `Revoke` / `ListGrants`), not a scope.

## Provisioning a tenant

Commy does not manage users. Create the tenant's database and its users and keys with the
`auth-framework` library (or its admin tooling), configured with the tenant's secret. The secret is
`commy::server::framework_verifier::derive_tenant_secret(master, tenant)`, or
`FrameworkVerifier::tenant_secret`:

```rust
let v = FrameworkVerifier::from_env(dir)?;               // reads COMMY_AUTH_MASTER_SECRET
let options = sqlx::sqlite::SqliteConnectOptions::new()
    .filename(v.database_path("acme"))
    .create_if_missing(true);
let pool = sqlx::sqlite::SqlitePoolOptions::new().connect_with(options).await?;
let storage = SqliteStorage::new(pool);
storage.migrate().await?;
let mut fw = AuthFramework::new_with_storage(
    AuthConfig::new().secret(v.tenant_secret("acme")),
    Arc::new(storage),
);
fw.initialize().await?;
let user = fw.register_user("alice", "alice@example.com", "a strong password").await?;
let api_key = fw.create_api_key_with_role(&user, "user", None).await?;   // roles: admin | user | api_user
```

The `a_provisioned_sqlite_database_is_found_and_its_keys_and_tokens_work` test is this flow end to end.

## Choosing the accepted methods

`auth_methods` (file key, or `COMMY_AUTH_METHODS`) is a comma-separated list of `api_key`, `jwt`,
`basic` and `mtls`; the default is `api_key,jwt`. A method not in the list is `UnsupportedMethod`
before any verifier is asked. An unknown name, an empty list, or `mtls` without `tls_client_auth` stops
the server at start-up. The list is read once; changing it needs a restart.

## Basic (user name and password)

`basic` goes through `auth-framework`'s own password login in the tenant's database (bcrypt, its rate
limit and minimum response time). The identity is the user's id and the scopes are the ones the
framework issues. A wrong password, an unknown or deactivated user, a user of another tenant, and a
login that needs MFA (Commy cannot run an MFA challenge) are all `Rejected`. Users are created with the
framework, as for API keys.

## mTLS (TLS client certificates)

Settings: `tls_client_ca_path` (PEM file of the CAs whose client certificates are trusted) and
`tls_client_auth` = `off` (default), `optional` or `required`. rustls's own `WebPkiClientVerifier`
checks the chain in the handshake: with `required` a client without a valid certificate cannot
connect; with `optional` a client without one can connect and use another method, but a certificate
that is presented and invalid still fails the handshake.

A certificate names a Commy identity with exactly one Subject Alternative Name URI
`commy://<tenant>/<client_id>` (tenant: `[A-Za-z0-9_-]`, 1 to 64; client id: `[A-Za-z0-9_.@:-]`, 1 to
128, one path segment). The subject, the CN and every other SAN are ignored; no such URI, two of them,
or a malformed one names nothing. The client then sends `Authenticate` with `method: "mtls"` and the
`tenant_id` it wants; the server accepts it only when the certificate names **that** tenant and the
tenant has a database. The identity is the `client_id`. **A certificate carries no tenant-level scope**
(no `service:create`, no `admin`): what the identity may do is its grants (`Grant` / `Revoke`).

Not done: revocation. Use short-lived certificates; a CRL file for the verifier is future work.

## Not covered

Custom credentials are refused (`UnsupportedMethod`); a verifier is installed by an embedder in code,
never loaded by name. The old MessagePack protocol, with its development-only `Authenticate`, was
removed in 0.21.0 (ROADMAP R12): `Authenticate` is the JSON message of `commy-wire`.
