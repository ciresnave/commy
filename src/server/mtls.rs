// SPDX-License-Identifier: MIT
//! Client-certificate identity (R25, `docs/PLAN-R25-AUTH-METHODS.md`).
//!
//! The TLS handshake (rustls's `WebPkiClientVerifier`, see [`crate::server::tls`]) decides whether a
//! client certificate is **trusted**. This module decides **who it names**: the certificate must carry
//! exactly one Subject Alternative Name URI of the form `commy://<tenant>/<client_id>`. The tenant and
//! the client id come from that URI and nowhere else; the certificate's subject, its CN and any other
//! SAN are never consulted. Two `commy://` URIs, or none, is a refusal, never a pick.

use crate::tenant_id::TenantId;
use x509_parser::extensions::GeneralName;
use x509_parser::prelude::{FromDer, X509Certificate};

/// The identity a client certificate names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    pub tenant: String,
    pub client_id: String,
}

/// Why a certificate does not name a Commy identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtlsError {
    /// The DER does not parse as an X.509 certificate.
    Malformed,
    /// No SAN URI starts with `commy://`.
    NoIdentity,
    /// More than one SAN URI starts with `commy://`: which one is meant is not for us to guess.
    Ambiguous,
    /// The URI is not `commy://<tenant>/<client_id>` with a valid tenant and client id.
    BadUri(&'static str),
}

fn valid_client_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= commy_wire::MAX_CLIENT_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | ':'))
}

/// The identity named by a DER-encoded certificate (the verified peer's leaf certificate).
pub fn identity_from_certificate(der: &[u8]) -> Result<PeerIdentity, MtlsError> {
    let (_, cert) = X509Certificate::from_der(der).map_err(|_| MtlsError::Malformed)?;
    let san = cert
        .subject_alternative_name()
        .map_err(|_| MtlsError::Malformed)?;
    let mut found: Option<&str> = None;
    if let Some(san) = san {
        for name in &san.value.general_names {
            if let GeneralName::URI(uri) = name {
                if uri.starts_with("commy://") {
                    if found.is_some() {
                        return Err(MtlsError::Ambiguous);
                    }
                    found = Some(uri);
                }
            }
        }
    }
    let uri = found.ok_or(MtlsError::NoIdentity)?;
    let rest = &uri["commy://".len()..];
    let (tenant, client_id) = rest
        .split_once('/')
        .ok_or(MtlsError::BadUri("expected commy://<tenant>/<client_id>"))?;
    if client_id.contains(['/', '?', '#']) {
        return Err(MtlsError::BadUri("the client id must be one path segment"));
    }
    TenantId::parse(tenant).map_err(|_| MtlsError::BadUri("not a valid tenant id"))?;
    if !valid_client_id(client_id) {
        return Err(MtlsError::BadUri("not a valid client id"));
    }
    Ok(PeerIdentity {
        tenant: tenant.to_string(),
        client_id: client_id.to_string(),
    })
}

#[cfg(test)]
pub(crate) mod test_pki {
    //! Throwaway certificates for tests.
    use rcgen::{
        BasicConstraints, CertificateParams, CertificateRevocationListParams, IsCa, Issuer, KeyIdMethod, KeyPair,
        RevokedCertParams, SanType, SerialNumber,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1000);

    pub struct Ca {
        pub pem: String,
        key: KeyPair,
        params: CertificateParams,
    }

    pub struct Leaf {
        pub cert_pem: String,
        pub key_pem: String,
        pub der: Vec<u8>,
        /// The serial number, so a CRL can name it.
        pub serial: u64,
    }

    impl Ca {
        /// A CRL (PEM) signed by this CA revoking the certificates with these serials.
        pub fn crl_pem(&self, revoked: &[u64]) -> String {
            let issuer = Issuer::new(self.params.clone(), &self.key);
            CertificateRevocationListParams {
                this_update: rcgen::date_time_ymd(2020, 1, 1),
                next_update: rcgen::date_time_ymd(2099, 1, 1),
                crl_number: SerialNumber::from(1),
                issuing_distribution_point: None,
                revoked_certs: revoked
                    .iter()
                    .map(|s| RevokedCertParams {
                        serial_number: SerialNumber::from(*s),
                        revocation_time: rcgen::date_time_ymd(2024, 1, 1),
                        reason_code: None,
                        invalidity_date: None,
                    })
                    .collect(),
                key_identifier_method: KeyIdMethod::Sha256,
            }
            .signed_by(&issuer)
            .unwrap()
            .pem()
            .unwrap()
        }
    }

    pub fn ca(name: &str) -> Ca {
        let mut params = CertificateParams::new(vec![]).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, name);
        let key = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        Ca { pem: cert.pem(), key, params }
    }

    /// A certificate signed by `ca` with exactly these SANs (`dns:` or `uri:` prefixed).
    pub fn leaf(ca: &Ca, sans: &[&str]) -> Leaf {
        let names = sans
            .iter()
            .map(|s| match s.split_once(':').unwrap() {
                ("dns", n) => SanType::DnsName(n.try_into().unwrap()),
                ("uri", n) => SanType::URI(n.try_into().unwrap()),
                _ => panic!("bad san {s}"),
            })
            .collect::<Vec<_>>();
        let mut params = CertificateParams::new(vec![]).unwrap();
        params.subject_alt_names = names;
        let serial = NEXT_SERIAL.fetch_add(1, Ordering::Relaxed);
        params.serial_number = Some(SerialNumber::from(serial));
        let key = KeyPair::generate().unwrap();
        let issuer = Issuer::new(ca.params.clone(), &ca.key);
        let cert = params.signed_by(&key, &issuer).unwrap();
        Leaf { cert_pem: cert.pem(), key_pem: key.serialize_pem(), der: cert.der().to_vec(), serial }
    }
}

#[cfg(test)]
mod tests {
    use super::test_pki::*;
    use super::*;

    fn id(sans: &[&str]) -> Result<PeerIdentity, MtlsError> {
        let ca = ca("test ca");
        identity_from_certificate(&leaf(&ca, sans).der)
    }

    #[test]
    fn a_commy_uri_names_the_tenant_and_the_client() {
        assert_eq!(
            id(&["uri:commy://acme/alice@example.com"]).unwrap(),
            PeerIdentity { tenant: "acme".into(), client_id: "alice@example.com".into() }
        );
    }

    #[test]
    fn other_san_names_are_never_the_identity() {
        assert_eq!(id(&["dns:acme.example.com"]), Err(MtlsError::NoIdentity));
        assert_eq!(id(&["uri:https://acme/alice"]), Err(MtlsError::NoIdentity));
        // a commy URI beside other names is still found
        assert_eq!(
            id(&["dns:host.example", "uri:https://x/y", "uri:commy://acme/bob"]).unwrap().client_id,
            "bob"
        );
    }

    #[test]
    fn two_commy_uris_are_refused_not_picked() {
        assert_eq!(
            id(&["uri:commy://acme/alice", "uri:commy://globex/alice"]),
            Err(MtlsError::Ambiguous)
        );
    }

    #[test]
    fn malformed_commy_uris_are_refused() {
        for bad in [
            "commy://acme",
            "commy://acme/",
            "commy:///alice",
            "commy://ac me/alice",
            "commy://acme/alice/extra",
            "commy://acme/alice?x=1",
            "commy://acme/alice#f",
            "commy://acme/al ice",
        ] {
            assert!(
                matches!(id(&[&format!("uri:{bad}")]), Err(MtlsError::BadUri(_))),
                "{bad}"
            );
        }
        let long = format!("commy://acme/{}", "a".repeat(commy_wire::MAX_CLIENT_ID_LEN + 1));
        assert!(matches!(id(&[&format!("uri:{long}")]), Err(MtlsError::BadUri(_))));
    }

    #[test]
    fn garbage_is_malformed() {
        assert_eq!(identity_from_certificate(b"not a certificate"), Err(MtlsError::Malformed));
    }
}
