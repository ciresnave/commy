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
        let cert_path = cert_path.as_ref();
        let key_path = key_path.as_ref();

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
        let config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| {
                TlsError::InvalidCertificate(format!("Failed to select TLS versions: {:?}", e))
            })?
            .with_no_client_auth()
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
}
