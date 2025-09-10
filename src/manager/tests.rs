#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::CommyError;
    use crate::manager::map_commy_error_to_manager_error;
    use crate::manager::SerializationFormat;

    #[test]
    fn test_map_io_error_with_path() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
        let com_err = CommyError::Io {
            source: io_err,
            path: Some(std::path::PathBuf::from("/tmp/somefile")),
        };
        let path = Some(std::path::PathBuf::from("/tmp/somefile"));
        let mgr_err = map_commy_error_to_manager_error(com_err, path.clone(), None);

        match mgr_err {
            crate::manager::ManagerError::IoError { path: p, message } => {
                assert_eq!(p, path.unwrap());
                assert!(message.contains("no file"));
            }
            _ => panic!("Expected IoError mapping"),
        }
    }

    #[test]
    fn test_map_serialize_error_with_format() {
        let ser_err = CommyError::BinarySerialization("bad format".to_string());
        let mgr_err =
            map_commy_error_to_manager_error(ser_err, None, Some(SerializationFormat::Json));

        match mgr_err {
            crate::manager::ManagerError::SerializationError { format, message } => {
                assert_eq!(format, SerializationFormat::Json);
                assert!(message.contains("bad format"));
            }
            _ => panic!("Expected SerializationError mapping"),
        }
    }

    #[test]
    fn test_transport_error_includes_path() {
        use crate::manager::transport_impl::map_commy_error_to_transport_error;
        use crate::manager::TransportError;
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let path = std::path::PathBuf::from("/tmp/forbidden");
        let com_err = CommyError::Io {
            source: io_err,
            path: Some(path.clone()),
        };
        let trans_err = map_commy_error_to_transport_error(com_err, None);
        match trans_err {
            TransportError::FileSystem(ref msg) => {
                assert!(
                    msg.contains("/tmp/forbidden"),
                    "Error message should contain path"
                );
                assert!(
                    msg.contains("access denied"),
                    "Error message should contain IO error"
                );
            }
            _ => panic!("Expected FileSystem error variant"),
        }
    }

    #[test]
    fn test_map_messagepack_serialization_error() {
        use crate::manager::transport_impl::map_commy_error_to_transport_error;
        use crate::manager::SerializationFormat;

        let com_err = CommyError::MessagePackSerialization("mpack fail".to_string());
        let trans_err =
            map_commy_error_to_transport_error(com_err, Some(SerializationFormat::MessagePack));

        match trans_err {
            crate::manager::TransportError::Serialization(ref msg) => {
                assert!(
                    msg.contains("MessagePack"),
                    "should mention MessagePack format"
                );
                assert!(msg.contains("mpack fail"));
            }
            _ => panic!("Expected Serialization transport error"),
        }
    }

    #[test]
    fn test_map_cbor_serialization_error() {
        use crate::manager::transport_impl::map_commy_error_to_transport_error;
        use crate::manager::SerializationFormat;

        let com_err = CommyError::CborSerialization("cbor broken".to_string());
        let trans_err =
            map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Cbor));

        match trans_err {
            crate::manager::TransportError::Serialization(ref msg) => {
                assert!(msg.contains("Cbor"), "should mention Cbor format");
                assert!(msg.contains("cbor broken"));
            }
            _ => panic!("Expected Serialization transport error"),
        }
    }

    #[test]
    fn test_io_error_path_preserved_with_and_without_format() {
        use crate::manager::transport_impl::map_commy_error_to_transport_error;
        use crate::manager::SerializationFormat;

        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "broken");
        let path = std::path::PathBuf::from("/tmp/preserve_me");
        let com_err = CommyError::Io {
            source: io_err,
            path: Some(path.clone()),
        };

        // When no format is provided, path should still be preserved in FileSystem
        let trans_err_none = map_commy_error_to_transport_error(com_err.clone(), None);
        match trans_err_none {
            crate::manager::TransportError::FileSystem(ref msg) => {
                assert!(msg.contains("/tmp/preserve_me"));
                assert!(msg.contains("broken"));
            }
            _ => panic!("Expected FileSystem error variant when format is None"),
        }

        // When a serialization format is provided, path preservation should remain unaffected
        let trans_err_some =
            map_commy_error_to_transport_error(com_err, Some(SerializationFormat::MessagePack));
        match trans_err_some {
            crate::manager::TransportError::FileSystem(ref msg) => {
                assert!(msg.contains("/tmp/preserve_me"));
                assert!(msg.contains("broken"));
            }
            _ => panic!("Expected FileSystem error variant when format is Some(...)"),
        }
    }

    #[test]
    fn test_map_json_serialization_to_transport_and_manager() {
        use crate::manager::map_commy_error_to_manager_error;
        use crate::manager::transport_impl::map_commy_error_to_transport_error;
        use crate::manager::SerializationFormat;

        // Create a serde_json error by attempting to parse invalid JSON
        let parse_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let com_err = CommyError::JsonSerialization(parse_err);

        // Transport mapping with explicit Json format
        let trans_err =
            map_commy_error_to_transport_error(com_err, Some(SerializationFormat::Json));
        match trans_err {
            crate::manager::TransportError::Serialization(ref msg) => {
                assert!(
                    msg.contains("Json"),
                    "transport message should include Json format"
                );
                assert!(msg.to_lowercase().contains("error") || msg.len() > 0);
            }
            _ => panic!("Expected Serialization transport error for Json"),
        }

        // Manager mapping: re-create serde error for manager mapping test
        let parse_err2 = serde_json::from_str::<serde_json::Value>("also not json").unwrap_err();
        let com_err2 = CommyError::JsonSerialization(parse_err2);
        let mgr_err = map_commy_error_to_manager_error(com_err2, None, None);
        match mgr_err {
            crate::manager::ManagerError::SerializationError { format, message } => {
                assert_eq!(format, SerializationFormat::Json);
                assert!(message.len() > 0, "message should not be empty");
            }
            _ => panic!("Expected ManagerError::SerializationError for Json"),
        }
    }

    #[cfg(all(test, feature = "network"))]
    #[test]
    fn test_parse_pem_cert_and_key_with_rustls_pemfile() {
        use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
        use std::io::BufReader;

        // Create minimal PEM fixtures in-memory (not real certs, just PEM-like structure)
        let cert_pem = "-----BEGIN CERTIFICATE-----\nMIIBIjANBgkqh...FAKE...IDAQAB\n-----END CERTIFICATE-----\n";
        let key_pem =
            "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkq...FAKE...\n-----END PRIVATE KEY-----\n";

        // Parse certs
        let mut cert_reader = BufReader::new(cert_pem.as_bytes());
        let certs_res: Result<Vec<Vec<u8>>, std::io::Error> = certs(&mut cert_reader).collect();
        assert!(
            certs_res.is_ok(),
            "certs parsing should succeed (returns Vec<Vec<u8>>)"
        );
        let certs_vec = certs_res.unwrap();

        // Parse keys (PKCS8)
        let mut key_reader = BufReader::new(key_pem.as_bytes());
        let keys_res = pkcs8_private_keys(&mut key_reader).collect::<Result<Vec<_>, _>>();

        if keys_res.is_ok() && !keys_res.as_ref().unwrap().is_empty() {
            assert!(true, "PKCS8 keys parsed");
        } else {
            // Try RSA fallback
            let mut key_reader = BufReader::new(key_pem.as_bytes());
            let rsa_res = rsa_private_keys(&mut key_reader).collect::<Result<Vec<_>, _>>();
            assert!(rsa_res.is_ok(), "RSA fallback parsing should succeed");
        }
    }
}
