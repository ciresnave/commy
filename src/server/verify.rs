// SPDX-License-Identifier: MIT
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
    /// The identity the connection's verified TLS client certificate names (R25); `None` when the
    /// client presented no certificate or one that names no Commy identity.
    pub peer_identity: Option<&'a crate::server::mtls::PeerIdentity>,
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

/// Lets only the named credential methods through to the verifier it wraps (R25 `auth_methods`).
/// Any other method is [`VerifyError::UnsupportedMethod`] and the inner verifier is never asked.
pub struct MethodFilter<V> {
    inner: V,
    allowed: Vec<String>,
}

impl<V> MethodFilter<V> {
    pub fn new(inner: V, allowed: Vec<String>) -> Self {
        MethodFilter { inner, allowed }
    }
}

impl<V: CredentialVerifier> CredentialVerifier for MethodFilter<V> {
    fn verify<'a>(&'a self, request: VerifyRequest<'a>) -> VerifyFuture<'a> {
        if !self.allowed.iter().any(|m| m == request.credentials.method()) {
            return Box::pin(async { Err(VerifyError::UnsupportedMethod) });
        }
        self.inner.verify(request)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Accepts everything and counts how often it was asked.
    struct Counting(Arc<AtomicUsize>);

    impl CredentialVerifier for Counting {
        fn verify<'a>(&'a self, _r: VerifyRequest<'a>) -> VerifyFuture<'a> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Ok(Verified {
                    client_id: "c".into(),
                    permissions: PermissionSet::from_scopes(&[]),
                    scopes: vec![],
                    expires_in_seconds: None,
                })
            })
        }
    }

    async fn ask(v: &impl CredentialVerifier, c: Credentials) -> Result<Verified, VerifyError> {
        v.verify(VerifyRequest {
            tenant_id: "acme",
            credentials: &c,
            client_id_hint: None,
            dev_scopes: None,
            session_id: "s",
            peer_identity: None,
        })
        .await
    }

    #[tokio::test]
    async fn the_filter_passes_only_the_allowed_methods_and_never_asks_the_inner_verifier_for_others() {
        let asked = Arc::new(AtomicUsize::new(0));
        let f = MethodFilter::new(Counting(asked.clone()), vec!["api_key".into(), "jwt".into()]);
        assert!(ask(&f, Credentials::ApiKey { key: "k".into() }).await.is_ok());
        assert!(ask(&f, Credentials::Jwt { token: "t".into() }).await.is_ok());
        assert_eq!(asked.load(Ordering::SeqCst), 2);
        let basic = Credentials::Basic { username: "u".into(), password: "p".into() };
        assert_eq!(ask(&f, basic).await.unwrap_err(), VerifyError::UnsupportedMethod);
        let custom = Credentials::Custom { data: serde_json::json!({}) };
        assert_eq!(ask(&f, custom).await.unwrap_err(), VerifyError::UnsupportedMethod);
        assert_eq!(asked.load(Ordering::SeqCst), 2, "refused methods never reached the inner verifier");
    }
}
