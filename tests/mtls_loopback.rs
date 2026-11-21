// SPDX-License-Identifier: MIT
//! mTLS end to end (R25): the real `WssServer` with client certificates on, the real
//! `FrameworkVerifier`, and real clients over TLS and WebSocket. The identity of an `mtls`
//! `Authenticate` is the client certificate's `commy://<tenant>/<client_id>` SAN URI.

use auth_framework::config::AuthConfig;
use auth_framework::AuthFramework;
use commy::server::framework_verifier::{derive_tenant_secret, FrameworkVerifier};
use commy::server::tls::ClientAuth;
use commy::server::verify::MethodFilter;
use commy::{Server, WssServer, WssServerConfig};
use futures::{SinkExt, StreamExt};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, SanType};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio_rustls::rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, ServerName};
use tokio_tungstenite::tungstenite::Message;

const MASTER: &str = "0123456789abcdef0123456789abcdef-master";

type Client = tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>;

struct Ca {
    pem: String,
    key: KeyPair,
    params: CertificateParams,
}

struct Leaf {
    cert_pem: String,
    key_pem: String,
}

fn ca(name: &str) -> Ca {
    let mut params = CertificateParams::new(vec![]).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.distinguished_name.push(DnType::CommonName, name);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    Ca { pem: cert.pem(), key, params }
}

fn leaf(ca: &Ca, san: SanType) -> Leaf {
    let mut params = CertificateParams::new(vec![]).unwrap();
    params.subject_alt_names = vec![san];
    let key = KeyPair::generate().unwrap();
    let issuer = Issuer::new(ca.params.clone(), &ca.key);
    let cert = params.signed_by(&key, &issuer).unwrap();
    Leaf { cert_pem: cert.pem(), key_pem: key.serialize_pem() }
}

fn identity(ca: &Ca, uri: &str) -> Leaf {
    leaf(ca, SanType::URI(uri.try_into().unwrap()))
}

struct Rig {
    _dir: tempfile::TempDir,
    port: u16,
    server_ca: Ca,
    client_ca: Ca,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// The real server: client certificates in `mode`, tenants `acme` and `globex`, `methods` accepted.
async fn start(mode: ClientAuth, methods: &[&str]) -> Rig {
    let server_ca = ca("server ca");
    let client_ca = ca("client ca");
    let server_cert = leaf(&server_ca, SanType::DnsName("localhost".try_into().unwrap()));
    let dir = tempfile::tempdir().unwrap();
    let p = |n: &str| dir.path().join(n).to_string_lossy().into_owned();
    std::fs::write(p("cert.pem"), &server_cert.cert_pem).unwrap();
    std::fs::write(p("key.pem"), &server_cert.key_pem).unwrap();
    std::fs::write(p("clients.pem"), &client_ca.pem).unwrap();

    let mut verifier = FrameworkVerifier::new(dir.path(), MASTER).unwrap();
    for tenant in ["acme", "globex"] {
        let mut f = AuthFramework::new(AuthConfig::new().secret(derive_tenant_secret(MASTER, tenant)));
        f.initialize().await.unwrap();
        verifier = verifier.with_framework(tenant, Arc::new(f)).await;
    }
    let verifier = MethodFilter::new(verifier, methods.iter().map(|m| m.to_string()).collect());
    let server = Arc::new(RwLock::new(Server::with_dev_auth(false).with_verifier(Arc::new(verifier))));

    let port = free_port();
    let config = WssServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port,
        cert_path: Some(p("cert.pem")),
        key_path: Some(p("key.pem")),
        client_ca_path: Some(p("clients.pem")),
        client_auth: Some(mode),
        ..Default::default()
    };
    let mut wss = WssServer::new(config, server);
    wss.initialize_tls().expect("TLS with client certificates must initialise");
    tokio::spawn(async move {
        let _ = wss.run().await;
    });
    Rig { _dir: dir, port, server_ca, client_ca }
}

/// Connect, presenting `client` if given. `Err` when the handshake or the upgrade failed.
async fn connect(rig: &Rig, client: Option<&Leaf>) -> Result<Client, String> {
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
    for c in CertificateDer::pem_slice_iter(rig.server_ca.pem.as_bytes()) {
        roots.add(c.unwrap()).unwrap();
    }
    let builder = tokio_rustls::rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots);
    let config = match client {
        Some(l) => builder
            .with_client_auth_cert(
                CertificateDer::pem_slice_iter(l.cert_pem.as_bytes()).collect::<Result<_, _>>().unwrap(),
                PrivateKeyDer::from_pem_slice(l.key_pem.as_bytes()).unwrap(),
            )
            .unwrap(),
        None => builder.with_no_client_auth(),
    };
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let mut tcp = None;
    for _ in 0..50 {
        match TcpStream::connect(("127.0.0.1", rig.port)).await {
            Ok(s) => {
                tcp = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
        }
    }
    let tls = connector
        .connect(ServerName::try_from("localhost").unwrap(), tcp.expect("server never listened"))
        .await
        .map_err(|e| e.to_string())?;
    let (ws, _) = tokio_tungstenite::client_async(format!("wss://localhost:{}/", rig.port), tls)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ws)
}

/// Send an `mtls` Authenticate for `tenant` and return the reply's `data`.
async fn mtls_authenticate(c: &mut Client, tenant: &str) -> Value {
    let msg = json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"mtls"}});
    c.send(Message::Text(msg.to_string().into())).await.unwrap();
    let reply = tokio::time::timeout(std::time::Duration::from_secs(5), c.next())
        .await
        .expect("timed out")
        .expect("connection closed")
        .expect("websocket error");
    match reply {
        Message::Text(t) => serde_json::from_str::<Value>(t.as_str()).unwrap()["data"].clone(),
        other => panic!("expected text, got {other:?}"),
    }
}

#[tokio::test]
async fn a_certificate_naming_the_tenant_authenticates_as_its_client_id() {
    let rig = start(ClientAuth::Required, &["mtls"]).await;
    let cert = identity(&rig.client_ca, "commy://acme/alice");
    let mut c = connect(&rig, Some(&cert)).await.unwrap();
    let r = mtls_authenticate(&mut c, "acme").await;
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["client_id"], "alice", "{r}");
}

#[tokio::test]
async fn a_certificate_naming_another_tenant_is_refused() {
    let rig = start(ClientAuth::Required, &["mtls"]).await;
    let cert = identity(&rig.client_ca, "commy://globex/alice");
    let mut c = connect(&rig, Some(&cert)).await.unwrap();
    let r = mtls_authenticate(&mut c, "acme").await;
    assert_eq!(r["success"], false, "{r}");
}

#[tokio::test]
async fn a_certificate_that_names_no_commy_identity_is_refused() {
    let rig = start(ClientAuth::Required, &["mtls"]).await;
    let cert = leaf(&rig.client_ca, SanType::DnsName("alice.example.com".try_into().unwrap()));
    let mut c = connect(&rig, Some(&cert)).await.unwrap();
    let r = mtls_authenticate(&mut c, "acme").await;
    assert_eq!(r["success"], false, "{r}");
}

#[tokio::test]
async fn optional_lets_a_client_without_a_certificate_connect_but_not_use_mtls() {
    let rig = start(ClientAuth::Optional, &["mtls"]).await;
    let mut c = connect(&rig, None).await.unwrap();
    let r = mtls_authenticate(&mut c, "acme").await;
    assert_eq!(r["success"], false, "{r}");
}

#[tokio::test]
async fn required_refuses_a_client_without_a_certificate_and_one_from_another_ca() {
    let rig = start(ClientAuth::Required, &["mtls"]).await;
    // TLS 1.3: the client's handshake can finish before the server's alert arrives, so the failure
    // shows at the upgrade or at the first exchange.
    let refused = |r: Result<Client, String>| async move {
        match r {
            Err(_) => true,
            Ok(mut c) => {
                let msg = json!({"type":"Authenticate","data":{"tenant_id":"acme","request_id":"a","protocol_version":1,"method":"mtls"}});
                let _ = c.send(Message::Text(msg.to_string().into())).await;
                !matches!(c.next().await, Some(Ok(Message::Text(_))))
            }
        }
    };
    assert!(refused(connect(&rig, None).await).await, "no certificate");
    let stranger = identity(&ca("stranger ca"), "commy://acme/alice");
    assert!(refused(connect(&rig, Some(&stranger)).await).await, "certificate from another CA");
}

#[tokio::test]
async fn a_method_left_out_of_auth_methods_is_refused_even_with_a_good_certificate() {
    let rig = start(ClientAuth::Required, &["api_key", "jwt"]).await;
    let cert = identity(&rig.client_ca, "commy://acme/alice");
    let mut c = connect(&rig, Some(&cert)).await.unwrap();
    let r = mtls_authenticate(&mut c, "acme").await;
    assert_eq!(r["success"], false, "{r}");
}
