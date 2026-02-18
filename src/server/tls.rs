//! TLS Configuration and Certificate Loading
//!
//! Handles loading and validation of TLS certificates and keys for WSS support.

use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::{self, BufReader};
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

        // Load certificate
        let cert_file = File::open(cert_path)?;
        let mut cert_reader = BufReader::new(cert_file);
        let certs = certs(&mut cert_reader).map_err(|_| {
            TlsError::InvalidCertificate("Failed to parse PEM certificates".to_string())
        })?;

        if certs.is_empty() {
            return Err(TlsError::InvalidCertificate(
                "No certificates found in file".to_string(),
            ));
        }

        let certificates: Vec<Certificate> = certs.into_iter().map(Certificate).collect();

        // Load private key
        let key_file = File::open(key_path)?;
        let mut key_reader = BufReader::new(key_file);
        let mut keys = pkcs8_private_keys(&mut key_reader).map_err(|_| {
            TlsError::InvalidPrivateKey("Failed to parse PEM private keys".to_string())
        })?;

        if keys.is_empty() {
            return Err(TlsError::InvalidPrivateKey(
                "No private keys found in file".to_string(),
            ));
        }

        let private_key = PrivateKey(keys.remove(0));

        // Create ServerConfig
        let config = ServerConfig::builder()
            .with_safe_defaults()
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
    #[allow(dead_code)]
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
}
