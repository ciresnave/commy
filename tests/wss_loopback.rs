//! Real TLS + WebSocket loopback: a self-signed certificate, the real `WssServer` accept loop and
//! connection loop, and real clients. This is the path the handler unit tests cannot reach: the
//! TLS provider choice, the binary-frame dispatch and the outbound-queue select.

use commy::{Server, WssServer, WssServerConfig};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_tungstenite::tungstenite::Message;

type Client = tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>;

struct Certs {
    _dir: tempfile::TempDir,
    cert_path: String,
    key_path: String,
    cert_der: Vec<u8>,
}

fn self_signed() -> Certs {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::write(&cert_path, ck.cert.pem()).unwrap();
    std::fs::write(&key_path, ck.signing_key.serialize_pem()).unwrap();
    Certs {
        cert_path: cert_path.to_string_lossy().into_owned(),
        key_path: key_path.to_string_lossy().into_owned(),
        cert_der: ck.cert.der().to_vec(),
        _dir: dir,
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Start the real server on a free port. Returns (port, the shared Server, certs).
async fn start_server() -> (u16, Arc<RwLock<Server>>, Certs) {
    let certs = self_signed();
    let port = free_port();
    let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
    let config = WssServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port,
        cert_path: Some(certs.cert_path.clone()),
        key_path: Some(certs.key_path.clone()),
        ..Default::default()
    };
    let mut wss = WssServer::new(config, Arc::clone(&server));
    wss.initialize_tls().expect("TLS must initialise with a real certificate and key");
    tokio::spawn(async move {
        let _ = wss.run().await;
    });
    (port, server, certs)
}

async fn connect(port: u16, certs: &Certs) -> Client {
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
    roots
        .add(tokio_rustls::rustls::pki_types::CertificateDer::from(certs.cert_der.clone()))
        .unwrap();
    let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    // The server task may not be listening yet.
    let mut tcp = None;
    for _ in 0..50 {
        match TcpStream::connect(("127.0.0.1", port)).await {
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
        .expect("TLS handshake");
    let (ws, _resp) = tokio_tungstenite::client_async(format!("wss://localhost:{port}/"), tls)
        .await
        .expect("WebSocket upgrade");
    ws
}

async fn say(c: &mut Client, v: Value) {
    c.send(Message::Text(v.to_string().into())).await.unwrap();
}

async fn next(c: &mut Client) -> Message {
    tokio::time::timeout(std::time::Duration::from_secs(5), c.next())
        .await
        .expect("timed out waiting for the server")
        .expect("connection closed")
        .expect("websocket error")
}

async fn next_text(c: &mut Client) -> Value {
    match next(c).await {
        Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        other => panic!("expected text, got {other:?}"),
    }
}

async fn next_binary(c: &mut Client) -> Vec<u8> {
    match next(c).await {
        Message::Binary(b) => b.to_vec(),
        other => panic!("expected binary, got {other:?}"),
    }
}

async fn authenticate(c: &mut Client, tenant: &str) {
    say(c, json!({"type":"Authenticate","data":{"tenant_id":tenant,"api_key":"x"}})).await;
    let r = next_text(c).await;
    assert_eq!(r["data"]["success"], true, "{r}");
}

fn cleanup(tenant: &str) {
    let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
}

/// The server must start TLS with a real certificate and key (no process-level provider panic).
#[tokio::test]
async fn tls_initialises_and_a_client_can_complete_the_handshake() {
    let (port, _server, certs) = start_server().await;
    let mut c = connect(port, &certs).await;
    say(&mut c, json!({"type":"Heartbeat","data":{"session_id":"s"}})).await;
    let r = next_text(&mut c).await;
    assert_ne!(r["type"], "Error", "{r}");
}

/// Two real clients: A maps the file locally, B mirrors it remotely. A's flush reaches B as range
/// frames; B's binary frames and `FlushEnd` reach A as `RangesUpdated`. Both directions need the
/// outbound-queue select and the binary dispatch.
#[tokio::test]
async fn a_local_and_a_remote_client_see_each_others_flushes_over_real_wss() {
    let tenant = format!("lb_{}", uuid::Uuid::new_v4().simple());
    let (port, server, certs) = start_server().await;
    let mut a = connect(port, &certs).await;
    let mut b = connect(port, &certs).await;
    authenticate(&mut a, &tenant).await;
    authenticate(&mut b, &tenant).await;

    // A creates a 4 KiB service; an admin opens it to local mapping.
    say(&mut a, json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":4096}})).await;
    let r = next_text(&mut a).await;
    assert_eq!(r["type"], "Service", "{r}");
    let service_id = r["data"]["service_id"].as_str().unwrap().to_string();
    let (file_id, path) = {
        let mut g = server.write().await;
        assert!(g.set_trusted_local(&service_id, true));
        let rec = &g.service_registry[&service_id];
        (rec.file_id, rec.file_path.clone())
    };

    // A: local handshake.
    say(&mut a, json!({"type":"GetServiceFilePath","data":{"request_id":"g","service_id":service_id}})).await;
    let r = next_text(&mut a).await;
    assert_eq!(r["type"], "ServiceFilePath", "{r}");
    let nonce = std::fs::read_to_string(r["data"]["nonce_path"].as_str().unwrap()).unwrap();
    say(&mut a, json!({"type":"ConfirmLocal","data":{"request_id":"c","file_id":file_id,"nonce":nonce}})).await;
    let r = next_text(&mut a).await;
    assert_eq!(r["type"], "ConfirmLocalAck", "{r}");

    // B: remote mirror via Resync (4 KiB: one frame, then ResyncDone).
    say(&mut b, json!({"type":"Resync","data":{"request_id":"rs","file_id":file_id}})).await;
    let snapshot = next_binary(&mut b).await;
    assert_eq!(commy_wire::RangeFrame::decode(&snapshot).unwrap().data.len(), 4096);
    let r = next_text(&mut b).await;
    assert_eq!(r["type"], "ResyncDone", "{r}");

    // A writes through the file and reports the range; B gets the bytes as a binary frame.
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.seek(SeekFrom::Start(64)).unwrap();
        f.write_all(&[1, 2, 3, 4]).unwrap();
    }
    say(&mut a, json!({"type":"RangesChanged","data":{"request_id":"f1","file_id":file_id,"ranges":[{"offset":64,"len":4}]}})).await;
    let ack = next_text(&mut a).await;
    assert_eq!(ack["type"], "FlushAck", "{ack}");
    let frame = commy_wire::RangeFrame::decode(&next_binary(&mut b).await).unwrap();
    assert_eq!((frame.offset, frame.data.as_slice()), (64, &[1u8, 2, 3, 4][..]));
    assert_eq!(frame.seq, ack["data"]["seq"].as_u64().unwrap());

    // B sends a binary frame and FlushEnd; A is told which range changed.
    let out = commy_wire::RangeFrame { file_id, seq: frame.seq, offset: 200, data: vec![9, 9] };
    b.send(Message::Binary(out.encode().into())).await.unwrap();
    say(&mut b, json!({"type":"FlushEnd","data":{"request_id":"f2","file_id":file_id}})).await;
    let ack = next_text(&mut b).await;
    assert_eq!(ack["type"], "FlushAck", "{ack}");
    let upd = next_text(&mut a).await;
    assert_eq!(upd["type"], "RangesUpdated", "{upd}");
    assert_eq!(upd["data"]["ranges"], json!([{"offset":200,"len":2}]));
    assert_eq!(&std::fs::read(&path).unwrap()[200..202], &[9, 9]);

    cleanup(&tenant);
}

/// A resize over real WSS (prepare and commit): the drain barrier reaches both clients through the
/// outbound queue, the requester alone is handed the new file, fills it with a binary range frame and
/// `FlushEnd`, answers `RotationFilled`, and both receive the commit.
#[tokio::test]
async fn a_resize_drains_prepares_fills_and_commits_over_real_wss() {
    let tenant = format!("lr_{}", uuid::Uuid::new_v4().simple());
    let (port, server, certs) = start_server().await;
    {
        let mut g = server.write().await;
        g.drain_timeout = std::time::Duration::from_secs(3);
        g.fill_timeout = std::time::Duration::from_secs(5);
        g.rotation_min_interval = std::time::Duration::ZERO;
    }
    let mut a = connect(port, &certs).await;
    let mut b = connect(port, &certs).await;
    authenticate(&mut a, &tenant).await;
    authenticate(&mut b, &tenant).await;
    say(&mut a, json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":4096}})).await;
    let r = next_text(&mut a).await;
    let service_id = r["data"]["service_id"].as_str().unwrap().to_string();
    let file_id = server.read().await.service_registry[&service_id].file_id;

    // Both clients become remote mirrors (Resync: one frame, then ResyncDone).
    for c in [&mut a, &mut b] {
        say(c, json!({"type":"Resync","data":{"request_id":"rs","file_id":file_id}})).await;
        let _frame = next_binary(c).await;
        assert_eq!(next_text(c).await["type"], "ResyncDone");
    }

    say(&mut a, json!({"type":"RotateRequest","data":{"request_id":"q","service_id":service_id,"reason":"RESIZE","new_size":8192}})).await;
    // A gets RotationAccepted and RotationPending in either order; B gets RotationPending.
    let mut a_pending = None;
    for _ in 0..2 {
        let m = next_text(&mut a).await;
        match m["type"].as_str().unwrap() {
            "RotationAccepted" => {}
            "RotationPending" => a_pending = Some(m),
            other => panic!("unexpected {other}: {m}"),
        }
    }
    let a_pending = a_pending.expect("A must receive RotationPending");
    let b_pending = next_text(&mut b).await;
    assert_eq!(b_pending["type"], "RotationPending", "{b_pending}");
    assert_eq!(a_pending["data"]["fill_ms"], 5000);

    for (c, p) in [(&mut a, &a_pending), (&mut b, &b_pending)] {
        say(c, json!({"type":"RotationDrained","data":{"request_id":"d","rotation_id":p["data"]["rotation_id"],"file_id":file_id,"seq":0}})).await;
    }

    // The requester alone is handed the prepared file (no path: it is a remote mirror).
    let prep = next_text(&mut a).await;
    assert_eq!(prep["type"], "RotationPrepared", "{prep}");
    assert!(prep["data"].get("path").is_none(), "{prep}");
    let new_fid = prep["data"]["new_file_id"].as_u64().unwrap() as u32;
    let frame = commy_wire::RangeFrame { file_id: new_fid, seq: 0, offset: 300, data: vec![8, 8] };
    a.send(Message::Binary(frame.encode().into())).await.unwrap();
    say(&mut a, json!({"type":"FlushEnd","data":{"request_id":"fe","file_id":new_fid}})).await;
    let ack = next_text(&mut a).await;
    assert_eq!((ack["type"].as_str(), ack["data"]["seq"].as_u64()), (Some("FlushAck"), Some(0)), "{ack}");
    say(&mut a, json!({"type":"RotationFilled","data":{"request_id":"fl","rotation_id":prep["data"]["rotation_id"]}})).await;

    let ca = next_text(&mut a).await;
    let cb = next_text(&mut b).await;
    assert_eq!(ca["type"], "RotationCommit", "{ca}");
    assert_eq!(ca, cb);
    assert_eq!(ca["data"]["size"], 8192);
    assert_eq!(ca["data"]["old_file_id"], file_id);
    assert_eq!(ca["data"]["new_file_id"], new_fid);
    let live = server.read().await.service_registry[&service_id].file_path.clone();
    assert_eq!(&std::fs::read(live).unwrap()[300..302], &[8, 8]);

    cleanup(&tenant);
}

/// What an SDK end-to-end run does against the real server: open a service to local mapping with
/// the administration message (no code call), get the path and nonce file, confirm.
#[tokio::test]
async fn an_admin_opens_local_mapping_and_a_client_completes_the_handshake_over_real_wss() {
    let tenant = format!("lo_{}", uuid::Uuid::new_v4().simple());
    let (port, _server, certs) = start_server().await;
    let mut c = connect(port, &certs).await;
    say(&mut c, json!({"type":"Authenticate","data":{"tenant_id":tenant,"api_key":"x","client_id":"admin"}})).await;
    let r = next_text(&mut c).await;
    assert_eq!(r["data"]["client_id"], "admin", "{r}");

    say(&mut c, json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":4096}})).await;
    let r = next_text(&mut c).await;
    let service_id = r["data"]["service_id"].as_str().unwrap().to_string();

    say(&mut c, json!({"type":"GetServiceFilePath","data":{"request_id":"a","service_id":service_id}})).await;
    let closed = next_text(&mut c).await;
    assert!(closed["data"].get("path").is_none(), "closed by default: {closed}");

    say(&mut c, json!({"type":"SetLocalMapping","data":{"request_id":"s","service_id":service_id,"enabled":true}})).await;
    let ack = next_text(&mut c).await;
    assert_eq!(ack["type"], "LocalMappingAck", "{ack}");

    say(&mut c, json!({"type":"GetServiceFilePath","data":{"request_id":"b","service_id":service_id}})).await;
    let offer = next_text(&mut c).await;
    let path = std::path::PathBuf::from(offer["data"]["path"].as_str().expect("a path is offered"));
    let nonce_path = std::path::PathBuf::from(offer["data"]["nonce_path"].as_str().unwrap());
    assert!(path.is_absolute() && path.exists());
    let nonce = std::fs::read_to_string(&nonce_path).unwrap();
    assert_eq!(nonce.len(), 64);
    let file_id = offer["data"]["file_id"].as_u64().unwrap();

    say(&mut c, json!({"type":"ConfirmLocal","data":{"request_id":"c","file_id":file_id,"nonce":nonce}})).await;
    let done = next_text(&mut c).await;
    assert_eq!(done["type"], "ConfirmLocalAck", "{done}");
    assert!(!nonce_path.exists(), "the nonce file is deleted on first use");

    cleanup(&tenant);
}
