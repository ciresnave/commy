//! The production credential verifier (plan R4): API keys and JWTs checked by `auth-framework`.
//!
//! One framework instance **per tenant**, each with its own SQLite database and its own JWT secret,
//! so a key or token issued for one tenant means nothing to another and there is no shared
//! multi-tenant state to get wrong. The identity of an authenticated client is the credential's
//! subject (the framework's user id, or the JWT `sub`), never anything the client says.
//!
//! **Provisioning is not Commy's job.** Create the tenant's database and its users and keys with
//! `auth-framework`: see `docs/AUTHENTICATION.md`. A login attempt never creates anything: a tenant
//! whose database file does not exist is simply refused.

use crate::auth::PermissionSet;
use crate::server::verify::{CredentialVerifier, Verified, VerifyError, VerifyFuture, VerifyRequest};
use auth_framework::config::AuthConfig;
use auth_framework::storage::sqlite::SqliteStorage;
use auth_framework::AuthFramework;
use commy_wire::Credentials;
use hkdf::Hkdf;
use sha2::Sha256;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// The environment variable holding the master secret per-tenant secrets are derived from.
pub const MASTER_SECRET_ENV: &str = "COMMY_AUTH_MASTER_SECRET";

/// The shortest master secret accepted, in bytes.
pub const MIN_MASTER_SECRET_LEN: usize = 32;

/// Why a verifier could not be created.
#[derive(Debug, PartialEq, Eq)]
pub enum SetupError {
    /// `COMMY_AUTH_MASTER_SECRET` is not set. There is no default: a guessable default secret would
    /// make every token forgeable.
    MasterSecretMissing,
    /// The master secret is shorter than [`MIN_MASTER_SECRET_LEN`] bytes.
    MasterSecretTooShort,
    Io(String),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::MasterSecretMissing => write!(
                f,
                "{MASTER_SECRET_ENV} is not set: the credential verifier will not start without a master secret (there is no default)"
            ),
            SetupError::MasterSecretTooShort => {
                write!(f, "{MASTER_SECRET_ENV} must be at least {MIN_MASTER_SECRET_LEN} bytes")
            }
            SetupError::Io(e) => write!(f, "cannot prepare the authentication directory: {e}"),
        }
    }
}

/// Derive a tenant's JWT secret: HKDF-SHA256 with the master secret as input keying material, a
/// fixed salt naming this use, and the tenant id as `info`. Deterministic, so tokens survive a
/// restart; different per tenant, so one tenant's tokens never verify for another.
pub fn derive_tenant_secret(master: &str, tenant_id: &str) -> String {
    let hk = Hkdf::<Sha256>::new(Some(b"commy-auth-framework-v1"), master.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(tenant_id.as_bytes(), &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm.iter().map(|b| format!("{b:02x}")).collect()
}

/// Tenant-level scopes for the role a framework API key carries. An API key can only carry
/// `admin`, `user` or `api_user`. Anything finer than this is a Commy grant on the key's subject.
pub fn scopes_for_roles(roles: &[String]) -> Vec<String> {
    let mut scopes: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        if !scopes.iter().any(|x| x == s) {
            scopes.push(s.to_string());
        }
    };
    for r in roles {
        match r.as_str() {
            "admin" => {
                add("admin");
                add("service:create");
            }
            "user" => add("service:create"),
            _ => {}
        }
    }
    scopes
}

/// Verifies API keys and JWTs with one `auth-framework` per tenant.
pub struct FrameworkVerifier {
    dir: PathBuf,
    master: String,
    frameworks: Mutex<HashMap<String, Arc<AuthFramework>>>,
}

impl FrameworkVerifier {
    /// A verifier whose tenant databases live in `dir` (`<tenant>.sqlite`), created private.
    pub fn new(dir: &Path, master_secret: &str) -> Result<Self, SetupError> {
        if master_secret.len() < MIN_MASTER_SECRET_LEN {
            return Err(SetupError::MasterSecretTooShort);
        }
        crate::private_dir::create_secret(dir).map_err(|e| SetupError::Io(e.to_string()))?;
        Ok(FrameworkVerifier {
            dir: dir.to_path_buf(),
            master: master_secret.to_string(),
            frameworks: Mutex::new(HashMap::new()),
        })
    }

    /// As [`FrameworkVerifier::new`], reading the master secret from `COMMY_AUTH_MASTER_SECRET`.
    /// Fails if it is not set.
    pub fn from_env(dir: &Path) -> Result<Self, SetupError> {
        match std::env::var(MASTER_SECRET_ENV) {
            Ok(s) if !s.is_empty() => Self::new(dir, &s),
            _ => Err(SetupError::MasterSecretMissing),
        }
    }

    /// The database file of a tenant.
    pub fn database_path(&self, tenant_id: &str) -> PathBuf {
        self.dir.join(format!("{tenant_id}.sqlite"))
    }

    /// The JWT secret of a tenant (what its framework must be configured with).
    pub fn tenant_secret(&self, tenant_id: &str) -> String {
        derive_tenant_secret(&self.master, tenant_id)
    }

    /// Use `framework` for `tenant_id` (tests give it in-memory storage).
    pub async fn with_framework(self, tenant_id: &str, framework: Arc<AuthFramework>) -> Self {
        self.frameworks
            .lock()
            .await
            .insert(tenant_id.to_string(), framework);
        self
    }

    /// The tenant's framework: the one already open, or the one in its database file if that file
    /// exists. **Never creates a file**: an unknown tenant has no framework.
    async fn framework(&self, tenant_id: &str) -> Option<Arc<AuthFramework>> {
        let mut map = self.frameworks.lock().await;
        if let Some(f) = map.get(tenant_id) {
            return Some(Arc::clone(f));
        }
        if crate::tenant_id::TenantId::parse(tenant_id).is_err() {
            return None;
        }
        let path = self.database_path(tenant_id);
        if !path.is_file() {
            return None;
        }
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(false);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .ok()?;
        let storage = SqliteStorage::new(pool);
        storage.migrate().await.ok()?;
        let mut framework = AuthFramework::new_with_storage(
            AuthConfig::new().secret(self.tenant_secret(tenant_id)),
            Arc::new(storage),
        );
        framework.initialize().await.ok()?;
        let framework = Arc::new(framework);
        map.insert(tenant_id.to_string(), Arc::clone(&framework));
        Some(framework)
    }
}

fn valid_subject(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= commy_wire::MAX_CLIENT_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | ':'))
}

impl CredentialVerifier for FrameworkVerifier {
    fn verify<'a>(&'a self, request: VerifyRequest<'a>) -> VerifyFuture<'a> {
        Box::pin(async move {
            let Some(framework) = self.framework(request.tenant_id).await else {
                // No such tenant database: say "no" like any other failure.
                return Err(VerifyError::Rejected);
            };
            let (subject, scopes) = match request.credentials {
                Credentials::ApiKey { key } => {
                    let user = framework
                        .validate_api_key(key)
                        .await
                        .map_err(|_| VerifyError::Rejected)?;
                    let roles: Vec<String> = user.roles.iter().map(|r| r.to_string()).collect();
                    (user.id, scopes_for_roles(&roles))
                }
                Credentials::Jwt { token } => {
                    let claims = framework
                        .validate_access_jwt(token)
                        .await
                        .map_err(|_| VerifyError::Rejected)?;
                    let scopes = claims
                        .scope
                        .split_whitespace()
                        .map(str::to_string)
                        .collect();
                    (claims.sub, scopes)
                }
                _ => return Err(VerifyError::UnsupportedMethod),
            };
            if !valid_subject(&subject) {
                return Err(VerifyError::Backend(
                    "the credential's subject is not a valid client id".to_string(),
                ));
            }
            Ok(Verified {
                client_id: subject,
                permissions: PermissionSet::from_scopes(&scopes),
                scopes,
                expires_in_seconds: None,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const MASTER: &str = "0123456789abcdef0123456789abcdef-master";

    /// A framework for `tenant` configured exactly as the verifier would open it, in memory.
    async fn framework_for(tenant: &str) -> Arc<AuthFramework> {
        let mut f = AuthFramework::new(AuthConfig::new().secret(derive_tenant_secret(MASTER, tenant)));
        f.initialize().await.unwrap();
        Arc::new(f)
    }

    /// A verifier over in-memory frameworks for the given tenants.
    async fn verifier(tenants: &[&str]) -> (FrameworkVerifier, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut v = FrameworkVerifier::new(dir.path(), MASTER).unwrap();
        for t in tenants {
            v = v.with_framework(t, framework_for(t).await).await;
        }
        (v, dir)
    }

    async fn check(v: &FrameworkVerifier, tenant: &str, creds: Credentials) -> Result<Verified, VerifyError> {
        v.verify(VerifyRequest {
            tenant_id: tenant,
            credentials: &creds,
            client_id_hint: Some("root"),
            dev_scopes: Some(&["admin".to_string()]),
            session_id: "s",
        })
        .await
    }

    fn key(k: &str) -> Credentials {
        Credentials::ApiKey { key: k.to_string() }
    }

    fn jwt(t: &str) -> Credentials {
        Credentials::Jwt { token: t.to_string() }
    }

    async fn user_with_key(f: &AuthFramework, name: &str, role: &str, ttl: Option<Duration>) -> (String, String) {
        let id = f
            .register_user(name, &format!("{name}@example.com"), "S3cur3-P@ssw0rd!x")
            .await
            .unwrap();
        let k = f.create_api_key_with_role(&id, role, ttl).await.unwrap();
        (id, k)
    }

    #[test]
    fn tenant_secrets_are_hkdf_outputs_that_differ_per_tenant_and_per_master() {
        let a = derive_tenant_secret(MASTER, "acme");
        assert_eq!(a, derive_tenant_secret(MASTER, "acme"), "deterministic, so tokens survive a restart");
        assert_ne!(a, derive_tenant_secret(MASTER, "other"));
        assert_ne!(a, derive_tenant_secret("another-master-secret-of-enough-length!!", "acme"));
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!a.contains("acme") && !a.contains(MASTER), "not a concatenation");
    }

    #[test]
    fn the_verifier_refuses_to_start_without_a_long_enough_master_secret() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(FrameworkVerifier::new(dir.path(), "short").err(), Some(SetupError::MasterSecretTooShort));
        assert_eq!(FrameworkVerifier::new(dir.path(), "").err(), Some(SetupError::MasterSecretTooShort));
        std::env::remove_var(MASTER_SECRET_ENV);
        assert_eq!(
            FrameworkVerifier::from_env(dir.path()).err(),
            Some(SetupError::MasterSecretMissing),
            "there is no default"
        );
    }

    #[test]
    fn roles_map_to_tenant_level_scopes_and_nothing_else() {
        let s = |r: &[&str]| scopes_for_roles(&r.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&["admin"]), ["admin", "service:create"]);
        assert_eq!(s(&["user"]), ["service:create"]);
        assert!(s(&["api_user"]).is_empty());
        assert!(s(&["root", "superuser", "service:admin", ""]).is_empty(), "unknown roles grant nothing");
        assert_eq!(s(&["admin", "user"]), ["admin", "service:create"], "no duplicates");
    }

    #[tokio::test]
    async fn a_valid_api_key_is_the_users_identity_with_its_roles_scopes() {
        let (v, _d) = verifier(&["acme"]).await;
        let f = v.framework("acme").await.unwrap();
        let (id, k) = user_with_key(&f, "alice", "user", None).await;
        let got = check(&v, "acme", key(&k)).await.unwrap();
        assert_eq!(got.client_id, id, "identity = the subject, not the hint 'root'");
        assert_eq!(got.scopes, ["service:create"]);
        let (aid, ak) = user_with_key(&f, "boss", "admin", None).await;
        let got = check(&v, "acme", key(&ak)).await.unwrap();
        assert_eq!(got.client_id, aid);
        assert_eq!(got.scopes, ["admin", "service:create"]);
    }

    #[tokio::test]
    async fn a_wrong_or_expired_api_key_is_rejected() {
        let (v, _d) = verifier(&["acme"]).await;
        let f = v.framework("acme").await.unwrap();
        let (_id, k) = user_with_key(&f, "alice", "user", None).await;
        assert_eq!(check(&v, "acme", key("ak_not-a-real-key")).await.err(), Some(VerifyError::Rejected));
        assert_eq!(check(&v, "acme", key("")).await.err(), Some(VerifyError::Rejected));
        let mut flipped = k.clone();
        flipped.pop();
        flipped.push('x');
        assert_eq!(check(&v, "acme", key(&flipped)).await.err(), Some(VerifyError::Rejected));
        let (_id2, short) = user_with_key(&f, "shortlived", "user", Some(Duration::from_secs(1))).await;
        assert!(check(&v, "acme", key(&short)).await.is_ok(), "valid while fresh");
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert_eq!(check(&v, "acme", key(&short)).await.err(), Some(VerifyError::Rejected), "expired");
    }

    #[tokio::test]
    async fn an_api_key_for_tenant_a_does_not_open_tenant_b() {
        let (v, _d) = verifier(&["acme", "globex"]).await;
        let f = v.framework("acme").await.unwrap();
        let (_id, k) = user_with_key(&f, "alice", "user", None).await;
        assert!(check(&v, "acme", key(&k)).await.is_ok());
        assert_eq!(check(&v, "globex", key(&k)).await.err(), Some(VerifyError::Rejected));
    }

    #[tokio::test]
    async fn a_valid_jwt_gives_its_subject_and_scopes() {
        let (v, _d) = verifier(&["acme"]).await;
        let f = v.framework("acme").await.unwrap();
        let (id, _k) = user_with_key(&f, "dave", "user", None).await;
        let t = f
            .token_manager()
            .create_jwt_token(id.clone(), vec!["read".into(), "service:create".into()], Some(Duration::from_secs(60)))
            .unwrap();
        let got = check(&v, "acme", jwt(&t)).await.unwrap();
        assert_eq!(got.client_id, id);
        assert_eq!(got.scopes, ["read", "service:create"]);
    }

    #[tokio::test]
    async fn a_jwt_for_tenant_a_is_rejected_for_tenant_b() {
        let (v, _d) = verifier(&["acme", "globex"]).await;
        let fa = v.framework("acme").await.unwrap();
        let (id, _k) = user_with_key(&fa, "erin", "user", None).await;
        let t = fa
            .token_manager()
            .create_jwt_token(id, vec!["admin".into()], Some(Duration::from_secs(60)))
            .unwrap();
        assert!(check(&v, "acme", jwt(&t)).await.is_ok());
        assert_eq!(
            check(&v, "globex", jwt(&t)).await.err(),
            Some(VerifyError::Rejected),
            "different tenant, different secret"
        );
    }

    /// The isolation must come from the tenant's secret, not merely from the user being absent: here
    /// tenant B's database is a copy of A's, so the same user exists in both and only the derived
    /// secret differs.
    #[tokio::test]
    async fn a_jwt_is_rejected_for_a_tenant_with_the_same_user_but_a_different_secret() {
        let dir = tempfile::tempdir().unwrap();
        let v = FrameworkVerifier::new(dir.path(), MASTER).unwrap();
        let path = v.database_path("acme");
        let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&path).create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
        let storage = SqliteStorage::new(pool.clone());
        storage.migrate().await.unwrap();
        let mut f = AuthFramework::new_with_storage(AuthConfig::new().secret(v.tenant_secret("acme")), Arc::new(storage));
        f.initialize().await.unwrap();
        let (id, _k) = user_with_key(&f, "frank", "user", None).await;
        let t = f
            .token_manager()
            .create_jwt_token(id, vec!["admin".into()], Some(Duration::from_secs(60)))
            .unwrap();
        drop(f);
        pool.close().await;
        std::fs::copy(&path, v.database_path("globex")).unwrap();

        let fresh = FrameworkVerifier::new(dir.path(), MASTER).unwrap();
        assert!(check(&fresh, "acme", jwt(&t)).await.is_ok(), "control: valid for its own tenant");
        assert_eq!(check(&fresh, "globex", jwt(&t)).await.err(), Some(VerifyError::Rejected));
    }

    #[tokio::test]
    async fn an_expired_tampered_or_foreign_jwt_is_rejected() {
        let (v, _d) = verifier(&["acme"]).await;
        let f = v.framework("acme").await.unwrap();
        let expired = f
            .token_manager()
            .create_jwt_token("user-42", vec![], Some(Duration::from_secs(1)))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert_eq!(check(&v, "acme", jwt(&expired)).await.err(), Some(VerifyError::Rejected), "expired");

        let good = f
            .token_manager()
            .create_jwt_token("user-42", vec!["admin".into()], Some(Duration::from_secs(60)))
            .unwrap();
        let mut tampered = good.clone();
        tampered.push('A');
        assert_eq!(check(&v, "acme", jwt(&tampered)).await.err(), Some(VerifyError::Rejected), "tampered signature");
        assert_eq!(check(&v, "acme", jwt("not.a.jwt")).await.err(), Some(VerifyError::Rejected));
        assert_eq!(check(&v, "acme", jwt("")).await.err(), Some(VerifyError::Rejected));

        // signed with some other secret
        let mut other = AuthFramework::new(AuthConfig::new().secret("a-completely-different-secret-of-length-32+"));
        other.initialize().await.unwrap();
        let forged = other
            .token_manager()
            .create_jwt_token("user-42", vec!["admin".into()], Some(Duration::from_secs(60)))
            .unwrap();
        assert_eq!(check(&v, "acme", jwt(&forged)).await.err(), Some(VerifyError::Rejected), "foreign secret");
    }

    #[tokio::test]
    async fn methods_other_than_api_key_and_jwt_are_unsupported() {
        let (v, _d) = verifier(&["acme"]).await;
        let basic = Credentials::Basic { username: "a".into(), password: "b".into() };
        assert_eq!(check(&v, "acme", basic).await.err(), Some(VerifyError::UnsupportedMethod));
        let custom = Credentials::Custom { data: serde_json::json!({}) };
        assert_eq!(check(&v, "acme", custom).await.err(), Some(VerifyError::UnsupportedMethod));
    }

    #[tokio::test]
    async fn an_unknown_tenant_is_rejected_and_never_creates_a_database_file() {
        let (v, dir) = verifier(&[]).await;
        for tenant in ["nobody_here", "x", "../escape", "a/b", ""] {
            assert_eq!(check(&v, tenant, key("ak_whatever")).await.err(), Some(VerifyError::Rejected), "{tenant}");
            assert_eq!(check(&v, tenant, jwt("a.b.c")).await.err(), Some(VerifyError::Rejected), "{tenant}");
        }
        let created: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert!(created.is_empty(), "a login attempt created {created:?}");
        assert!(!dir.path().parent().unwrap().join("escape.sqlite").exists());
    }

    /// The whole path a deployment uses: a tenant database provisioned with `auth-framework`'s library
    /// calls (as `docs/AUTHENTICATION.md` shows), then found and used by the verifier.
    #[tokio::test]
    async fn a_provisioned_sqlite_database_is_found_and_its_keys_and_tokens_work() {
        let dir = tempfile::tempdir().unwrap();
        let v = FrameworkVerifier::new(dir.path(), MASTER).unwrap();
        // provisioning, done once by an operator
        let path = v.database_path("acme");
        let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&path).create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .unwrap();
        let storage = SqliteStorage::new(pool);
        storage.migrate().await.unwrap();
        let mut f = AuthFramework::new_with_storage(AuthConfig::new().secret(v.tenant_secret("acme")), Arc::new(storage));
        f.initialize().await.unwrap();
        let (id, k) = user_with_key(&f, "carol", "user", None).await;
        let t = f
            .token_manager()
            .create_jwt_token(id.clone(), vec!["read".into()], Some(Duration::from_secs(60)))
            .unwrap();
        drop(f);

        // a verifier that has never seen the tenant opens the file
        let fresh = FrameworkVerifier::new(dir.path(), MASTER).unwrap();
        assert_eq!(check(&fresh, "acme", key(&k)).await.unwrap().client_id, id);
        assert_eq!(check(&fresh, "acme", jwt(&t)).await.unwrap().scopes, ["read"]);
        // a verifier with a different master secret cannot validate the token
        let wrong_master =
            FrameworkVerifier::new(dir.path(), "a-different-master-secret-of-enough-length").unwrap();
        assert_eq!(check(&wrong_master, "acme", jwt(&t)).await.err(), Some(VerifyError::Rejected));
    }
}
