#[cfg(all(test, feature = "network"))]
mod tests {
    use super::*;
    use rcgen::generate_simple_self_signed;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn create_tls_acceptor_from_in_memory_pem() {
        // Generate a self-signed certificate with rcgen
        let cert = generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.serialize_pem().unwrap();
        let priv_key_pem = cert.serialize_private_key_pem();

        // Write cert and key to temp files
        let mut cert_file = NamedTempFile::new().expect("create cert temp file");
        cert_file
            .write_all(cert_pem.as_bytes())
            .expect("write cert");
        let cert_path = cert_file.path().to_path_buf();

        let mut key_file = NamedTempFile::new().expect("create key temp file");
        key_file
            .write_all(priv_key_pem.as_bytes())
            .expect("write key");
        let key_path = key_file.path().to_path_buf();

        // Call the manager's create_tls_acceptor
        let result = SharedFileManager::create_tls_acceptor(&cert_path, &key_path).await;
        assert!(
            result.is_ok(),
            "TlsAcceptor construction failed: {:?}",
            result
        );
    }
}
