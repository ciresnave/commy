// SPDX-License-Identifier: MIT
//! TLS Configuration and Certificate Loading
//!
//! Handles loading and validation of TLS certificates and keys for WSS support.

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use std::io;
use std::path::Path;
use std::sync::Arc;

/// TLS configuration errors
#[derive(Debug)]
pub enum TlsError {
    /// Certificate file not found or not readable
    CertificateNotFound(String),
    /// Private key file not found or not readable
    KeyNotFound(String),
    /// Invalid certificate format
    InvalidCertificate(String),
    /// Invalid private key format
    InvalidPrivateKey(String),
    /// Failed to parse PEM
    PemParseError(String),
    /// Other IO errors
    IoError(io::Error),
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsError::CertificateNotFound(path) => write!(f, "Certificate not found: {}", path),
            TlsError::KeyNotFound(path) => write!(f, "Private key not found: {}", path),
            TlsError::InvalidCertificate(msg) => write!(f, "Invalid certificate: {}", msg),
            TlsError::InvalidPrivateKey(msg) => write!(f, "Invalid private key: {}", msg),
            TlsError::PemParseError(msg) => write!(f, "PEM parse error: {}", msg),
            TlsError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<io::Error> for TlsError {
    fn from(err: io::Error) -> Self {
        TlsError::IoError(err)
    }
}

/// Map a failure to open or start reading a PEM file to a [`TlsError`]: an I/O failure stays an I/O
/// failure, anything else is a bad file of the kind `make` names.
fn pem_error(e: rustls::pki_types::pem::Error, what: &str, make: fn(String) -> TlsError) -> TlsError {
    match e {
        rustls::pki_types::pem::Error::Io(io) => TlsError::IoError(io),
        _ => make(format!("Failed to read PEM {what}")),
    }
}

/// Whether the server asks clients for a certificate (R25).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientAuth {
    /// A client may send a certificate; one without is accepted.
    Optional,
    /// A client without a valid certificate is refused at the handshake.
    Required,
}

/// Loads and validates TLS certificates and keys
pub struct TlsConfiguration {
    /// The loaded Rustls ServerConfig
    pub config: Arc<ServerConfig>,
}

impl TlsConfiguration {
    /// Create a new TLS configuration from certificate and key files
    ///
    /// # Arguments
    ///
    /// * `cert_path` - Path to the TLS certificate file (PEM format)
    /// * `key_path` - Path to the TLS private key file (PEM format)
    ///
    /// # Errors
    ///
    /// Returns `TlsError` if certificate or key cannot be loaded or parsed
    pub fn from_files<P: AsRef<Path>>(cert_path: P, key_path: P) -> Result<Self, TlsError> {
        Self::load(cert_path.as_ref(), key_path.as_ref(), None)
    }

    /// [`TlsConfiguration::from_files`] plus client certificates (mTLS, R25): a client certificate is
    /// verified against the CAs in `client_ca_path` (PEM) by rustls's own `WebPkiClientVerifier`.
    /// With [`ClientAuth::Required`] a connection without a valid certificate fails its handshake; with
    /// [`ClientAuth::Optional`] one without a certificate is accepted (and may use another credential
    /// method), but a certificate that is presented and invalid still fails the handshake.
    pub fn from_files_with_client_auth<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
        client_ca_path: P,
        mode: ClientAuth,
    ) -> Result<Self, TlsError> {
        Self::from_files_with_client_auth_and_crl(cert_path, key_path, client_ca_path, mode, None)
    }

    /// [`TlsConfiguration::from_files_with_client_auth`] plus certificate revocation (R33): `crl_path` is
    /// a PEM file of one or more CRLs, handed to the same `WebPkiClientVerifier`, so a client certificate
    /// listed in its issuer's CRL fails the handshake. With a CRL given, rustls refuses a certificate
    /// whose issuer has no CRL in the file (status unknown), so supply one CRL per trusted CA. A missing,
    /// empty or unparsable file stops start-up; revocation is never silently off. The file is read once,
    /// at start-up (a new CRL needs a restart). OCSP is not supported.
    pub fn from_files_with_client_auth_and_crl<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
        client_ca_path: P,
        mode: ClientAuth,
        crl_path: Option<&Path>,
    ) -> Result<Self, TlsError> {
        let ca_path = client_ca_path.as_ref();
        if !ca_path.exists() {
            return Err(TlsError::CertificateNotFound(ca_path.to_string_lossy().to_string()));
        }
        let mut roots = rustls::RootCertStore::empty();
        for cert in CertificateDer::pem_file_iter(ca_path)
            .map_err(|e| pem_error(e, "client CA certificates", TlsError::InvalidCertificate))?
        {
            let cert = cert.map_err(|_| {
                TlsError::InvalidCertificate("Failed to parse PEM client CA certificates".to_string())
            })?;
            roots
                .add(cert)
                .map_err(|e| TlsError::InvalidCertificate(format!("Bad client CA certificate: {e:?}")))?;
        }
        if roots.is_empty() {
            return Err(TlsError::InvalidCertificate("No client CA certificates found in file".to_string()));
        }
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut builder =
            rustls::server::WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider);
        if mode == ClientAuth::Optional {
            builder = builder.allow_unauthenticated();
        }
        if let Some(crl_path) = crl_path {
            if !crl_path.exists() {
                return Err(TlsError::CertificateNotFound(crl_path.to_string_lossy().to_string()));
            }
            let crls = rustls::pki_types::CertificateRevocationListDer::pem_file_iter(crl_path)
                .map_err(|e| pem_error(e, "client certificate revocation lists", TlsError::InvalidCertificate))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| TlsError::InvalidCertificate("Failed to parse PEM revocation lists".to_string()))?;
            if crls.is_empty() {
                return Err(TlsError::InvalidCertificate("No revocation lists found in file".to_string()));
            }
            builder = builder.with_crls(crls);
        }
        let verifier = builder
            .build()
            .map_err(|e| TlsError::InvalidCertificate(format!("Failed to build the client verifier: {e:?}")))?;
        Self::load(cert_path.as_ref(), key_path.as_ref(), Some(verifier))
    }

    fn load(
        cert_path: &Path,
        key_path: &Path,
        client_verifier: Option<Arc<dyn rustls::server::danger::ClientCertVerifier>>,
    ) -> Result<Self, TlsError> {
        // Verify files exist
        if !cert_path.exists() {
            return Err(TlsError::CertificateNotFound(
                cert_path.to_string_lossy().to_string(),
            ));
        }
        if !key_path.exists() {
            return Err(TlsError::KeyNotFound(
                key_path.to_string_lossy().to_string(),
            ));
        }

        // Load the certificate chain. (rustls-pki-types reads the PEM itself; rustls-pemfile is
        // unmaintained, RUSTSEC-2025-0134.)
        let certificates: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_path)
            .map_err(|e| pem_error(e, "certificates", TlsError::InvalidCertificate))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                TlsError::InvalidCertificate("Failed to parse PEM certificates".to_string())
            })?;

        if certificates.is_empty() {
            return Err(TlsError::InvalidCertificate(
                "No certificates found in file".to_string(),
            ));
        }

        // Load the private key: the first PKCS#8, PKCS#1 (RSA) or SEC1 (EC) key in the file.
        let private_key = PrivateKeyDer::from_pem_file(key_path).map_err(|e| match e {
            rustls::pki_types::pem::Error::NoItemsFound => {
                TlsError::InvalidPrivateKey("No private keys found in file".to_string())
            }
            rustls::pki_types::pem::Error::Io(io) => TlsError::IoError(io),
            _ => TlsError::InvalidPrivateKey("Failed to parse PEM private keys".to_string()),
        })?;

        // Create ServerConfig with an explicit crypto provider. The dependency tree enables both the
        // aws-lc-rs and ring backends, so rustls cannot pick a process-level default on its own and
        // `ServerConfig::builder()` would panic.
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let builder = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| {
                TlsError::InvalidCertificate(format!("Failed to select TLS versions: {:?}", e))
            })?;
        let builder = match client_verifier {
            Some(v) => builder.with_client_cert_verifier(v),
            None => builder.with_no_client_auth(),
        };
        let config = builder
            .with_single_cert(certificates, private_key)
            .map_err(|e| {
                TlsError::InvalidCertificate(format!("Failed to build config: {:?}", e))
            })?;

        Ok(TlsConfiguration {
            config: Arc::new(config),
        })
    }

    /// Create a self-signed certificate configuration for testing (if available)
    ///
    /// This is a convenience method for development/testing.
    /// Production should use proper certificates.
    #[allow(dead_code, reason = "public API of the library; the binary does not call it, the tests do")]
    pub fn for_development() -> Result<Self, TlsError> {
        // In development, you would generate or load test certificates
        // For now, this returns an error as no test certs are bundled
        Err(TlsError::PemParseError(
            "Development certificates not available. Use from_files() with proper certificates."
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_certificate_error() {
        let result = TlsConfiguration::from_files("/nonexistent/cert.pem", "/nonexistent/key.pem");
        assert!(result.is_err());
        match result {
            Err(TlsError::CertificateNotFound(path)) => {
                assert!(path.contains("cert.pem"));
            }
            _ => panic!("Expected CertificateNotFound error"),
        }
    }

    #[test]
    fn test_tls_error_display() {
        let err = TlsError::CertificateNotFound("test.pem".to_string());
        assert!(err.to_string().contains("Certificate not found"));

        let err = TlsError::InvalidCertificate("bad format".to_string());
        assert!(err.to_string().contains("Invalid certificate"));

        let err = TlsError::InvalidPrivateKey("bad format".to_string());
        assert!(err.to_string().contains("Invalid private key"));
    }

    #[test]
    fn test_tls_error_display_remaining_variants() {
        let err = TlsError::KeyNotFound("mykey.pem".to_string());
        let s = err.to_string();
        assert!(s.contains("Private key not found"), "got: {}", s);
        assert!(s.contains("mykey.pem"), "got: {}", s);

        let err = TlsError::PemParseError("bad header".to_string());
        let s = err.to_string();
        assert!(s.contains("PEM parse error"), "got: {}", s);

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let err = TlsError::IoError(io_err);
        let s = err.to_string();
        assert!(s.contains("IO error"), "got: {}", s);
    }

    #[test]
    fn test_tls_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let tls_err: TlsError = TlsError::from(io_err);
        assert!(matches!(tls_err, TlsError::IoError(_)));
    }

    #[test]
    fn test_for_development_returns_pem_parse_error() {
        let result = TlsConfiguration::for_development();
        assert!(result.is_err());
        match result {
            Err(TlsError::PemParseError(msg)) => {
                assert!(msg.contains("Development certificates not available"), "got: {}", msg);
            }
            _ => panic!("Expected PemParseError from for_development()"),
        }
    }

    fn pems(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let cert = dir.join("cert.pem");
        let key = dir.join("key.pem");
        std::fs::write(&cert, ck.cert.pem()).unwrap();
        std::fs::write(&key, ck.signing_key.serialize_pem()).unwrap();
        (cert, key)
    }

    #[test]
    fn a_real_certificate_and_key_build_a_server_config() {
        let d = tempfile::tempdir().unwrap();
        let (cert, key) = pems(d.path());
        assert!(TlsConfiguration::from_files(&cert, &key).is_ok());
    }

    #[test]
    fn a_certificate_file_with_no_certificate_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let (_, key) = pems(d.path());
        let empty = d.path().join("empty.pem");
        std::fs::write(&empty, "nothing to see
").unwrap();
        let e = TlsConfiguration::from_files(&empty, &key).err().unwrap();
        assert!(matches!(e, TlsError::InvalidCertificate(_)), "{e}");
    }

    #[test]
    fn a_key_file_with_no_key_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let (cert, _) = pems(d.path());
        let nokey = d.path().join("nokey.pem");
        std::fs::write(&nokey, "nothing to see
").unwrap();
        let e = TlsConfiguration::from_files(&cert, &nokey).err().unwrap();
        assert!(matches!(e, TlsError::InvalidPrivateKey(_)), "{e}");
    }

    #[test]
    fn a_corrupt_certificate_is_refused_not_a_panic() {
        let d = tempfile::tempdir().unwrap();
        let (_, key) = pems(d.path());
        let bad = d.path().join("bad.pem");
        std::fs::write(&bad, "-----BEGIN CERTIFICATE-----
!!!not base64!!!
-----END CERTIFICATE-----
").unwrap();
        let e = TlsConfiguration::from_files(&bad, &key).err().unwrap();
        assert!(matches!(e, TlsError::InvalidCertificate(_)), "{e}");
    }

    #[test]
    fn a_certificate_that_does_not_match_the_key_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let (cert, _) = pems(d.path());
        let other = tempfile::tempdir().unwrap();
        let (_, other_key) = pems(other.path());
        assert!(TlsConfiguration::from_files(&cert, &other_key).is_err());
    }

    // ── client certificates (R25) ────────────────────────────────────────────────────────────────

    mod client_auth {
        use super::super::*;
        use crate::server::mtls::test_pki::{ca, leaf, Ca, Leaf};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        struct Setup {
            _dir: tempfile::TempDir,
            config: Arc<ServerConfig>,
            server_ca: Ca,
        }

        fn setup(mode: ClientAuth) -> (Setup, Ca) {
            setup_with_crl(mode, |_| None)
        }

        /// `crl` makes the CRL file's PEM from the client CA, or none.
        fn setup_with_crl(mode: ClientAuth, crl: impl FnOnce(&Ca) -> Option<String>) -> (Setup, Ca) {
            let server_ca = ca("server ca");
            let client_ca = ca("client ca");
            let server = leaf(&server_ca, &["dns:localhost"]);
            let dir = tempfile::tempdir().unwrap();
            let p = |n: &str| dir.path().join(n);
            std::fs::write(p("cert.pem"), &server.cert_pem).unwrap();
            std::fs::write(p("key.pem"), &server.key_pem).unwrap();
            std::fs::write(p("ca.pem"), &client_ca.pem).unwrap();
            let crl_file = crl(&client_ca).map(|pem| {
                std::fs::write(p("crl.pem"), pem).unwrap();
                p("crl.pem")
            });
            let config = TlsConfiguration::from_files_with_client_auth_and_crl(
                p("cert.pem"),
                p("key.pem"),
                p("ca.pem"),
                mode,
                crl_file.as_deref(),
            )
            .unwrap()
            .config;
            (Setup { _dir: dir, config, server_ca }, client_ca)
        }

        /// Run one handshake; the server's result and the peer's leaf certificate it saw.
        async fn handshake(s: &Setup, client_cert: Option<&Leaf>) -> Result<Option<Vec<u8>>, String> {
            let mut roots = rustls::RootCertStore::empty();
            for c in CertificateDer::pem_slice_iter(s.server_ca.pem.as_bytes()) {
                roots.add(c.unwrap()).unwrap();
            }
            let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
            let builder = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots);
            let client_config = match client_cert {
                Some(l) => builder
                    .with_client_auth_cert(
                        CertificateDer::pem_slice_iter(l.cert_pem.as_bytes()).collect::<Result<_, _>>().unwrap(),
                        PrivateKeyDer::from_pem_slice(l.key_pem.as_bytes()).unwrap(),
                    )
                    .unwrap(),
                None => builder.with_no_client_auth(),
            };
            let (c_end, s_end) = tokio::io::duplex(64 * 1024);
            let acceptor = tokio_rustls::TlsAcceptor::from(s.config.clone());
            let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
            let server = async move {
                let mut tls = acceptor.accept(s_end).await.map_err(|e| e.to_string())?;
                let peer = tls
                    .get_ref()
                    .1
                    .peer_certificates()
                    .and_then(|c| c.first())
                    .map(|c| c.as_ref().to_vec());
                let _ = tls.write_all(b"x").await;
                let _ = tls.shutdown().await;
                Ok(peer)
            };
            let client = async move {
                if let Ok(mut tls) = connector.connect("localhost".try_into().unwrap(), c_end).await {
                    let mut b = [0u8; 1];
                    let _ = tls.read(&mut b).await;
                }
            };
            let (r, _) = tokio::join!(server, client);
            r
        }

        #[tokio::test]
        async fn a_certificate_from_the_trusted_ca_is_accepted_and_seen_by_the_server() {
            let (s, client_ca) = setup(ClientAuth::Required);
            let l = leaf(&client_ca, &["uri:commy://acme/alice"]);
            let seen = handshake(&s, Some(&l)).await.unwrap().expect("the server saw a certificate");
            assert_eq!(seen, l.der);
        }

        #[tokio::test]
        async fn required_refuses_a_client_with_no_certificate() {
            let (s, _) = setup(ClientAuth::Required);
            assert!(handshake(&s, None).await.is_err());
        }

        #[tokio::test]
        async fn a_certificate_from_another_ca_is_refused_in_both_modes() {
            for mode in [ClientAuth::Required, ClientAuth::Optional] {
                let (s, _) = setup(mode);
                let stranger = leaf(&ca("stranger ca"), &["uri:commy://acme/alice"]);
                assert!(handshake(&s, Some(&stranger)).await.is_err(), "{mode:?}");
            }
        }

        #[tokio::test]
        async fn optional_accepts_a_client_with_no_certificate() {
            let (s, _) = setup(ClientAuth::Optional);
            assert_eq!(handshake(&s, None).await.unwrap(), None);
        }

        #[tokio::test]
        async fn a_revoked_certificate_is_refused_and_a_valid_one_still_accepted() {
            for mode in [ClientAuth::Required, ClientAuth::Optional] {
                // Build the leaves first: the CRL names the serial of one of them.
                let revoked_leaf = std::cell::RefCell::new(None);
                let valid_leaf = std::cell::RefCell::new(None);
                let (s, _) = setup_with_crl(mode, |client_ca| {
                    let revoked = leaf(client_ca, &["uri:commy://acme/mallory"]);
                    let valid = leaf(client_ca, &["uri:commy://acme/alice"]);
                    let pem = client_ca.crl_pem(&[revoked.serial]);
                    *revoked_leaf.borrow_mut() = Some(revoked);
                    *valid_leaf.borrow_mut() = Some(valid);
                    Some(pem)
                });
                let revoked = revoked_leaf.borrow_mut().take().unwrap();
                let valid = valid_leaf.borrow_mut().take().unwrap();
                let seen = handshake(&s, Some(&valid)).await.unwrap().expect("the valid certificate is seen");
                assert_eq!(seen, valid.der, "{mode:?}: control, a certificate not in the CRL is accepted");
                assert!(handshake(&s, Some(&revoked)).await.is_err(), "{mode:?}: the revoked certificate must fail");
            }
        }

        #[tokio::test]
        async fn without_a_crl_the_same_certificate_is_accepted() {
            // The control for the revocation test: the revoked certificate's only difference is the CRL.
            let (s, client_ca) = setup(ClientAuth::Required);
            let l = leaf(&client_ca, &["uri:commy://acme/mallory"]);
            assert!(handshake(&s, Some(&l)).await.is_ok());
        }

        #[test]
        fn a_missing_empty_or_garbled_crl_file_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            let client_ca = ca("client ca");
            let server = leaf(&ca("server ca"), &["dns:localhost"]);
            let p = |n: &str| dir.path().join(n);
            std::fs::write(p("cert.pem"), &server.cert_pem).unwrap();
            std::fs::write(p("key.pem"), &server.key_pem).unwrap();
            std::fs::write(p("ca.pem"), &client_ca.pem).unwrap();
            std::fs::write(p("empty.pem"), "").unwrap();
            std::fs::write(p("garbled.pem"), "-----BEGIN X509 CRL-----
not base64!
-----END X509 CRL-----
").unwrap();
            for name in ["nope.pem", "empty.pem", "garbled.pem"] {
                let r = TlsConfiguration::from_files_with_client_auth_and_crl(
                    p("cert.pem"), p("key.pem"), p("ca.pem"), ClientAuth::Required, Some(&p(name)));
                assert!(r.is_err(), "{name} must stop start-up");
            }
        }

        #[test]
        fn a_missing_or_empty_client_ca_file_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            let server = leaf(&ca("server ca"), &["dns:localhost"]);
            let p = |n: &str| dir.path().join(n);
            std::fs::write(p("cert.pem"), &server.cert_pem).unwrap();
            std::fs::write(p("key.pem"), &server.key_pem).unwrap();
            std::fs::write(p("empty.pem"), "").unwrap();
            let r = TlsConfiguration::from_files_with_client_auth(p("cert.pem"), p("key.pem"), p("nope.pem"), ClientAuth::Required);
            assert!(matches!(r, Err(TlsError::CertificateNotFound(_))));
            let r = TlsConfiguration::from_files_with_client_auth(p("cert.pem"), p("key.pem"), p("empty.pem"), ClientAuth::Required);
            assert!(r.is_err());
        }
    }
}
