//! Power-cut test (plan R5): the real `commy` binary is killed without any chance to shut down, then
//! started again on the same state directory. Services and grants must be what they were.

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_tungstenite::tungstenite::Message;

type Client = tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>;

struct Certs {
    cert_path: String,
    key_path: String,
    cert_der: Vec<u8>,
}

fn self_signed(dir: &Path) -> Certs {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, ck.cert.pem()).unwrap();
    std::fs::write(&key_path, ck.signing_key.serialize_pem()).unwrap();
    Certs {
        cert_path: cert_path.to_string_lossy().into_owned(),
        key_path: key_path.to_string_lossy().into_owned(),
        cert_der: ck.cert.der().to_vec(),
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// A running server process. Dropping it kills it.
struct Server {
    child: Child,
    port: u16,
}

impl Server {
    fn start(work: &Path, state: &Path, certs: &Certs) -> Server {
        let port = free_port();
        let child = Command::new(env!("CARGO_BIN_EXE_commy"))
            .current_dir(work)
            .env("COMMY_ALLOW_DEV_AUTH", "1")
            .env("COMMY_LISTEN_ADDR", "127.0.0.1")
            .env("COMMY_LISTEN_PORT", port.to_string())
            .env("COMMY_TLS_CERT_PATH", &certs.cert_path)
            .env("COMMY_TLS_KEY_PATH", &certs.key_path)
            .env("COMMY_STATE_DIR", state)
            .env("DATABASE_URL", "memory")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the commy binary");
        Server { child, port }
    }

    /// Kill without any chance to clean up: no signal handler, no shutdown path.
    fn kill(mut self) {
        self.child.kill().unwrap();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn connect(port: u16, certs: &Certs) -> Client {
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
    roots.add(CertificateDer::from(certs.cert_der.clone())).unwrap();
    let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    // The process needs a moment to open its port.
    for _ in 0..200 {
        if let Ok(tcp) = TcpStream::connect(("127.0.0.1", port)).await {
            if let Ok(tls) = connector.connect(ServerName::try_from("localhost").unwrap(), tcp).await {
                let (ws, _) = tokio_tungstenite::client_async(format!("wss://localhost:{port}/"), tls)
                    .await
                    .expect("WebSocket upgrade");
                return ws;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the server never accepted a connection on {port}");
}

async fn ask(c: &mut Client, v: Value) -> Value {
    c.send(Message::Text(v.to_string().into())).await.unwrap();
    loop {
        let m = tokio::time::timeout(std::time::Duration::from_secs(10), c.next())
            .await
            .expect("timed out waiting for the server")
            .expect("connection closed")
            .expect("websocket error");
        if let Message::Text(t) = m {
            return serde_json::from_str(t.as_str()).unwrap();
        }
    }
}

async fn login(c: &mut Client, tenant: &str, id: &str, scopes: Option<Value>) {
    let mut data = json!({"tenant_id": tenant, "api_key": "x", "client_id": id});
    if let Some(s) = scopes {
        data["dev_scopes"] = s;
    }
    let r = ask(c, json!({"type":"Authenticate","data":data})).await;
    assert_eq!(r["data"]["success"], true, "{r}");
}

fn path_request(service_id: &str) -> Value {
    json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":service_id}})
}

#[tokio::test]
async fn a_killed_server_comes_back_with_its_services_and_grants_and_revokes() {
    let work = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let certs = self_signed(work.path());
    let tenant = format!("rt_{}", uuid::Uuid::new_v4().simple());

    // First life: create a service, give bob and carol read, open it to local mapping, take
    // carol's access away again, then die.
    let first = Server::start(work.path(), state.path(), &certs);
    let service_id = {
        let mut admin = connect(first.port, &certs).await;
        login(&mut admin, &tenant, "admin", None).await;
        let r = ask(&mut admin, json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        for who in ["bob", "carol"] {
            let r = ask(&mut admin, json!({"type":"Grant","data":{"request_id":"g","client_id":who,"service_id":id,"permissions":["service:read"]}})).await;
            assert_eq!(r["type"], "GrantAck", "{r}");
        }
        let r = ask(&mut admin, json!({"type":"SetLocalMapping","data":{"request_id":"m","service_id":id,"enabled":true}})).await;
        assert_eq!(r["type"], "LocalMappingAck", "{r}");
        let r = ask(&mut admin, json!({"type":"Revoke","data":{"request_id":"v","client_id":"carol","service_id":id}})).await;
        assert_eq!(r["type"], "GrantAck", "{r}");
        id
    };
    first.kill();

    // Second life, same state directory.
    let second = Server::start(work.path(), state.path(), &certs);
    let mut bob = connect(second.port, &certs).await;
    login(&mut bob, &tenant, "bob", Some(json!([]))).await;
    let r = ask(&mut bob, path_request(&service_id)).await;
    assert_eq!(r["type"], "ServiceFilePath", "bob's grant survived the kill: {r}");

    let mut carol = connect(second.port, &certs).await;
    login(&mut carol, &tenant, "carol", Some(json!([]))).await;
    let r = ask(&mut carol, path_request(&service_id)).await;
    assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "carol's revoked access must not come back: {r}");

    let mut eve = connect(second.port, &certs).await;
    login(&mut eve, &tenant, "eve", Some(json!([]))).await;
    assert_eq!(ask(&mut eve, path_request(&service_id)).await["data"]["code"], "PERMISSION_DENIED");

    // the creator still administers it, and the service is still open to local mapping
    let mut admin = connect(second.port, &certs).await;
    login(&mut admin, &tenant, "admin", Some(json!([]))).await;
    let r = ask(&mut admin, path_request(&service_id)).await;
    assert!(r["data"]["path"].is_string(), "the admin's service is still open to local mapping: {r}");
    second.kill();

    let _ = std::fs::remove_dir_all(work.path().join(format!("tenant_{tenant}")));
}
