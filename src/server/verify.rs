//! Credential verification (plan R4): who is this client?
//!
//! A [`CredentialVerifier`] turns the credentials in an `Authenticate` request into an identity and
//! the permissions that identity holds. **The identity comes from the credential** (its subject),
//! never from what the client claims about itself; only [`DevVerifier`], which exists for development
//! and tests, takes the client's word, and it is used only when `COMMY_ALLOW_DEV_AUTH=1`.

use crate::auth::{Permission, PermissionSet};
use commy_wire::Credentials;
use std::future::Future;
use std::pin::Pin;

/// What a verifier is asked.
pub struct VerifyRequest<'a> {
    pub tenant_id: &'a str,
    pub credentials: &'a Credentials,
    /// The client's own suggestion for its id. Only [`DevVerifier`] looks at it.
    pub client_id_hint: Option<&'a str>,
    /// Development scopes the client asked for. Only [`DevVerifier`] looks at them.
    pub dev_scopes: Option<&'a [String]>,
    /// The connection's session id (the fallback identity of a development client).
    pub session_id: &'a str,
}

/// An authenticated identity.
#[derive(Debug, Clone)]
pub struct Verified {
    /// The identity grants are filed under: the credential's subject.
    pub client_id: String,
    /// What the identity may do at tenant level (imported as tenant-wide grants).
    pub permissions: PermissionSet,
    /// The scopes, for the reply (informational).
    pub scopes: Vec<String>,
    pub expires_in_seconds: Option<u64>,
}

/// Why verification did not produce an identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The credentials are wrong, unknown, expired or not for this tenant. Deliberately says no more.
    Rejected,
    /// This verifier does not handle this method.
    UnsupportedMethod,
    /// The verifier could not decide (its store is down). Not the client's fault.
    Backend(String),
}

/// A boxed future, so verifiers can be used as trait objects.
pub type VerifyFuture<'a> = Pin<Box<dyn Future<Output = Result<Verified, VerifyError>> + Send + 'a>>;

/// Turns credentials into an identity.
pub trait CredentialVerifier: Send + Sync {
    fn verify<'a>(&'a self, request: VerifyRequest<'a>) -> VerifyFuture<'a>;
}

/// Development and test verifier: accepts any credential. The client names itself (or gets its
/// session id) and gets administrator permissions unless it asks for exactly some scopes.
pub struct DevVerifier;

impl CredentialVerifier for DevVerifier {
    fn verify<'a>(&'a self, request: VerifyRequest<'a>) -> VerifyFuture<'a> {
        Box::pin(async move {
            let client_id = request
                .client_id_hint
                .map(str::to_string)
                .unwrap_or_else(|| request.session_id.to_string());
            let (permissions, scopes) = match request.dev_scopes {
                Some(scopes) => (PermissionSet::from_scopes(scopes), scopes.to_vec()),
                None => {
                    let mut p = PermissionSet::admin();
                    p.grant(Permission::ServiceCreate);
                    p.grant(Permission::ServiceRead);
                    p.grant(Permission::ServiceDelete);
                    p.grant(Permission::ServiceWrite);
                    (p, vec!["all".to_string()])
                }
            };
            Ok(Verified {
                client_id,
                permissions,
                scopes,
                expires_in_seconds: None,
            })
        })
    }
}

/// A verifier backed by a fixed list of API keys. For tests and for small fixed deployments that
/// want no framework; the framework-backed verifier is the production one.
pub struct StaticApiKeys {
    /// (key, tenant, subject, scopes)
    entries: Vec<(String, String, String, Vec<String>)>,
}

impl StaticApiKeys {
    pub fn new() -> Self {
        StaticApiKeys { entries: Vec::new() }
    }

    /// Accept `key` for `tenant`, as `subject`, holding `scopes`.
    pub fn with_key(mut self, key: &str, tenant: &str, subject: &str, scopes: &[&str]) -> Self {
        self.entries.push((
            key.to_string(),
            tenant.to_string(),
            subject.to_string(),
            scopes.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }
}

impl Default for StaticApiKeys {
    fn default() -> Self {
        Self::new()
    }
}

/// Compare two secrets without stopping at the first differing byte.
fn same_secret(a: &str, b: &str) -> bool {
    crate::server::local_mapping::secrets_match(a, b)
}

impl CredentialVerifier for StaticApiKeys {
    fn verify<'a>(&'a self, request: VerifyRequest<'a>) -> VerifyFuture<'a> {
        Box::pin(async move {
            let Credentials::ApiKey { key } = request.credentials else {
                return Err(VerifyError::UnsupportedMethod);
            };
            // Look at every entry so the time taken does not depend on which one matched.
            let mut found: Option<&(String, String, String, Vec<String>)> = None;
            for e in &self.entries {
                if same_secret(&e.0, key) && e.1 == request.tenant_id && found.is_none() {
                    found = Some(e);
                }
            }
            match found {
                Some((_, _, subject, scopes)) => Ok(Verified {
                    client_id: subject.clone(),
                    permissions: PermissionSet::from_scopes(scopes),
                    scopes: scopes.clone(),
                    expires_in_seconds: None,
                }),
                None => Err(VerifyError::Rejected),
            }
        })
    }
}
