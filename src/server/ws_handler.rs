// SPDX-License-Identifier: MIT
//! WebSocket Connection Handler (RFC 6455)
//!
//! Handles individual WebSocket connections from remote clients.
//! Implements WebSocket protocol (RFC 6455) with binary frames for MessagePack messages.
//! Manages WebSocket frame reading/writing, session lifecycle, and message routing.

use crate::protocol::{ClientSession, ClientState};
use crate::Server;
use futures::SinkExt;
use futures::StreamExt;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio_rustls::server::TlsStream;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// Handles a single WebSocket client connection (RFC 6455)
///
/// Accepts TLS-wrapped TCP stream, performs WebSocket handshake,
/// then manages frame-based protocol communication.
pub async fn handle_connection(
    stream: TlsStream<TcpStream>,
    peer_addr: SocketAddr,
    _server: Arc<RwLock<Server>>,
    sessions: Arc<RwLock<HashMap<String, ClientSession>>>,
    _config: super::WssServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Perform WebSocket handshake (HTTP upgrade to WebSocket)
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed for {}: {}", peer_addr, e);
            return Err(e.into());
        }
    };

    // Create initial client session
    let mut session = ClientSession::new();

    println!(
        "WebSocket connection established from {} (session_id: {})",
        peer_addr, session.session_id
    );

    // Register session
    sessions
        .write()
        .await
        .insert(session.session_id.clone(), session.clone());

    // Split the WebSocket stream into sender and receiver
    let (mut write, mut read) = ws_stream.split();

    // Other connections forward ranges to this one through this queue.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<
        crate::server::forwarding::Outbound,
    >();
    session.outbound = Some(out_tx);

    loop {
        // Wait for the next frame from the client, sending anything forwarded to us meanwhile.
        let next = loop {
            tokio::select! {
                Some(out) = out_rx.recv() => {
                    let sent = match out {
                        crate::server::forwarding::Outbound::Text(v) => {
                            write.send(Message::Text(v.to_string().into())).await
                        }
                        crate::server::forwarding::Outbound::Binary(b) => {
                            write.send(Message::Binary(b.into())).await
                        }
                    };
                    if sent.is_err() {
                        break None;
                    }
                }
                incoming = read.next() => break incoming,
            }
        };
        // Read WebSocket frame (RFC 6455 frame format)
        match next {
            Some(Ok(msg)) => {
                match msg {
                    Message::Binary(data) if data.first() == Some(&commy_wire::KIND_RANGE) => {
                        // A commy-wire range frame (a remote sender's changed bytes). MessagePack
                        // messages are maps, which never start with byte 1.
                        if let Some(err) = handle_range_frame(&data, &mut session, &_server).await {
                            if let Ok(text) = serde_json::to_string(&err) {
                                let _ = write.send(Message::Text(text.into())).await;
                            }
                        }
                    }
                    Message::Binary(_) => {
                        // The only binary frames are range frames (handled above). The older
                        // MessagePack protocol was removed in 0.21.0 (R12): refuse, in JSON.
                        eprintln!("Refused a non-range binary frame from {}", peer_addr);
                        let err = invalid_request(
                            "binary frames must be range frames; the MessagePack protocol was removed",
                        );
                        if let Ok(text) = serde_json::to_string(&err) {
                            let _ = write.send(Message::Text(text.into())).await;
                        }
                    }
                    Message::Text(text) => {
                        // Handle SDK protocol (JSON format) for new CRUD operations
                        // Parse as generic JSON value first to determine type
                        match serde_json::from_str::<serde_json::Value>(&text) {
                            Ok(msg_value) => {
                                println!(
                                    "Received SDK message from {}: {:?}",
                                    peer_addr,
                                    msg_value.get("type")
                                );

                                // Handle SDK message based on type field
                                let reply = handle_sdk_message(
                                    msg_value,
                                    &mut session,
                                    Arc::clone(&_server),
                                )
                                .await;
                                // Binary frames a handler queued (a resync snapshot) go out first,
                                // so the text reply that ends the snapshot arrives last.
                                let frames: Vec<Vec<u8>> =
                                    session.outbound_frames.drain(..).collect();
                                let mut send_failed = false;
                                for frame in frames {
                                    if let Err(e) = write.send(Message::Binary(frame.into())).await {
                                        eprintln!("Failed to send range frame: {}", e);
                                        send_failed = true;
                                        break;
                                    }
                                }
                                if send_failed {
                                    break;
                                }
                                if let Some(response) = reply {
                                    // Send response back as Text (JSON)
                                    if let Ok(serialized) = serde_json::to_string(&response) {
                                        println!(
                                            "[SDK Response] Sending: {}",
                                            &serialized[..std::cmp::min(100, serialized.len())]
                                        );
                                        if let Err(e) = write.send(Message::Text(serialized.into())).await
                                        {
                                            eprintln!("Failed to send response: {}", e);
                                            break;
                                        } else {
                                            println!("[SDK Response] Successfully sent");
                                        }
                                    } else {
                                        eprintln!("[SDK Response] Failed to serialize response");
                                    }
                                }

                                // Update session state
                                session.last_activity = chrono::Utc::now();
                                sessions
                                    .write()
                                    .await
                                    .insert(session.session_id.clone(), session.clone());
                            }
                            Err(_) => {
                                // Not valid JSON, reject
                                eprintln!(
                                    "Received invalid JSON message from {}: {}",
                                    peer_addr, text
                                );
                                let error = invalid_request("Invalid JSON format");
                                if let Ok(serialized) = serde_json::to_string(&error) {
                                    let _ = write.send(Message::Text(serialized.into())).await;
                                }
                            }
                        }
                    }
                    Message::Ping(ping_data) => {
                        // Respond to ping with pong (RFC 6455 keepalive)
                        if let Err(e) = write.send(Message::Pong(ping_data)).await {
                            eprintln!("Failed to send pong: {}", e);
                            break;
                        }
                    }
                    Message::Pong(_) => {
                        // Update activity on pong
                        session.last_activity = chrono::Utc::now();
                        sessions
                            .write()
                            .await
                            .insert(session.session_id.clone(), session.clone());
                    }
                    Message::Close(frame) => {
                        println!("Client {} initiated close: {:?}", session.session_id, frame);
                        break;
                    }
                    Message::Frame(_) => {
                        // Raw frame (should not occur with message-based API)
                        eprintln!("Received raw frame from {} (unexpected)", peer_addr);
                    }
                }
            }
            Some(Err(e)) => {
                eprintln!("WebSocket protocol error for {}: {}", session.session_id, e);
                break;
            }
            None => {
                // Connection closed
                println!("Client {} disconnected", session.session_id);
                break;
            }
        }
    }

    // Send close frame to gracefully close connection
    let _ = write.send(Message::Close(None)).await;

    // Cleanup: leave every file, drop unconfirmed nonces, remove session on disconnect
    _server.read().await.forwarder.leave_session(&session.session_id);
    session.drop_pending_nonces();
    sessions.write().await.remove(&session.session_id);
    println!("Client {} session cleaned up", session.session_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = ClientSession::new();
        assert_eq!(session.state, ClientState::Unauthenticated);
        assert!(session.tenant_id.is_none());
        assert!(session.client_id.is_none());
    }

    #[test]
    fn test_session_authenticated() {
        let mut session = ClientSession::new();
        session.tenant_id = Some("tenant_a".to_string());
        session.state = ClientState::Active;

        assert!(session.tenant_id.is_some());
        assert_eq!(session.state, ClientState::Active);
    }

    #[test]
    fn test_session_subscription() {
        let mut session = ClientSession::new();
        session.subscriptions.insert("service_a/var1".to_string());
        session.subscriptions.insert("service_a/var2".to_string());

        assert_eq!(session.subscriptions.len(), 2);
        assert!(session.subscriptions.contains("service_a/var1"));
    }
}
///
/// Processes new CRUD operations: CreateService, GetService, DeleteService
/// Returns ServerMessage response (as JSON) to send back to client
/// Why `Authenticate` is refused when development authentication is off.
const DEV_AUTH_REFUSED: &str = "Authentication is not available: no credential verifier is configured and development authentication is disabled (COMMY_ALLOW_DEV_AUTH=1 enables it for development only)";

fn error_json(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({ "type": "Error", "data": { "code": code, "message": message } })
}

fn invalid_request(message: &str) -> serde_json::Value {
    error_json("INVALID_REQUEST", message)
}

/// A session may use tenant resources only after it has authenticated.
fn session_is_authenticated(session: &ClientSession) -> bool {
    session.tenant_id.is_some()
        && session.token.is_some()
        && session.permissions.is_some()
        && !matches!(
            session.state,
            ClientState::Unauthenticated | ClientState::Disconnected
        )
}

/// Tenant creation and deletion: the session must hold `TenantAdmin` and may act only on the
/// tenant it is authenticated to.
fn require_own_tenant_admin(session: &ClientSession, tenant_id: &str) -> Option<serde_json::Value> {
    if session.tenant_id.as_deref() != Some(tenant_id) {
        return Some(error_json("PERMISSION_DENIED", "Not authenticated to this tenant"));
    }
    let admin = session
        .permissions
        .as_ref()
        .is_some_and(|p| p.has_permission(&crate::auth::Permission::TenantAdmin));
    if !admin {
        return Some(error_json(
            "PERMISSION_DENIED",
            "Permission denied: tenant admin required",
        ));
    }
    None
}

/// The checks that apply to every JSON message before any arm runs. Returns the refusal, or
/// `None` to let the message through.
///
/// 1. Every `tenant_id` on the wire must parse as a `TenantId`, so a bad id can not reach a path.
/// 2. `Authenticate` is refused unless development authentication is on (nothing else
///    verifies credentials yet).
/// 3. Everything except `Authenticate`, `Heartbeat` and `Disconnect` needs an authenticated
///    session.
/// 4. A message that names a service may use only a service of the session's own tenant.
async fn guard_sdk_message(
    msg_type: &str,
    data: Option<&serde_json::Value>,
    session: &ClientSession,
    server: &Arc<RwLock<Server>>,
) -> Option<serde_json::Value> {
    if let Some(tenant) = data.and_then(|d| d.get("tenant_id")) {
        match tenant.as_str().map(crate::tenant_id::TenantId::parse) {
            Some(Ok(_)) => {}
            Some(Err(e)) => return Some(invalid_request(&e.to_string())),
            None => return Some(invalid_request("tenant_id must be a string")),
        }
    }

    match msg_type {
        "Authenticate" => {
            let g = server.read().await;
            if !g.dev_auth && g.verifier.is_none() {
                return Some(error_json("UNAUTHENTICATED", DEV_AUTH_REFUSED));
            }
            None
        }
        "Heartbeat" | "Disconnect" => None,
        _ => {
            if !session_is_authenticated(session) {
                return Some(error_json("UNAUTHENTICATED", "Not authenticated"));
            }
            if let Some(service_id) = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
            {
                let guard = server.read().await;
                if let Some(record) = guard.service_registry.get(service_id) {
                    if session.tenant_id.as_deref() != Some(record.tenant_name.as_str()) {
                        return Some(error_json(
                            "PERMISSION_DENIED",
                            "Service belongs to another tenant",
                        ));
                    }
                    // Entitled to the service at all: any grant on it will do here; each
                    // handler then checks the action it needs.
                    use crate::server::grants::Action;
                    if !session_allows(&guard, session, service_id, Action::Read)
                        && !session_allows(&guard, session, service_id, Action::Resize)
                    {
                        return Some(error_json(
                            "PERMISSION_DENIED",
                            "No access to this service",
                        ));
                    }
                }
            }
            None
        }
    }
}

/// A reply or error built from the wire crate's own types, so spelling cannot drift.
fn wire_json(msg: &commy_wire::ControlMessage) -> serde_json::Value {
    serde_json::to_value(msg).unwrap_or_else(|_| error_json("INTERNAL_ERROR", "reply failed"))
}

fn wire_error(
    code: commy_wire::ErrorCode,
    message: &str,
    request_id: Option<&str>,
) -> serde_json::Value {
    wire_json(&commy_wire::ControlMessage::Error(commy_wire::ErrorReply {
        code,
        message: message.to_string(),
        request_id: request_id.map(str::to_string),
    }))
}

/// GetServiceFilePath and ConfirmLocal (plan C1). The session is already authenticated and, for a
/// known `service_id`, belongs to the service's tenant (see `guard_sdk_message`).
async fn handle_local_mapping(
    message: serde_json::Value,
    session: &mut ClientSession,
    server: &Arc<RwLock<Server>>,
) -> Option<serde_json::Value> {
    use commy_wire::{ControlMessage, ErrorCode};

    let request: ControlMessage = match serde_json::from_value(message) {
        Ok(m) => m,
        Err(e) => return Some(wire_error(ErrorCode::InvalidRequest, &e.to_string(), None)),
    };
    match request {
        ControlMessage::GetServiceFilePath(req) => {
            let rid = Some(req.request_id.as_str());
            let guard = server.read().await;
            let Some(record) = guard.service_registry.get(&req.service_id) else {
                return Some(wire_error(ErrorCode::NotFound, "Unknown service", rid));
            };
            if session.tenant_id.as_deref() != Some(record.tenant_name.as_str()) {
                return Some(wire_error(
                    ErrorCode::PermissionDenied,
                    "Service belongs to another tenant",
                    rid,
                ));
            }
            use crate::server::grants::Action;
            if !session_allows(&guard, session, &req.service_id, Action::Read) {
                return Some(wire_error(
                    ErrorCode::PermissionDenied,
                    "No access to this service",
                    rid,
                ));
            }
            let can_write = session_allows(&guard, session, &req.service_id, Action::Write);
            let mut reply = commy_wire::ServiceFilePath {
                request_id: req.request_id.clone(),
                service_id: req.service_id.clone(),
                file_id: record.file_id,
                path: None,
                nonce_path: None,
            };
            if record.trusted_local && can_write {
                use crate::server::local_mapping as lm;
                let file_path = std::path::Path::new(&record.file_path);
                let tenant_dir = file_path.parent().unwrap_or(std::path::Path::new("."));
                lm::sweep_stale(tenant_dir, lm::NONCE_TTL);
                if session.pending_nonces.len() >= lm::MAX_PENDING_NONCES
                    && !session.pending_nonces.contains_key(&record.file_id)
                {
                    return Some(wire_error(
                        ErrorCode::RateLimited,
                        "Too many unconfirmed local-mapping requests",
                        rid,
                    ));
                }
                let offered = lm::absolute(file_path).and_then(|path| {
                    let pending = lm::create_nonce_file(tenant_dir)?;
                    let nonce_path = lm::absolute(&pending.path)?;
                    Ok((path, nonce_path, pending))
                });
                match offered {
                    Ok((path, nonce_path, pending)) => {
                        if let Some(old) = session.pending_nonces.insert(record.file_id, pending) {
                            let _ = std::fs::remove_file(old.path);
                        }
                        reply.path = Some(path.to_string_lossy().into_owned());
                        reply.nonce_path = Some(nonce_path.to_string_lossy().into_owned());
                    }
                    // Not being able to offer a local mapping is not an error: stay on WSS.
                    Err(e) => eprintln!("[local-mapping] not offered: {e}"),
                }
            }
            Some(wire_json(&ControlMessage::ServiceFilePath(reply)))
        }
        ControlMessage::ConfirmLocal(req) => {
            let rid = Some(req.request_id.as_str());
            let Some(pending) = session.pending_nonces.remove(&req.file_id) else {
                return Some(wire_error(
                    ErrorCode::InvalidRequest,
                    "No local mapping is pending for this file",
                    rid,
                ));
            };
            // One attempt per nonce, match or not: the file is gone either way.
            let _ = std::fs::remove_file(&pending.path);
            if !crate::server::local_mapping::secrets_match(&pending.nonce, &req.nonce) {
                return Some(wire_error(
                    ErrorCode::PermissionDenied,
                    "Nonce mismatch",
                    rid,
                ));
            }
            session.local_files.insert(req.file_id);
            if let Some(tx) = &session.outbound {
                server.read().await.forwarder.join(
                    req.file_id,
                    &session.session_id,
                    session.client_id.as_deref(),
                    crate::server::forwarding::Mode::Local,
                    tx.clone(),
                );
            }
            Some(wire_json(&ControlMessage::ConfirmLocalAck(
                commy_wire::ConfirmLocalAck {
                    request_id: req.request_id,
                    file_id: req.file_id,
                },
            )))
        }
        ControlMessage::Resync(req) => {
            let rid = Some(req.request_id.as_str());
            let guard = server.read().await;
            if guard.retired_files.contains(&req.file_id) {
                return Some(wire_error(
                    ErrorCode::InvalidRequest,
                    "This file was rotated; fetch the new one",
                    rid,
                ));
            }
            let Some(record) = guard
                .service_registry
                .values()
                .find(|r| r.file_id == req.file_id)
            else {
                return Some(wire_error(ErrorCode::NotFound, "Unknown file", rid));
            };
            let can_read = guard
                .service_registry
                .iter()
                .find(|(_, r)| r.file_id == req.file_id)
                .is_some_and(|(sid, _)| {
                    session_allows(&guard, session, sid, crate::server::grants::Action::Read)
                });
            if session.tenant_id.as_deref() != Some(record.tenant_name.as_str()) || !can_read {
                return Some(wire_error(
                    ErrorCode::PermissionDenied,
                    "No read access to this file",
                    rid,
                ));
            }
            let bytes = match std::fs::read(&record.file_path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[resync] cannot read {}: {e}", record.file_path);
                    return Some(wire_error(
                        ErrorCode::InternalError,
                        "Cannot read the service file",
                        rid,
                    ));
                }
            };
            let seq = record.seq;
            for frame in commy_wire::RangeFrame::split(record.file_id, seq, 0, &bytes) {
                session.outbound_frames.push(frame.encode());
            }
            // From now on this session is a remote mirror of the file and is sent every flush.
            if let Some(tx) = &session.outbound {
                guard.forwarder.join(
                    record.file_id,
                    &session.session_id,
                    session.client_id.as_deref(),
                    crate::server::forwarding::Mode::Remote,
                    tx.clone(),
                );
            }
            Some(wire_json(&ControlMessage::ResyncDone(
                commy_wire::ResyncDone {
                    request_id: req.request_id,
                    file_id: record.file_id,
                    seq,
                    size: bytes.len() as u64,
                },
            )))
        }
        ControlMessage::RangesChanged(req) => {
            let rid = Some(req.request_id.as_str());
            if let Err(why) = commy_wire::validate_ranges(&req.ranges) {
                return Some(wire_error(ErrorCode::InvalidRequest, why, rid));
            }
            let file = match writable_file(session, server, req.file_id, rid).await {
                Ok(f) => f,
                Err(e) => return Some(e),
            };
            if !session.local_files.contains(&req.file_id) {
                return Some(wire_error(
                    ErrorCode::InvalidRequest,
                    "Not a local mapper of this file; remote senders send range frames, then FlushEnd",
                    rid,
                ));
            }
            if !ranges_in_bounds(&req.ranges, file.size) {
                return Some(wire_error(
                    ErrorCode::InvalidRequest,
                    "A range is outside the file",
                    rid,
                ));
            }
            let seq = commit_flush(session, server, req.file_id, &file.path, &req.ranges).await;
            Some(wire_json(&ControlMessage::FlushAck(commy_wire::FlushAck {
                request_id: req.request_id,
                file_id: req.file_id,
                seq,
            })))
        }
        ControlMessage::RotateRequest(req) => Some(handle_rotate(req, session, server).await),
        ControlMessage::SetLocalMapping(req) => {
            let rid = Some(req.request_id.as_str());
            let who = client_key(session);
            {
                let guard = server.read().await;
                let Some(record) = guard.service_registry.get(&req.service_id) else {
                    return Some(wire_error(ErrorCode::NotFound, "Unknown service", rid));
                };
                if session.tenant_id.as_deref() != Some(record.tenant_name.as_str())
                    || !session_allows(
                        &guard,
                        session,
                        &req.service_id,
                        crate::server::grants::Action::Admin,
                    )
                {
                    guard.audit(&format!(
                        "local mapping REFUSED service={} enabled={} by={who}",
                        req.service_id, req.enabled
                    ));
                    return Some(wire_error(
                        ErrorCode::PermissionDenied,
                        "Needs service:admin on this service",
                        rid,
                    ));
                }
            }
            let mut guard = server.write().await;
            if let Err(e) = guard.set_trusted_local_recorded(&req.service_id, req.enabled) {
                guard.audit(&format!(
                    "local mapping REFUSED service={} enabled={} by={who}: cannot record it: {e}",
                    req.service_id, req.enabled
                ));
                return Some(wire_error(
                    ErrorCode::InternalError,
                    "Could not record the change",
                    rid,
                ));
            }
            guard.audit(&format!(
                "local mapping {} service={} by={who}",
                if req.enabled { "OPENED" } else { "CLOSED" },
                req.service_id
            ));
            Some(wire_json(&ControlMessage::LocalMappingAck(
                commy_wire::LocalMappingAck {
                    request_id: req.request_id,
                    service_id: req.service_id,
                    enabled: req.enabled,
                },
            )))
        }
        m @ (ControlMessage::Grant(_) | ControlMessage::Revoke(_) | ControlMessage::ListGrants(_)) => {
            Some(handle_grants(m, session, server).await)
        }
        ControlMessage::RotationDrained(req) => {
            let rid = Some(req.request_id.as_str());
            let guard = server.read().await;
            match guard
                .rotations
                .mark_drained(&req.rotation_id, req.file_id, &session.session_id)
            {
                Ok(()) => None,
                Err(why) => Some(wire_error(ErrorCode::InvalidRequest, why, rid)),
            }
        }
        ControlMessage::RotationFilled(req) => {
            let rid = Some(req.request_id.as_str());
            let guard = server.read().await;
            match guard.rotations.respond(
                &req.rotation_id,
                &session.session_id,
                crate::server::rotation::FillResult::Filled,
            ) {
                Ok(()) => None,
                Err(why) => Some(wire_error(ErrorCode::InvalidRequest, why, rid)),
            }
        }
        ControlMessage::RotationAbort(req) => {
            let rid = Some(req.request_id.as_str());
            let guard = server.read().await;
            match guard.rotations.respond(
                &req.rotation_id,
                &session.session_id,
                crate::server::rotation::FillResult::Aborted,
            ) {
                Ok(()) => None,
                Err(why) => Some(wire_error(ErrorCode::InvalidRequest, why, rid)),
            }
        }
        ControlMessage::FlushEnd(req) => {
            let rid = Some(req.request_id.as_str());
            let file = match writable_file(session, server, req.file_id, rid).await {
                Ok(f) => f,
                Err(e) => return Some(e),
            };
            let pending = session.pending_flush.remove(&req.file_id).unwrap_or_default();
            if pending.is_empty() {
                return Some(wire_error(
                    ErrorCode::InvalidRequest,
                    "No range frames were sent for this flush",
                    rid,
                ));
            }
            let ranges = crate::server::forwarding::merge_ranges(pending);
            let seq = commit_flush(session, server, req.file_id, &file.path, &ranges).await;
            Some(wire_json(&ControlMessage::FlushAck(commy_wire::FlushAck {
                request_id: req.request_id,
                file_id: req.file_id,
                seq,
            })))
        }
        _ => Some(wire_error(
            ErrorCode::InvalidRequest,
            "Not a local-mapping, resync or flush request",
            None,
        )),
    }
}

/// Most merged ranges one remote flush may accumulate before `FlushEnd`; keeps a fragmented writer
/// from growing the pending list without bound.
const MAX_PENDING_RANGES: usize = 65_536;

/// The identity a session's grants are filed under: its client id, or its session id when it has
/// none.
fn client_key(session: &ClientSession) -> String {
    session
        .client_id
        .clone()
        .unwrap_or_else(|| session.session_id.clone())
}

/// Import the session's authentication permissions into the grant store, once. Done lazily so that
/// permissions assigned at authentication are what the first check sees.
fn ensure_imported(guard: &Server, session: &ClientSession) {
    use std::sync::atomic::Ordering;
    if session.grants_imported.swap(true, Ordering::AcqRel) {
        return;
    }
    if let (Some(tenant), Some(perms)) = (session.tenant_id.as_deref(), session.permissions.as_ref()) {
        if let Err(e) = guard.grants.import_session(tenant, &client_key(session), perms) {
            eprintln!("[grants] cannot import session permissions: {e}");
        }
    }
}

/// May the session do `action` to the service? Every service-scoped check goes through here.
fn session_allows(
    guard: &Server,
    session: &ClientSession,
    service_id: &str,
    action: crate::server::grants::Action,
) -> bool {
    let Some(tenant) = session.tenant_id.as_deref() else {
        return false;
    };
    ensure_imported(guard, session);
    guard
        .grants
        .allows(tenant, &client_key(session), service_id, action)
}

/// A file the session may write, as the flush handlers need it.
struct WritableFile {
    path: String,
    size: u64,
}

/// Find the file, check it is in the session's tenant (another tenant's file is reported as unknown)
/// and that the session holds a write grant.
async fn writable_file(
    session: &ClientSession,
    server: &Arc<RwLock<Server>>,
    file_id: u32,
    rid: Option<&str>,
) -> Result<WritableFile, serde_json::Value> {
    use commy_wire::ErrorCode;
    let guard = server.read().await;
    if let Some(p) = guard.prepared.get(&file_id) {
        // A file being prepared for a resize is visible to the requester alone.
        if p.requester_session != session.session_id {
            return Err(wire_error(ErrorCode::NotFound, "Unknown file", rid));
        }
        return Ok(WritableFile {
            path: p.path.clone(),
            size: p.size,
        });
    }
    if guard.retired_files.contains(&file_id) {
        return Err(wire_error(
            ErrorCode::InvalidRequest,
            "This file was rotated; fetch the new one",
            rid,
        ));
    }
    let (service_id, record) = guard
        .service_registry
        .iter()
        .find(|(_, r)| r.file_id == file_id)
        .filter(|(_, r)| session.tenant_id.as_deref() == Some(r.tenant_name.as_str()))
        .ok_or_else(|| wire_error(ErrorCode::NotFound, "Unknown file", rid))?;
    let can_write = session_allows(&guard, session, service_id, crate::server::grants::Action::Write);
    if !can_write {
        return Err(wire_error(
            ErrorCode::PermissionDenied,
            "No write access to this file",
            rid,
        ));
    }
    let size = std::fs::metadata(&record.file_path)
        .map(|m| m.len())
        .map_err(|_| wire_error(ErrorCode::InternalError, "Cannot read the service file", rid))?;
    Ok(WritableFile {
        path: record.file_path.clone(),
        size,
    })
}

fn ranges_in_bounds(ranges: &[commy_wire::ByteRange], size: u64) -> bool {
    ranges
        .iter()
        .all(|r| r.offset.checked_add(r.len).is_some_and(|end| end <= size))
}

/// Give a flush its sequence number and forward it to every other member of the file: local members
/// get the ranges, remote members get the ranges plus the bytes read now. Returns the new `seq`.
async fn commit_flush(
    session: &ClientSession,
    server: &Arc<RwLock<Server>>,
    file_id: u32,
    path: &str,
    ranges: &[commy_wire::ByteRange],
) -> u64 {
    use crate::server::forwarding::{read_frames, Mode, Outbound};
    let (seq, receivers) = {
        let mut guard = server.write().await;
        let seq = match guard
            .service_registry
            .values_mut()
            .find(|r| r.file_id == file_id)
        {
            Some(record) => {
                record.seq += 1;
                record.seq
            }
            None => 0,
        };
        (seq, guard.forwarder.receivers(file_id, &session.session_id))
    };
    if receivers.is_empty() {
        return seq;
    }
    let frames = if receivers.iter().any(|(m, _)| *m == Mode::Remote) {
        match read_frames(path, file_id, seq, ranges) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("[forward] cannot read {path}: {e}");
                None
            }
        }
    } else {
        None
    };
    for (mode, tx) in receivers {
        match mode {
            Mode::Local => {
                for chunk in ranges.chunks(commy_wire::MAX_RANGES_PER_MESSAGE) {
                    let msg = commy_wire::ControlMessage::RangesUpdated(commy_wire::RangesUpdated {
                        file_id,
                        seq,
                        ranges: chunk.to_vec(),
                    });
                    let _ = tx.send(Outbound::Text(wire_json(&msg)));
                }
            }
            Mode::Remote => {
                if let Some(frames) = &frames {
                    for f in frames {
                        let _ = tx.send(Outbound::Binary(f.clone()));
                    }
                }
            }
        }
    }
    seq
}

/// One binary range frame from a remote sender: write its bytes into the file now and remember the
/// range. The flush is forwarded, and acknowledged, at `FlushEnd`. Returns an error reply, or
/// `None` when the frame was accepted.
pub(crate) async fn handle_range_frame(
    bytes: &[u8],
    session: &mut ClientSession,
    server: &Arc<RwLock<Server>>,
) -> Option<serde_json::Value> {
    use commy_wire::ErrorCode;
    if !session_is_authenticated(session) {
        return Some(wire_error(ErrorCode::Unauthenticated, "Not authenticated", None));
    }
    let frame = match commy_wire::RangeFrame::decode(bytes) {
        Ok(f) => f,
        Err(e) => {
            return Some(wire_error(
                ErrorCode::InvalidRequest,
                &format!("bad range frame: {e:?}"),
                None,
            ))
        }
    };
    let file = match writable_file(session, server, frame.file_id, None).await {
        Ok(f) => f,
        Err(e) => return Some(e),
    };
    if session.local_files.contains(&frame.file_id) {
        return Some(wire_error(
            ErrorCode::InvalidRequest,
            "A local mapper reports changes with RangesChanged, not range frames",
            None,
        ));
    }
    let range = commy_wire::ByteRange {
        offset: frame.offset,
        len: frame.data.len() as u64,
    };
    if !ranges_in_bounds(std::slice::from_ref(&range), file.size) {
        return Some(wire_error(
            ErrorCode::InvalidRequest,
            "Range is outside the file",
            None,
        ));
    }
    let pending = session.pending_flush.entry(frame.file_id).or_default();
    if pending.len() >= MAX_PENDING_RANGES {
        return Some(wire_error(
            ErrorCode::InvalidRequest,
            "Too many ranges in one flush; send FlushEnd",
            None,
        ));
    }
    let wrote = (|| -> std::io::Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(&file.path)?;
        f.seek(SeekFrom::Start(frame.offset))?;
        f.write_all(&frame.data)
    })();
    if let Err(e) = wrote {
        eprintln!("[forward] cannot write {}: {e}", file.path);
        return Some(wire_error(
            ErrorCode::InternalError,
            "Cannot write the service file",
            None,
        ));
    }
    pending.push(range);
    *pending = crate::server::forwarding::merge_ranges(std::mem::take(pending));
    None
}

/// A refused rotation request: audited, then answered with the wire error.
fn rotation_refused(
    guard: &Server,
    who: &str,
    req: &commy_wire::RotateRequest,
    code: commy_wire::ErrorCode,
    why: &str,
) -> serde_json::Value {
    guard.audit(&format!(
        "rotation REFUSED service={} by={} reason={:?}: {}",
        req.service_id, who, req.reason, why
    ));
    wire_error(code, why, Some(req.request_id.as_str()))
}

/// `RotateRequest` (plan C4): check, start, answer `RotationAccepted`; the rotation itself runs in a
/// background task (`run_rotation`) so the connection keeps reading while SDKs drain.
async fn handle_rotate(
    req: commy_wire::RotateRequest,
    session: &ClientSession,
    server: &Arc<RwLock<Server>>,
) -> serde_json::Value {
    use commy_wire::{ControlMessage, ErrorCode, RotateReason};
    let who = session
        .client_id
        .clone()
        .unwrap_or_else(|| session.session_id.clone());
    let guard = server.read().await;
    guard.audit(&format!(
        "rotation REQUESTED service={} by={} reason={:?}",
        req.service_id, who, req.reason
    ));
    if let Err(why) = req.validate() {
        return rotation_refused(&guard, &who, &req, ErrorCode::InvalidRequest, why);
    }
    let Some(record) = guard.service_registry.get(&req.service_id) else {
        return rotation_refused(&guard, &who, &req, ErrorCode::NotFound, "Unknown service");
    };
    if session.tenant_id.as_deref() != Some(record.tenant_name.as_str()) {
        return rotation_refused(
            &guard,
            &who,
            &req,
            ErrorCode::PermissionDenied,
            "Service belongs to another tenant",
        );
    }
    use crate::server::grants::Action;
    let needed = match req.reason {
        RotateReason::Resize => Action::Resize,
        _ => Action::Admin,
    };
    if !session_allows(&guard, session, &req.service_id, needed) {
        return rotation_refused(
            &guard,
            &who,
            &req,
            ErrorCode::PermissionDenied,
            "Not permitted to rotate this service",
        );
    }
    let file_id = record.file_id;
    let members = guard.forwarder.members(file_id);
    let new_size = match req.reason {
        RotateReason::Resize => {
            match crate::service_file_size(req.new_size.unwrap_or(0), guard.max_service_size) {
                Ok(n) => Some(n),
                Err(crate::ServiceSizeError::Zero) => {
                    return rotation_refused(
                        &guard,
                        &who,
                        &req,
                        ErrorCode::InvalidRequest,
                        "new_size must be at least 1 byte",
                    )
                }
                Err(_) => {
                    return rotation_refused(
                        &guard,
                        &who,
                        &req,
                        ErrorCode::InvalidRequest,
                        &format!(
                            "new_size exceeds the server maximum of {} bytes",
                            guard.max_service_size
                        ),
                    )
                }
            }
        }
        _ => {
            let target = req.revoke_client_id.as_deref().unwrap_or("");
            if !members
                .iter()
                .any(|m| m.client_id.as_deref() == Some(target))
            {
                return rotation_refused(
                    &guard,
                    &who,
                    &req,
                    ErrorCode::NotFound,
                    "That client is not on this service",
                );
            }
            None
        }
    };
    let expected = match req.reason {
        RotateReason::Resize => members.iter().map(|m| m.session_id.clone()).collect(),
        _ => std::collections::HashSet::new(),
    };
    let active = match guard.rotations.begin(
        &req.service_id,
        &who,
        file_id,
        expected,
        guard.rotation_min_interval,
        std::time::Instant::now(),
    ) {
        Ok(a) => a,
        Err(why) => {
            let text = match why {
                crate::server::rotation::Refusal::InProgress => "A rotation is already running",
                crate::server::rotation::Refusal::TooSoon => "This service rotated too recently",
                crate::server::rotation::Refusal::RequesterLimit => {
                    "Too many rotation requests from this client"
                }
            };
            return rotation_refused(&guard, &who, &req, ErrorCode::RateLimited, text);
        }
    };
    guard
        .rotations
        .set_requester(&req.service_id, &session.session_id, session.outbound.clone());
    let mut active = active;
    active.requester_session = Some(session.session_id.clone());
    active.requester_tx = session.outbound.clone();
    guard.audit(&format!(
        "rotation GRANTED rotation_id={} service={} by={} reason={:?}",
        active.rotation_id, req.service_id, who, req.reason
    ));
    drop(guard);
    let reply = commy_wire::RotationAccepted {
        request_id: req.request_id.clone(),
        rotation_id: active.rotation_id.clone(),
    };
    tokio::spawn(run_rotation(
        Arc::clone(server),
        req.service_id.clone(),
        active,
        req.reason,
        new_size,
        req.revoke_client_id.clone(),
    ));
    wire_json(&ControlMessage::RotationAccepted(reply))
}

/// Run a rotation to its end. RESIZE: drain barrier, then prepare a zeroed new file for the requester
/// to fill (plan Q11(b)), then commit or abort. REVOCATION: build a copy and commit at once.
/// Stragglers get a `Loss` at a commit; an abort drops no one.
async fn run_rotation(
    server: Arc<RwLock<Server>>,
    service_id: String,
    active: crate::server::rotation::Active,
    reason: commy_wire::RotateReason,
    new_size: Option<u64>,
    revoke_client_id: Option<String>,
) {
    use crate::server::forwarding::{Mode, Outbound};
    use crate::server::rotation::FillResult;
    use commy_wire::{AbortReason, ControlMessage, LossKind, RotateReason};

    let (drain_timeout, fill_timeout, members) = {
        let g = server.read().await;
        (g.drain_timeout, g.fill_timeout, g.forwarder.members(active.file_id))
    };
    let requester_session = active.requester_session.clone().unwrap_or_default();
    let requester_tx = active.requester_tx.clone();
    let send = |tx: &crate::server::forwarding::OutTx, msg: &ControlMessage| {
        let _ = tx.send(Outbound::Text(wire_json(msg)));
    };

    // Phase 1 (resize): the drain barrier.
    let mut prepared_file: Option<(u32, String, u64)> = None;
    if reason == RotateReason::Resize {
        let pending = ControlMessage::RotationPending(commy_wire::RotationPending {
            rotation_id: active.rotation_id.clone(),
            service_id: service_id.clone(),
            file_id: active.file_id,
            deadline_ms: drain_timeout.as_millis() as u64,
            fill_ms: fill_timeout.as_millis() as u64,
        });
        for m in &members {
            send(&m.tx, &pending);
        }
        let deadline = std::time::Instant::now() + drain_timeout;
        loop {
            let done = server
                .read()
                .await
                .rotations
                .active(&service_id)
                .is_none_or(|a| a.all_drained());
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if done || left.is_zero() {
                break;
            }
            let _ = tokio::time::timeout(left, active.notify.notified()).await;
        }

        // Phase 2: prepare the new file and give it to the requester only.
        let abort_reason = match prepare_file(&server, &service_id, &requester_session, new_size).await {
            Ok((new_file_id, path, size)) => {
                prepared_file = Some((new_file_id, path.clone(), size));
                let requester_is_local = members
                    .iter()
                    .any(|m| m.session_id == requester_session && m.mode == Mode::Local);
                let offered = if requester_is_local {
                    crate::server::local_mapping::absolute(std::path::Path::new(&path))
                        .ok()
                        .map(|p| p.to_string_lossy().into_owned())
                } else {
                    None
                };
                let msg = ControlMessage::RotationPrepared(commy_wire::RotationPrepared {
                    rotation_id: active.rotation_id.clone(),
                    service_id: service_id.clone(),
                    old_file_id: active.file_id,
                    new_file_id,
                    size,
                    path: offered,
                });
                match &requester_tx {
                    Some(tx) => {
                        server.read().await.rotations.set_filling(&service_id);
                        send(tx, &msg);
                        // Phase 3: wait for the requester to fill it.
                        let deadline = std::time::Instant::now() + fill_timeout;
                        let result = loop {
                            let r = server
                                .read()
                                .await
                                .rotations
                                .active(&service_id)
                                .and_then(|a| a.fill_result);
                            let left = deadline.saturating_duration_since(std::time::Instant::now());
                            if r.is_some() || left.is_zero() {
                                break r;
                            }
                            let _ = tokio::time::timeout(left, active.notify.notified()).await;
                        };
                        match result {
                            Some(FillResult::Filled) => None,
                            Some(FillResult::Aborted) => Some(AbortReason::RequesterAbort),
                            None => Some(AbortReason::FillTimeout),
                        }
                    }
                    None => Some(AbortReason::ServerError),
                }
            }
            Err(e) => {
                eprintln!("[rotation] {service_id}: cannot prepare the new file: {e}");
                Some(AbortReason::ServerError)
            }
        };
        if let Some(why) = abort_reason {
            abort_rotation(&server, &service_id, &active, prepared_file.take(), why, &members, requester_tx.as_ref())
                .await;
            return;
        }
    }

    let drained = server
        .read()
        .await
        .rotations
        .active(&service_id)
        .map(|a| a.drained)
        .unwrap_or_default();

    // Commit: make the new file live.
    let outcome = {
        let mut g = server.write().await;
        let result = match prepared_file.take() {
            Some((new_file_id, path, size)) => switch_to_prepared(&mut g, &service_id, new_file_id, &path, size),
            None => switch_to_copy(&mut g, &service_id),
        };
        // The rotation's final state is not needed once the result is in hand; ending it is idempotent.
        let _ = g.rotations.finish(&service_id);
        match &result {
            Ok(sw) => g.audit(&format!(
                "rotation COMMITTED rotation_id={} service={} old_file={} new_file={} size={}",
                active.rotation_id, service_id, sw.old_file_id, sw.new_file_id, sw.size
            )),
            Err(e) => g.audit(&format!(
                "rotation FAILED rotation_id={} service={}: {e}",
                active.rotation_id, service_id
            )),
        }
        result.map(|sw| {
            let members = g.forwarder.take_file(sw.old_file_id);
            (sw, members)
        })
    };
    let Ok((sw, members)) = outcome else {
        eprintln!("[rotation] {service_id}: could not switch to the new file");
        return;
    };

    let mut told: std::collections::HashSet<String> = std::collections::HashSet::new();
    for m in members {
        told.insert(m.session_id.clone());
        let lost = match reason {
            RotateReason::Resize if !drained.contains(&m.session_id) => {
                Some(LossKind::RotationStraggler)
            }
            RotateReason::Revocation
                if m.client_id.is_some() && m.client_id == revoke_client_id =>
            {
                Some(LossKind::Revoked)
            }
            _ => None,
        };
        let msg = match lost {
            Some(kind) => ControlMessage::Loss(commy_wire::Loss {
                service_id: service_id.clone(),
                file_id: sw.old_file_id,
                kind,
                ranges: Vec::new(),
            }),
            None => commit_message(&active.rotation_id, &service_id, reason, &sw),
        };
        send(&m.tx, &msg);
    }
    // The requester may not be a member of the file (an admin tool): it still hears the outcome.
    if let Some(tx) = &requester_tx {
        if !told.contains(&requester_session) {
            send(tx, &commit_message(&active.rotation_id, &service_id, reason, &sw));
        }
    }
    // Best effort: the old file may still be mapped by a local SDK (Windows refuses to delete it).
    let _ = std::fs::remove_file(&sw.old_path);
}

fn commit_message(
    rotation_id: &str,
    service_id: &str,
    reason: commy_wire::RotateReason,
    sw: &Switch,
) -> commy_wire::ControlMessage {
    commy_wire::ControlMessage::RotationCommit(commy_wire::RotationCommit {
        rotation_id: rotation_id.to_string(),
        service_id: service_id.to_string(),
        reason,
        old_file_id: sw.old_file_id,
        new_file_id: sw.new_file_id,
        size: sw.size,
    })
}

/// Abandon a rotation: drop the prepared file, keep the old one live, tell every member and the
/// requester. Nobody is dropped and nothing is lost.
async fn abort_rotation(
    server: &Arc<RwLock<Server>>,
    service_id: &str,
    active: &crate::server::rotation::Active,
    prepared: Option<(u32, String, u64)>,
    why: commy_wire::AbortReason,
    members: &[crate::server::forwarding::MemberInfo],
    requester_tx: Option<&crate::server::forwarding::OutTx>,
) {
    use crate::server::forwarding::Outbound;
    {
        let mut g = server.write().await;
        if let Some((file_id, path, _)) = prepared {
            g.prepared.remove(&file_id);
            let _ = std::fs::remove_file(path);
        }
        // The rotation's final state is not needed here; ending it is idempotent.
        let _ = g.rotations.finish(service_id);
        g.audit(&format!(
            "rotation ABORTED rotation_id={} service={} reason={:?}",
            active.rotation_id, service_id, why
        ));
    }
    let msg = commy_wire::ControlMessage::RotationAborted(commy_wire::RotationAborted {
        rotation_id: active.rotation_id.clone(),
        service_id: service_id.to_string(),
        file_id: active.file_id,
        reason: why,
    });
    let json = wire_json(&msg);
    for m in members {
        let _ = m.tx.send(Outbound::Text(json.clone()));
    }
    if let Some(tx) = requester_tx {
        if !members
            .iter()
            .any(|m| Some(&m.session_id) == active.requester_session.as_ref())
        {
            let _ = tx.send(Outbound::Text(json));
        }
    }
}

/// What a rotation changed.
struct Switch {
    old_file_id: u32,
    new_file_id: u32,
    size: u64,
    old_path: String,
}

/// Create the zero-filled, not-yet-live file of `new_size` bytes for a resize and record it as
/// prepared for the requester. Returns (new `file_id`, path, size).
async fn prepare_file(
    server: &Arc<RwLock<Server>>,
    service_id: &str,
    requester_session: &str,
    new_size: Option<u64>,
) -> Result<(u32, String, u64), String> {
    let mut g = server.write().await;
    let old_path = g
        .service_registry
        .get(service_id)
        .ok_or("service vanished")?
        .file_path
        .clone();
    let size = new_size.ok_or("resize without a size")?;
    let dir = std::path::Path::new(&old_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string());
    let path = format!("{dir}/service_{}.mem", uuid::Uuid::new_v4());
    std::fs::write(&path, vec![0u8; size as usize]).map_err(|e| e.to_string())?;
    let file_id = g.next_file_id;
    g.next_file_id = file_id.wrapping_add(1).max(1);
    g.prepared.insert(
        file_id,
        crate::PreparedFile {
            service_id: service_id.to_string(),
            requester_session: requester_session.to_string(),
            path: path.clone(),
            size,
        },
    );
    Ok((file_id, path, size))
}

/// Make the prepared file the service's file. Old-offset writes are never replayed.
fn switch_to_prepared(
    g: &mut Server,
    service_id: &str,
    new_file_id: u32,
    new_path: &str,
    size: u64,
) -> Result<Switch, String> {
    let (old_path, old_file_id, tenant_name, service_name) = {
        let r = g.service_registry.get(service_id).ok_or("service vanished")?;
        (r.file_path.clone(), r.file_id, r.tenant_name.clone(), r.service_name.clone())
    };
    let tenant = g.tenants.get_mut(&tenant_name).ok_or("tenant vanished")?;
    tenant
        .register_service(&service_name, new_path, size as usize)
        .map_err(|e| e.to_string())?;
    // Record the new file before the switch counts: a restart must find the file the clients use.
    if let Some(mut row) = g.row_of(service_id) {
        row.path = new_path.to_string();
        row.file_id = new_file_id;
        g.persist_row(service_id, &row).map_err(|e| e.to_string())?;
    }
    {
        let r = g.service_registry.get_mut(service_id).ok_or("service vanished")?;
        r.file_path = new_path.to_string();
        r.file_id = new_file_id;
        r.seq = 0;
    }
    g.prepared.remove(&new_file_id);
    g.retired_files.insert(old_file_id);
    Ok(Switch { old_file_id, new_file_id, size, old_path })
}

/// Revocation: build a same-size copy that keeps the old bytes, and switch to it.
fn switch_to_copy(g: &mut Server, service_id: &str) -> Result<Switch, String> {
    let old_path = g
        .service_registry
        .get(service_id)
        .ok_or("service vanished")?
        .file_path
        .clone();
    let data = std::fs::read(&old_path).map_err(|e| e.to_string())?;
    let size = data.len() as u64;
    let dir = std::path::Path::new(&old_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string());
    let new_path = format!("{dir}/service_{}.mem", uuid::Uuid::new_v4());
    std::fs::write(&new_path, &data).map_err(|e| e.to_string())?;
    let new_file_id = g.next_file_id;
    g.next_file_id = new_file_id.wrapping_add(1).max(1);
    switch_to_prepared(g, service_id, new_file_id, &new_path, size)
}

/// Why a grant message was refused: audited, then answered with the wire error.
fn grant_refused(
    guard: &Server,
    who: &str,
    what: &str,
    code: commy_wire::ErrorCode,
    why: &str,
    rid: &str,
) -> serde_json::Value {
    guard.audit(&format!("grant REFUSED {what} by={who}: {why}"));
    wire_error(code, why, Some(rid))
}

/// A grant error that is the server's fault (it could not record the change) is an internal error;
/// anything else is the caller's.
fn grant_error_code(e: &crate::server::grants::GrantError) -> commy_wire::ErrorCode {
    match e {
        crate::server::grants::GrantError::Backend(_) => commy_wire::ErrorCode::InternalError,
        _ => commy_wire::ErrorCode::InvalidRequest,
    }
}

/// Parse wire permission names into grant specs.
fn parse_specs(names: &[String]) -> Result<Vec<crate::server::grants::Spec>, String> {
    names
        .iter()
        .map(|n| crate::server::grants::parse_permission(n).map_err(|e| e.to_string()))
        .collect()
}

/// Revoking access moves the file, so the revoked client's knowledge of its path is worthless and it
/// is dropped from the file with `Loss{REVOKED}`. Access must end now, so this is not subject to the
/// rotation rate limits; it waits out a rotation that is already running.
fn spawn_revocation_rotation(
    server: Arc<RwLock<Server>>,
    service_id: String,
    client_id: String,
    who: String,
) {
    tokio::spawn(async move {
        for _ in 0..300 {
            let started = {
                let g = server.read().await;
                let Some(record) = g.service_registry.get(&service_id) else {
                    return;
                };
                match g.rotations.begin_forced(
                    &service_id,
                    &who,
                    record.file_id,
                    std::collections::HashSet::new(),
                    std::time::Instant::now(),
                ) {
                    Ok(active) => {
                        g.audit(&format!(
                            "rotation GRANTED rotation_id={} service={} by={} reason=Revocation (access revoked from {})",
                            active.rotation_id, service_id, who, client_id
                        ));
                        Some(active)
                    }
                    Err(_) => None,
                }
            };
            if let Some(active) = started {
                run_rotation(
                    server,
                    service_id,
                    active,
                    commy_wire::RotateReason::Revocation,
                    None,
                    Some(client_id),
                )
                .await;
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        server.read().await.audit(&format!(
            "rotation FAILED service={service_id}: another rotation never finished; access revoked from {client_id} but the file was not moved"
        ));
    });
}

/// `Grant`, `Revoke` and `ListGrants` (plan C10).
async fn handle_grants(
    request: commy_wire::ControlMessage,
    session: &ClientSession,
    server: &Arc<RwLock<Server>>,
) -> serde_json::Value {
    use crate::server::grants::{Action, Spec};
    use commy_wire::{ControlMessage, ErrorCode};

    let who = client_key(session);
    let Some(tenant) = session.tenant_id.clone() else {
        return wire_error(ErrorCode::Unauthenticated, "Not authenticated", None);
    };
    match request {
        ControlMessage::Grant(req) => {
            let rid = req.request_id.clone();
            let what = format!("Grant client={} service={:?} {:?}", req.client_id, req.service_id, req.permissions);
            let guard = server.read().await;
            if let Err(why) = req.validate() {
                return grant_refused(&guard, &who, &what, ErrorCode::InvalidRequest, why, &rid);
            }
            let specs = match parse_specs(&req.permissions) {
                Ok(s) => s,
                Err(e) => return grant_refused(&guard, &who, &what, ErrorCode::InvalidRequest, &e, &rid),
            };
            ensure_imported(&guard, session);
            if let Err(e) = authorise_grant_change(&guard, session, &tenant, &who, req.service_id.as_deref(), &specs) {
                return grant_refused(&guard, &who, &what, e.0, e.1, &rid);
            }
            if let Err(e) = guard.grant(&tenant, &req.client_id, req.service_id.as_deref(), &specs) {
                return grant_refused(&guard, &who, &what, grant_error_code(&e), &e.to_string(), &rid);
            }
            guard.audit(&format!("grant GRANTED {what} by={who}"));
            wire_json(&ControlMessage::GrantAck(commy_wire::GrantAck { request_id: rid }))
        }
        ControlMessage::Revoke(req) => {
            let rid = req.request_id.clone();
            let what = format!("Revoke client={} service={:?} {:?}", req.client_id, req.service_id, req.permissions);
            let guard = server.read().await;
            if let Err(why) = req.validate() {
                return grant_refused(&guard, &who, &what, ErrorCode::InvalidRequest, why, &rid);
            }
            let specs = match parse_specs(&req.permissions) {
                Ok(s) => s,
                Err(e) => return grant_refused(&guard, &who, &what, ErrorCode::InvalidRequest, &e, &rid),
            };
            ensure_imported(&guard, session);
            if let Err(e) = authorise_grant_change(&guard, session, &tenant, &who, req.service_id.as_deref(), &specs) {
                return grant_refused(&guard, &who, &what, e.0, e.1, &rid);
            }
            // Which of this tenant's services could the client reach before?
            let reach = |g: &Server| -> Vec<String> {
                g.service_registry
                    .iter()
                    .filter(|(_, r)| r.tenant_name == tenant)
                    .filter(|(id, _)| {
                        g.grants.allows(&tenant, &req.client_id, id, Action::Read)
                            || g.grants.allows(&tenant, &req.client_id, id, Action::Resize)
                    })
                    .map(|(id, _)| id.clone())
                    .collect()
            };
            let before = reach(&guard);
            if let Err(e) = guard.revoke(&tenant, &req.client_id, req.service_id.as_deref(), &specs) {
                return grant_refused(&guard, &who, &what, grant_error_code(&e), &e.to_string(), &rid);
            }
            let after = reach(&guard);
            guard.audit(&format!("grant REVOKED {what} by={who}"));
            // Every service the client lost access to moves to a new file.
            let lost: Vec<String> = before.into_iter().filter(|id| !after.contains(id)).collect();
            drop(guard);
            for service_id in lost {
                spawn_revocation_rotation(Arc::clone(server), service_id, req.client_id.clone(), who.clone());
            }
            wire_json(&ControlMessage::GrantAck(commy_wire::GrantAck { request_id: rid }))
        }
        ControlMessage::ListGrants(req) => {
            let rid = req.request_id.clone();
            let guard = server.read().await;
            ensure_imported(&guard, session);
            if let Some(sid) = &req.service_id {
                match guard.service_registry.get(sid) {
                    Some(r) if r.tenant_name == tenant => {}
                    _ => return wire_error(ErrorCode::NotFound, "Unknown service", Some(&rid)),
                }
            }
            let tenant_admin = guard.grants.is_tenant_admin(&tenant, &who);
            let service_admin = req
                .service_id
                .as_deref()
                .is_some_and(|sid| guard.grants.allows(&tenant, &who, sid, Action::Admin));
            // A tenant admin sees everything, a service admin that service, anyone else their own.
            let see_all = tenant_admin || service_admin;
            let client_filter = if see_all { req.client_id.as_deref() } else { Some(who.as_str()) };
            if !see_all && req.client_id.as_deref().is_some_and(|c| c != who) {
                return wire_error(ErrorCode::PermissionDenied, "You may list only your own grants", Some(&rid));
            }
            let entries = guard
                .grants
                .list(&tenant, client_filter, req.service_id.as_deref())
                .into_iter()
                .map(|e| commy_wire::GrantEntry {
                    client_id: e.client_id,
                    service_id: e.service_id,
                    permissions: e.permissions,
                })
                .collect();
            let _ = Spec::TenantAdmin;
            wire_json(&ControlMessage::Grants(commy_wire::Grants { request_id: rid, entries }))
        }
        _ => wire_error(ErrorCode::InvalidRequest, "Not a grant request", None),
    }
}

/// May the caller change grants at this scope? Service scope needs `service:admin` on that service;
/// tenant scope needs tenant admin; handing out `tenant:admin` needs tenant admin.
fn authorise_grant_change(
    guard: &Server,
    session: &ClientSession,
    tenant: &str,
    who: &str,
    service: Option<&str>,
    specs: &[crate::server::grants::Spec],
) -> Result<(), (commy_wire::ErrorCode, &'static str)> {
    use commy_wire::ErrorCode;
    let tenant_admin = guard.grants.is_tenant_admin(tenant, who);
    match service {
        Some(sid) => {
            match guard.service_registry.get(sid) {
                Some(r) if r.tenant_name == tenant => {}
                _ => return Err((ErrorCode::NotFound, "Unknown service")),
            }
            if !session_allows(guard, session, sid, crate::server::grants::Action::Admin) {
                return Err((ErrorCode::PermissionDenied, "Needs service:admin on this service"));
            }
        }
        None => {
            if !tenant_admin {
                return Err((ErrorCode::PermissionDenied, "Tenant-wide grants need tenant:admin"));
            }
        }
    }
    if specs.iter().any(|s| matches!(s, crate::server::grants::Spec::TenantAdmin)) && !tenant_admin {
        return Err((ErrorCode::PermissionDenied, "Handing out tenant:admin needs tenant:admin"));
    }
    Ok(())
}

/// Handles one SDK-protocol message. An error reply, or the reply to a service or tenant request,
/// echoes the request's `request_id` when it had one, so a client can match the reply to the request (`commy-wire`: every request carries a
/// `request_id` that the reply echoes).
async fn handle_sdk_message(
    message: serde_json::Value,
    session: &mut ClientSession,
    server: Arc<RwLock<Server>>,
) -> Option<serde_json::Value> {
    let request_id = message
        .get("data")
        .and_then(|d| d.get("request_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let mut reply = handle_sdk_message_inner(message, session, server).await;
    if let (Some(rid), Some(r)) = (request_id, reply.as_mut()) {
        // Errors and the legacy replies to the CRUD requests (a `Result` used to carry a fresh random
        // id, which nothing can match) echo the request's id.
        let echoes = matches!(
            r.get("type").and_then(|t| t.as_str()),
            Some("Error" | "Result" | "Service" | "TenantResult")
        );
        if echoes {
            if let Some(data) = r.get_mut("data").and_then(|d| d.as_object_mut()) {
                data.insert("request_id".to_string(), serde_json::Value::String(rid));
            }
        }
    }
    reply
}

async fn handle_sdk_message_inner(
    message: serde_json::Value,
    session: &mut ClientSession,
    server: Arc<RwLock<Server>>,
) -> Option<serde_json::Value> {
    let msg_type = message
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");
    let data = message.get("data");

    // One guard for every message, so a new arm cannot forget it. Fail closed.
    if let Some(refusal) = guard_sdk_message(msg_type, data, session, &server).await {
        return Some(refusal);
    }

    match msg_type {
        "Authenticate" => {
            // The request is the commy-wire type. A malformed one is answered without echoing any
            // of its content (it may hold a secret): serde's own messages quote the offending value.
            let request: commy_wire::Authenticate = match data
                .cloned()
                .and_then(|d| serde_json::from_value(d).ok())
            {
                Some(r) => r,
                None => {
                    return Some(invalid_request(
                        "malformed Authenticate request (see commy-wire for the fields)",
                    ))
                }
            };
            let rid = request.request_id.clone();
            if let Err(why) = request.validate() {
                return Some(wire_error(commy_wire::ErrorCode::InvalidRequest, why, Some(&rid)));
            }
            if request.protocol_version != commy_wire::PROTOCOL_VERSION {
                return Some(wire_error(
                    commy_wire::ErrorCode::UnsupportedVersion,
                    &format!("protocol version {} is not supported; this server speaks {}", request.protocol_version, commy_wire::PROTOCOL_VERSION),
                    Some(&rid),
                ));
            }
            let tenant_id = request.tenant_id.clone();
            let method = request.credentials.method();

            let (dev, verifier) = {
                let g = server.read().await;
                (g.dev_auth, g.verifier.clone())
            };
            // Development hints are honoured only by the development verifier. A production server
            // refuses a request that asks for scopes it never granted rather than ignoring it.
            if !dev && request.dev_scopes.is_some() {
                return Some(wire_error(
                    commy_wire::ErrorCode::InvalidRequest,
                    "dev_scopes is for development servers only",
                    Some(&rid),
                ));
            }
            let dev_verifier = crate::server::verify::DevVerifier;
            let chosen: &dyn crate::server::verify::CredentialVerifier = if dev {
                &dev_verifier
            } else if let Some(v) = verifier.as_deref() {
                v
            } else {
                return Some(wire_error(
                    commy_wire::ErrorCode::Unauthenticated,
                    DEV_AUTH_REFUSED,
                    Some(&rid),
                ));
            };
            let verified = chosen
                .verify(crate::server::verify::VerifyRequest {
                    tenant_id: &tenant_id,
                    credentials: &request.credentials,
                    client_id_hint: request.client_id.as_deref(),
                    dev_scopes: request.dev_scopes.as_deref(),
                    session_id: &session.session_id,
                })
                .await;
            let verified = match verified {
                Ok(v) => v,
                Err(crate::server::verify::VerifyError::Backend(e)) => {
                    server.read().await.audit(&format!(
                        "authentication ERROR tenant={tenant_id} method={method}: {e}"
                    ));
                    return Some(wire_error(
                        commy_wire::ErrorCode::InternalError,
                        "Could not verify credentials",
                        Some(&rid),
                    ));
                }
                Err(refusal) => {
                    // Say no more than "no": never which part was wrong, never the credential.
                    server.read().await.audit(&format!(
                        "authentication REFUSED tenant={tenant_id} method={method} ({refusal:?})"
                    ));
                    return Some(wire_json(&commy_wire::ControlMessage::AuthenticationResult(
                        commy_wire::AuthenticationResult {
                            request_id: rid,
                            success: false,
                            message: "Authentication failed".to_string(),
                            server_version: env!("CARGO_PKG_VERSION").to_string(),
                            client_id: None,
                            permissions: None,
                            expires_in_seconds: None,
                        },
                    )));
                }
            };

            // Set session authentication
            session.client_id = Some(verified.client_id.clone());
            session.tenant_id = Some(tenant_id.clone());
            session.state = ClientState::Active;
            session.token = Some(if dev {
                format!(
                    "dev-token-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                )
            } else {
                uuid::Uuid::new_v4().to_string()
            });
            session.permissions = Some(verified.permissions.clone());
            session
                .grants_imported
                .store(false, std::sync::atomic::Ordering::Release);

            // Ensure the tenant record exists in the server so services can be created under it
            {
                let mut server_guard = server.write().await;
                let _ = server_guard.get_tenant(&tenant_id);
                server_guard.audit(&format!(
                    "authentication ACCEPTED client={} tenant={tenant_id} method={method}{}",
                    verified.client_id,
                    if dev { " (development authentication)" } else { "" }
                ));
            }

            Some(wire_json(&commy_wire::ControlMessage::AuthenticationResult(
                commy_wire::AuthenticationResult {
                    request_id: rid,
                    success: true,
                    message: if dev {
                        "Authentication successful (development mode)".to_string()
                    } else {
                        "Authentication successful".to_string()
                    },
                    server_version: env!("CARGO_PKG_VERSION").to_string(),
                    client_id: Some(verified.client_id),
                    permissions: Some(verified.scopes),
                    expires_in_seconds: verified.expires_in_seconds,
                },
            )))
        }

        "CreateService" => {
            println!("[CreateService] Raw data: {:?}", data);
            let tenant_id = data
                .and_then(|d| d.get("tenant_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let service_name = data
                .and_then(|d| d.get("service_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // The creating SDK sets the file size: required, at least one byte, page-rounded,
            // and capped by the server maximum.
            let requested_size = match data.and_then(|d| d.get("size")).and_then(|v| v.as_u64()) {
                Some(n) => n,
                None => return Some(invalid_request("size is required (bytes, a positive integer)")),
            };

            println!(
                "[CreateService] Session tenant_id: {:?}, Request tenant_id: {}, Session state: {:?}",
                session.tenant_id, tenant_id, session.state
            );

            // Check authentication to tenant
            if session.tenant_id.as_ref().map(|t| t.as_str()) != Some(tenant_id) {
                println!(
                    "[CreateService] Auth check failed: session.tenant_id={:?} vs request.tenant_id={}",
                    session.tenant_id, tenant_id
                );
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": {
                        "code": "PERMISSION_DENIED",
                        "message": "Not authenticated to this tenant"
                    }
                }));
            }

            // Check create_service permission
            if let Some(perms) = &session.permissions {
                if !perms.has_permission(&crate::auth::Permission::ServiceCreate) {
                    return Some(serde_json::json!({
                        "type": "Error",
                        "data": {
                            "code": "PERMISSION_DENIED",
                            "message": "Permission denied: create_service required"
                        }
                    }));
                }
            }

            let file_size = {
                let max = server.read().await.max_service_size;
                match crate::service_file_size(requested_size, max) {
                    Ok(n) => n,
                    Err(crate::ServiceSizeError::Zero) => {
                        return Some(invalid_request("size must be at least 1 byte"))
                    }
                    Err(crate::ServiceSizeError::TooLarge) => {
                        return Some(invalid_request(&format!(
                            "size exceeds the server maximum of {max} bytes"
                        )))
                    }
                }
            };

            // Generate cryptographically random service ID; only Server records the file mapping
            let service_id = uuid::Uuid::new_v4().to_string();
            // The guard has already parsed every tenant id on the wire; parse again here so the
            // path is built from a `TenantId` and from nothing else.
            let tenant_dir = match crate::tenant_id::TenantId::parse(tenant_id) {
                Ok(id) => id.dir_name(),
                Err(e) => return Some(invalid_request(&e.to_string())),
            };
            let file_path = format!("{}/service_{}.mem", tenant_dir, service_id);

            if let Err(e) = crate::private_dir::create_private(std::path::Path::new(&tenant_dir)) {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "INTERNAL_ERROR", "message": format!("Failed to create tenant directory: {}", e) }
                }));
            }

            let mut server_guard = server.write().await;
            let file_id = server_guard.next_file_id;
            server_guard.next_file_id = file_id.wrapping_add(1).max(1);
            let tenant = match server_guard.tenants.get_mut(tenant_id) {
                Some(t) => t,
                None => {
                    return Some(serde_json::json!({
                        "type": "Error",
                        "data": { "code": "NOT_FOUND", "message": format!("Tenant '{}' not found", tenant_id) }
                    }))
                }
            };

            if let Err(e) = tenant.register_service(service_name, &file_path, file_size as usize) {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "INTERNAL_ERROR", "message": format!("Failed to create service file: {}", e) }
                }));
            }

            {
                // The creator administers what it creates.
                let creator = client_key(session);
                if let Err(e) = server_guard.grant(
                    tenant_id,
                    &creator,
                    Some(&service_id),
                    &[crate::server::grants::Spec::Service(
                        crate::server::grants::Action::Admin,
                    )],
                ) {
                    eprintln!("[grants] cannot grant the creator admin: {e}");
                    let _ = std::fs::remove_file(&file_path);
                    return Some(serde_json::json!({
                        "type": "Error",
                        "data": { "code": "INTERNAL_ERROR", "message": "Could not record the new service" }
                    }));
                }
            }
            if let Err(e) = server_guard.persist_row(
                &service_id,
                &crate::persist::ServiceRow {
                    tenant: tenant_id.to_string(),
                    name: service_name.to_string(),
                    path: file_path.clone(),
                    file_id,
                    trusted_local: false,
                },
            ) {
                eprintln!("[persist] cannot record the new service: {e}");
                let _ = std::fs::remove_file(&file_path);
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "INTERNAL_ERROR", "message": "Could not record the new service" }
                }));
            }
            server_guard.service_registry.insert(
                service_id.clone(),
                crate::ServiceRecord {
                    tenant_name: tenant_id.to_string(),
                    service_name: service_name.to_string(),
                    file_path,
                    file_id,
                    trusted_local: false,
                    seq: 0,
                },
            );

            println!(
                "[CreateService] Created service '{}' in tenant '{}' with ID {}",
                service_name, tenant_id, service_id
            );

            Some(serde_json::json!({
                "type": "Service",
                "data": {
                    "service_id": service_id,
                    "service_name": service_name,
                    "tenant_id": tenant_id,
                    "size": file_size
                }
            }))
        }

        "GetService" => {
            let tenant_id = data
                .and_then(|d| d.get("tenant_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let service_name = data
                .and_then(|d| d.get("service_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            println!(
                "[GetService] Session tenant_id: {:?}, Request tenant_id: {}",
                session.tenant_id, tenant_id
            );

            // Check authentication to tenant
            if session.tenant_id.as_ref().map(|t| t.as_str()) != Some(tenant_id) {
                println!(
                    "[GetService] Auth check failed: session.tenant_id={:?} vs request.tenant_id={}",
                    session.tenant_id, tenant_id
                );
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": {
                        "code": "PERMISSION_DENIED",
                        "message": "Not authenticated to this tenant"
                    }
                }));
            }

            // Check read_service permission
            if let Some(perms) = &session.permissions {
                if !perms.has_permission(&crate::auth::Permission::ServiceRead) {
                    return Some(serde_json::json!({
                        "type": "Error",
                        "data": {
                            "code": "PERMISSION_DENIED",
                            "message": "Permission denied: read_service required"
                        }
                    }));
                }
            }

            // Look up service in registry — only Server knows the (tenant, service) → file mapping
            let server_guard = server.read().await;
            let found = server_guard
                .service_registry
                .iter()
                .find(|(_, r)| r.tenant_name == tenant_id && r.service_name == service_name)
                .map(|(id, _)| id.clone());

            // A service the client has no access to looks the same as one that does not exist.
            let found = found.filter(|id| {
                session_allows(
                    &server_guard,
                    session,
                    id,
                    crate::server::grants::Action::Read,
                )
            });

            match found {
                Some(service_id) => {
                    println!(
                        "[GetService] Found service '{}' in tenant '{}' → id={}",
                        service_name, tenant_id, service_id
                    );
                    Some(serde_json::json!({
                        "type": "Service",
                        "data": {
                            "service_id": service_id,
                            "service_name": service_name,
                            "tenant_id": tenant_id
                        }
                    }))
                }
                None => {
                    println!(
                        "[GetService] Service '{}' not found in tenant '{}'",
                        service_name, tenant_id
                    );
                    Some(serde_json::json!({
                        "type": "Error",
                        "data": {
                            "code": "NOT_FOUND",
                            "message": format!("Service '{}' not found in tenant '{}'", service_name, tenant_id)
                        }
                    }))
                }
            }
        }

        "DeleteService" => {
            println!("[DeleteService] Raw data: {:?}", data);
            let tenant_id = data
                .and_then(|d| d.get("tenant_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let service_name = data
                .and_then(|d| d.get("service_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            println!(
                "[DeleteService] Session tenant_id: {:?}, Request tenant_id: {}, Session state: {:?}",
                session.tenant_id, tenant_id, session.state
            );

            // Check authentication to tenant
            if session.tenant_id.as_ref().map(|t| t.as_str()) != Some(tenant_id) {
                println!("[DeleteService] Auth check failed: session.tenant_id={:?} vs request.tenant_id={}", session.tenant_id, tenant_id);
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": {
                        "code": "PERMISSION_DENIED",
                        "message": "Not authenticated to this tenant"
                    }
                }));
            }

            // Which service? It is named, not identified, in this message.
            let server_guard = server.read().await;
            let Some(service_id) = server_guard
                .service_registry
                .iter()
                .find(|(_, r)| r.tenant_name == tenant_id && r.service_name == service_name)
                .map(|(id, _)| id.clone())
            else {
                return Some(wire_error(
                    commy_wire::ErrorCode::NotFound,
                    &format!("Service '{}' not found in tenant '{}'", service_name, tenant_id),
                    None,
                ));
            };
            // Deleting a service takes service:admin on that service (a tenant admin has it too).
            if !session_allows(
                &server_guard,
                session,
                &service_id,
                crate::server::grants::Action::Admin,
            ) {
                return Some(wire_error(
                    commy_wire::ErrorCode::PermissionDenied,
                    "Needs service:admin on this service",
                    None,
                ));
            }

            // A service in the middle of a rotation cannot be deleted from under it.
            if server_guard.rotations.active(&service_id).is_some() {
                return Some(wire_error(
                    commy_wire::ErrorCode::RateLimited,
                    "A rotation of this service is running; try again when it has finished",
                    None,
                ));
            }
            drop(server_guard);

            let who = client_key(session);
            let mut guard = server.write().await;
            // Record the deletion first and refuse if that fails: a service that was removed but
            // is still stored would come back, with its file gone, after a restart.
            if let Err(e) = guard.forget_row(&service_id) {
                guard.audit(&format!(
                    "service delete REFUSED service={service_id} by={who}: cannot record it: {e}"
                ));
                return Some(wire_error(
                    commy_wire::ErrorCode::InternalError,
                    "Could not record the deletion",
                    None,
                ));
            }
            let Some(record) = guard.service_registry.remove(&service_id) else {
                return Some(wire_error(commy_wire::ErrorCode::NotFound, "Unknown service", None));
            };
            if let Some(tenant) = guard.tenants.get_mut(tenant_id) {
                // false only if the tenant's name map had already lost the service; the registry entry is gone either way.
                let _ = tenant.remove_service(&record.service_name);
            }
            guard.retired_files.insert(record.file_id);
            // The grants named this service id, which is never reused; drop them so nothing
            // lingers in the grant list (and the log, so a replay does not resurrect them).
            let mut clients: Vec<String> = guard
                .grants
                .list(tenant_id, None, Some(&service_id))
                .into_iter()
                .filter(|e| e.service_id.as_deref() == Some(service_id.as_str()))
                .map(|e| e.client_id)
                .collect();
            clients.sort();
            clients.dedup();
            for client in clients {
                if let Err(e) = guard.revoke(tenant_id, &client, Some(&service_id), &[]) {
                    guard.audit(&format!(
                        "service delete: could not drop {client}'s grants on {service_id}: {e}"
                    ));
                }
            }
            // Everyone on the file is told and dropped; unflushed writes are lost, not replayed.
            let members = guard.forwarder.take_file(record.file_id);
            guard.audit(&format!(
                "service DELETED service={service_id} name={} tenant={tenant_id} by={who}",
                record.service_name
            ));
            for m in members {
                let _ = m.tx.send(crate::server::forwarding::Outbound::Text(wire_json(
                    &commy_wire::ControlMessage::Loss(commy_wire::Loss {
                        service_id: service_id.clone(),
                        file_id: record.file_id,
                        kind: commy_wire::LossKind::Revoked,
                        ranges: Vec::new(),
                    }),
                )));
            }
            // Best effort: a local client may still have the file mapped (Windows then refuses).
            if let Err(e) = std::fs::remove_file(&record.file_path) {
                guard.audit(&format!(
                    "service delete: could not remove {}: {e}",
                    record.file_path
                ));
            }
            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": true,
                    "message": "Service deleted"
                }
            }))
        }

        "CreateTenant" => {
            let tenant_id = data
                .and_then(|d| d.get("tenant_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tenant_name = data
                .and_then(|d| d.get("tenant_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if tenant_id.is_empty() || tenant_name.is_empty() {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": {
                        "code": "INVALID_REQUEST",
                        "message": "tenant_id and tenant_name are required"
                    }
                }));
            }

            if let Some(refusal) = require_own_tenant_admin(session, tenant_id) {
                return Some(refusal);
            }

            // Create tenant
            let mut server_guard = server.write().await;

            // Get or create the tenant
            let _tenant = server_guard.get_tenant(tenant_id);

            println!("Created tenant '{}' (name: {})", tenant_id, tenant_name);

            Some(serde_json::json!({
                "type": "TenantResult",
                "data": {
                    "success": true,
                    "tenant_id": tenant_id.to_string(),
                    "message": format!("Tenant '{}' created", tenant_name)
                }
            }))
        }

        "DeleteTenant" => {
            let tenant_id = data
                .and_then(|d| d.get("tenant_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if tenant_id.is_empty() {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": {
                        "code": "INVALID_REQUEST",
                        "message": "tenant_id is required"
                    }
                }));
            }

            if let Some(refusal) = require_own_tenant_admin(session, tenant_id) {
                return Some(refusal);
            }

            // Delete tenant: its record, its services and its directory. The reply says
            // success only if the tenant existed and has been removed.
            let mut server_guard = server.write().await;
            if server_guard.tenants.remove(tenant_id).is_none() {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NOT_FOUND", "message": format!("Tenant '{}' not found", tenant_id) }
                }));
            }
            server_guard
                .service_registry
                .retain(|_, record| record.tenant_name != tenant_id);
            drop(server_guard);
            if let Ok(id) = crate::tenant_id::TenantId::parse(tenant_id) {
                let _ = std::fs::remove_dir_all(id.dir_name());
            }
            session.tenant_id = None;
            session.token = None;
            session.permissions = None;
            session.subscriptions.clear();
            session.state = ClientState::Unauthenticated;
            println!("Deleted tenant '{}'", tenant_id);

            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": true,
                    "message": format!("Tenant '{}' deleted", tenant_id)
                }
            }))
        }

        "Heartbeat" => {
            // Heartbeat is just a keep-alive, respond with acknowledgment
            session.last_heartbeat_ack = Some(chrono::Utc::now());
            Some(serde_json::json!({
                "type": "Heartbeat",
                "data": {
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            }))
        }

        "GetServiceFilePath" | "ConfirmLocal" | "Resync" | "RangesChanged" | "FlushEnd" | "RotateRequest" | "RotationDrained" | "RotationFilled" | "RotationAbort" | "Grant" | "Revoke" | "ListGrants" | "SetLocalMapping" => {
            handle_local_mapping(message, session, &server).await
        }

        "Disconnect" => {
            session.drop_pending_nonces();
            server.read().await.forwarder.leave_session(&session.session_id);
            session.state = ClientState::Disconnected;
            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": true,
                    "message": "Disconnected"
                }
            }))
        }

        _ => {
            eprintln!("[SDK] Unhandled message type: {}", msg_type);
            Some(serde_json::json!({
                "type": "Error",
                "data": {
                    "code": "INVALID_REQUEST",
                    "message": format!("Unknown message type: {}", msg_type)
                }
            }))
        }
    }
}

/// Tests for the JSON/SDK message path (`handle_sdk_message`).
///
/// These tests exist because the SDK sends JSON text frames, which go through
/// `handle_sdk_message`, NOT through `handle_message` (the binary/MessagePack path
/// tested in `mod tests`). Both paths must be tested independently — a test of one
/// gives zero confidence in the other.
#[cfg(test)]
mod sdk_tests {
    use super::*;

    // ── helpers ────────────────────────────────────────────────────────────────

    /// Build and authenticate a session against `tenant_id`, also ensuring the
    /// tenant record exists in `server`.  Returns the authenticated session.
    async fn make_authed_session(server: Arc<RwLock<Server>>, tenant_id: &str) -> ClientSession {
        let mut session = ClientSession::new();
        let msg = serde_json::json!({
            "type": "Authenticate",
            "data": { "tenant_id": tenant_id, "request_id": "a", "protocol_version": 1, "method": "api_key", "key": "test" }
        });
        let resp = handle_sdk_message(msg, &mut session, Arc::clone(&server))
            .await
            .expect("Authenticate must return a response");
        assert_eq!(resp["type"], "AuthenticationResult", "auth failed: {resp}");
        assert_eq!(resp["data"]["success"], true);
        session
    }

    /// Call CreateService and return the opaque service_id.
    async fn create_service(
        server: Arc<RwLock<Server>>,
        session: &mut ClientSession,
        tenant_id: &str,
        service_name: &str,
    ) -> String {
        let msg = serde_json::json!({
            "type": "CreateService",
            "data": { "tenant_id": tenant_id, "service_name": service_name, "size": 65536 }
        });
        let resp = handle_sdk_message(msg, session, Arc::clone(&server))
            .await
            .expect("CreateService must return a response");
        assert_eq!(resp["type"], "Service", "CreateService failed: {resp}");
        resp["data"]["service_id"]
            .as_str()
            .expect("service_id must be a string")
            .to_string()
    }

    /// Remove the tenant directory created during a test.
    fn cleanup_tenant(tenant_id: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{}", tenant_id));
    }

    // ── authentication ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_authenticate_sets_session_state() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        assert_eq!(session.state, ClientState::Active);
        assert_eq!(session.tenant_id.as_deref(), Some(tenant_id.as_str()));
        assert!(session.token.is_some());
        assert!(session.permissions.is_some());
        cleanup_tenant(&tenant_id);
    }

    #[tokio::test]
    async fn test_sdk_authenticate_creates_tenant_in_server() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        make_authed_session(Arc::clone(&server), &tenant_id).await;

        assert!(
            server.read().await.tenants.contains_key(&tenant_id),
            "Authenticate must register the tenant in Server so CreateService can find it"
        );
        cleanup_tenant(&tenant_id);
    }

    // ── service management ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_create_service_returns_service_id() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        let service_id = create_service(Arc::clone(&server), &mut session, &tenant_id, "cfg").await;
        assert!(!service_id.is_empty());

        // service_id must be registered in the server registry
        assert!(
            server
                .read()
                .await
                .service_registry
                .contains_key(&service_id),
            "Created service must appear in service_registry"
        );
        cleanup_tenant(&tenant_id);
    }

    #[tokio::test]
    async fn test_sdk_get_service_not_found_returns_error() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        let msg = serde_json::json!({
            "type": "GetService",
            "data": { "tenant_id": &tenant_id, "service_name": "does_not_exist" }
        });
        let resp = handle_sdk_message(msg, &mut session, Arc::clone(&server))
            .await
            .unwrap();
        assert_eq!(resp["type"], "Error");
        assert_eq!(resp["data"]["code"], "NOT_FOUND");
        cleanup_tenant(&tenant_id);
    }

    #[tokio::test]
    async fn test_sdk_get_service_after_create_returns_same_id() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        let created_id = create_service(Arc::clone(&server), &mut session, &tenant_id, "svc").await;

        let msg = serde_json::json!({
            "type": "GetService",
            "data": { "tenant_id": &tenant_id, "service_name": "svc" }
        });
        let resp = handle_sdk_message(msg, &mut session, Arc::clone(&server))
            .await
            .unwrap();
        assert_eq!(resp["type"], "Service");
        assert_eq!(resp["data"]["service_id"], created_id);
        cleanup_tenant(&tenant_id);
    }

    // ── variable round-trip (the test that would have caught the original bug) ─





    // ── allocate / deallocate ──────────────────────────────────────────────────



    // ── subscribe / unsubscribe ────────────────────────────────────────────────



    // ── regression guards ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_unknown_message_type_returns_error_not_not_implemented() {
        // Regression: before the fix, unknown types returned "Operation not implemented
        // on server" with code "INVALID_REQUEST", silently swallowing bugs where a known
        // type was simply misspelled.  This test pins the error message.
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        // Unknown types are only reported to authenticated sessions (the dispatcher guard
        // refuses everything else first).
        let mut session = make_authed_session(Arc::clone(&server), "unknown_type_tenant").await;

        let msg = serde_json::json!({ "type": "TotallyMadeUp", "data": {} });
        let resp = handle_sdk_message(msg, &mut session, server).await.unwrap();
        assert_eq!(resp["type"], "Error");
        // Must NOT say "Operation not implemented" — that was the misleading old message
        let msg_str = resp["data"]["message"].as_str().unwrap_or("");
        assert!(
            !msg_str.contains("not implemented"),
            "Error message must not say 'not implemented': {msg_str}"
        );
        // Must mention the offending type so the caller can diagnose it
        assert!(
            msg_str.contains("TotallyMadeUp"),
            "Error message must echo the unknown type name: {msg_str}"
        );
    }

    #[tokio::test]
    async fn test_sdk_create_service_without_auth_returns_error() {
        // CreateService for a different tenant than the authenticated one must be rejected.
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        // Try to create a service under a DIFFERENT tenant without authenticating to it
        let msg = serde_json::json!({
            "type": "CreateService",
            "data": { "tenant_id": "other_tenant", "service_name": "svc", "size": 65536 }
        });
        let resp = handle_sdk_message(msg, &mut session, Arc::clone(&server))
            .await
            .unwrap();
        assert_eq!(resp["type"], "Error");
        assert_ne!(
            resp["data"]["code"], "NOT_FOUND",
            "Wrong tenant must be an auth error, not NotFound"
        );
        cleanup_tenant(&tenant_id);
    }

    // ── full lifecycle ─────────────────────────────────────────────────────────


    // ── CreateTenant validation ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_create_tenant_empty_fields_returns_error() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = ClientSession::new();

        // Empty tenant_id
        let resp = handle_sdk_message(
            serde_json::json!({
                "type": "CreateTenant",
                "data": { "tenant_id": "", "tenant_name": "some_name" }
            }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(
            resp["type"], "Error",
            "empty tenant_id must be rejected: {resp}"
        );

        // Empty tenant_name
        let resp = handle_sdk_message(
            serde_json::json!({
                "type": "CreateTenant",
                "data": { "tenant_id": "valid_id", "tenant_name": "" }
            }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(
            resp["type"], "Error",
            "empty tenant_name must be rejected: {resp}"
        );
    }

    // ── AllocateVariable to non-existent service ───────────────────────────────


    // ── DeleteService cross-tenant ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_delete_service_cross_tenant_returns_unauthorized() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        // Attempt to delete a service from a different tenant
        let resp = handle_sdk_message(
            serde_json::json!({
                "type": "DeleteService",
                "data": { "tenant_id": "other_tenant_xyz", "service_name": "svc" }
            }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(
            resp["type"], "Error",
            "cross-tenant DeleteService must return Error: {resp}"
        );
        let code = resp["data"]["code"].as_str().unwrap_or("");
        assert!(
            code == "UNAUTHENTICATED" || code == "PERMISSION_DENIED",
            "wrong tenant must yield Unauthorized, got: {}",
            code
        );
        cleanup_tenant(&tenant_id);
    }

    // ── ReportVariableChanges ──────────────────────────────────────────────────



    // ── CreateTenant success ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_create_tenant_success() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        // CreateTenant needs TenantAdmin on the session's own tenant.
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        let resp = handle_sdk_message(
            serde_json::json!({
                "type": "CreateTenant",
                "data": {
                    "tenant_id": tenant_id,
                    "tenant_name": "Test Organisation"
                }
            }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();

        assert_eq!(
            resp["type"], "TenantResult",
            "CreateTenant with valid fields must return TenantResult: {resp}"
        );
        assert_eq!(
            resp["data"]["success"], true,
            "CreateTenant must report success: {resp}"
        );
        assert_eq!(
            resp["data"]["tenant_id"], tenant_id,
            "CreateTenant must echo back the tenant_id: {resp}"
        );
        cleanup_tenant(&tenant_id);
    }

    // ── DeleteService (permission and lookup; the deletion itself is in delete_service_tests) ──

    #[tokio::test]
    async fn test_sdk_delete_service_needs_service_admin_on_that_service() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = make_authed_session(Arc::clone(&server), &tenant_id).await;
        let service_id = create_service(Arc::clone(&server), &mut admin, &tenant_id, "svc").await;
        // a second client of the tenant that holds only service:write on it
        server
            .read()
            .await
            .grants
            .grant(
                &tenant_id,
                "writer",
                Some(&service_id),
                &[crate::server::grants::Spec::Service(crate::server::grants::Action::Write)],
            )
            .unwrap();
        let mut writer = ClientSession::new();
        let r = handle_sdk_message(
            serde_json::json!({"type":"Authenticate","data":{"tenant_id":tenant_id,"request_id":"a","protocol_version":1,"method":"api_key","key":"x","client_id":"writer","dev_scopes":[]}}),
            &mut writer,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(r["data"]["success"], true, "{r}");
        let resp = handle_sdk_message(
            serde_json::json!({"type":"DeleteService","data":{"tenant_id":tenant_id,"service_name":"svc"}}),
            &mut writer,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(resp["data"]["code"], "PERMISSION_DENIED", "{resp}");
        cleanup_tenant(&tenant_id);
    }

    #[tokio::test]
    async fn test_sdk_delete_service_of_an_unknown_name_is_not_found() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;
        let resp = handle_sdk_message(
            serde_json::json!({"type":"DeleteService","data":{"tenant_id":tenant_id,"service_name":"nope"}}),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(resp["data"]["code"], "NOT_FOUND", "{resp}");
        cleanup_tenant(&tenant_id);
    }

    // GetService names a service by name: it must not hand out the id of one the client cannot reach.
    #[tokio::test]
    async fn test_sdk_get_service_hides_services_the_client_has_no_access_to() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = make_authed_session(Arc::clone(&server), &tenant_id).await;
        let _id = create_service(Arc::clone(&server), &mut admin, &tenant_id, "secret").await;
        let mut reader = ClientSession::new();
        // tenant-level read, but this client's access to "secret" was never granted... and is denied
        let r = handle_sdk_message(
            serde_json::json!({"type":"Authenticate","data":{"tenant_id":tenant_id,"request_id":"a","protocol_version":1,"method":"api_key","key":"x","client_id":"nosy","dev_scopes":["service:read"]}}),
            &mut reader,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(r["data"]["success"], true, "{r}");
        let sid = server.read().await.service_registry.keys().next().unwrap().clone();
        server
            .read()
            .await
            .grants
            .revoke(&tenant_id, "nosy", Some(&sid), &[])
            .unwrap();
        let resp = handle_sdk_message(
            serde_json::json!({"type":"GetService","data":{"tenant_id":tenant_id,"service_name":"secret"}}),
            &mut reader,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(resp["type"], "Error", "the id must not be handed out: {resp}");
        assert!(resp["data"].get("service_id").is_none(), "{resp}");
        cleanup_tenant(&tenant_id);
    }

    // ── DeleteTenant ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_delete_tenant_empty_id_returns_error() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = ClientSession::new();

        let resp = handle_sdk_message(
            serde_json::json!({
                "type": "DeleteTenant",
                "data": { "tenant_id": "" }
            }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();

        assert_eq!(
            resp["type"], "Error",
            "DeleteTenant with empty tenant_id must return Error: {resp}"
        );
        assert_eq!(
            resp["data"]["code"], "INVALID_REQUEST",
            "empty tenant_id must yield InvalidRequest: {resp}"
        );
    }

    #[tokio::test]
    async fn test_sdk_delete_tenant_success() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;
        assert!(server.read().await.tenants.contains_key(&tenant_id));

        let resp = handle_sdk_message(
            serde_json::json!({
                "type": "DeleteTenant",
                "data": { "tenant_id": tenant_id }
            }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();

        assert_eq!(
            resp["type"], "Result",
            "DeleteTenant must return Result: {resp}"
        );
        assert_eq!(
            resp["data"]["success"], true,
            "DeleteTenant must report success: {resp}"
        );
        // Success means it is gone, and the session no longer holds it.
        assert!(!server.read().await.tenants.contains_key(&tenant_id));
        assert!(session.tenant_id.is_none());
    }

    // ── Heartbeat (SDK JSON path) ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_heartbeat_returns_timestamp() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = ClientSession::new();

        let resp = handle_sdk_message(
            serde_json::json!({ "type": "Heartbeat", "data": {} }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();

        assert_eq!(
            resp["type"], "Heartbeat",
            "SDK Heartbeat must return Heartbeat response: {resp}"
        );
        assert!(
            resp["data"]["timestamp"].is_string(),
            "Heartbeat response must include a timestamp string: {resp}"
        );
    }

    // ── Disconnect (SDK JSON path) ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_sdk_disconnect_sets_disconnected_state() {
        let tenant_id = format!("t_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut session = make_authed_session(Arc::clone(&server), &tenant_id).await;

        assert_eq!(
            session.state,
            ClientState::Active,
            "Session must be Active before Disconnect"
        );

        let resp = handle_sdk_message(
            serde_json::json!({ "type": "Disconnect", "data": {} }),
            &mut session,
            Arc::clone(&server),
        )
        .await
        .unwrap();

        assert_eq!(
            resp["type"], "Result",
            "Disconnect must return Result: {resp}"
        );
        assert_eq!(
            resp["data"]["success"], true,
            "Disconnect must report success: {resp}"
        );
        assert_eq!(
            session.state,
            ClientState::Disconnected,
            "Session state must be Disconnected after Disconnect message"
        );
        cleanup_tenant(&tenant_id);
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    async fn send(
        server: &Arc<RwLock<Server>>,
        session: &mut ClientSession,
        v: serde_json::Value,
    ) -> serde_json::Value {
        handle_sdk_message(v, session, Arc::clone(server)).await.unwrap()
    }

    async fn authed(server: &Arc<RwLock<Server>>, tenant: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = send(server, &mut s, serde_json::json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}})).await;
        assert_eq!(r["data"]["success"], true);
        s
    }

    async fn make_service(server: &Arc<RwLock<Server>>, s: &mut ClientSession, tenant: &str) -> String {
        let r = send(server, s, serde_json::json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":65536}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        r["data"]["service_id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn a_unauthenticated_create_tenant_is_refused() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut anon = ClientSession::new();
        let r = send(&server, &mut anon, serde_json::json!({"type":"CreateTenant","data":{"tenant_id":"victim_x","tenant_name":"n"}})).await;
        assert_eq!(r["type"], "Error", "unauthenticated CreateTenant must be refused, got {r}");
    }

    #[tokio::test]
    async fn b_unauthenticated_delete_tenant_is_refused() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut anon = ClientSession::new();
        let r = send(&server, &mut anon, serde_json::json!({"type":"DeleteTenant","data":{"tenant_id":"victim_x"}})).await;
        assert_eq!(r["type"], "Error", "unauthenticated DeleteTenant must be refused, got {r}");
    }

    #[tokio::test]
    async fn c_delete_tenant_actually_removes_the_tenant() {
        // Control: does DeleteTenant do anything at all?
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = authed(&server, "ctl_tenant").await;
        let _ = send(&server, &mut a, serde_json::json!({"type":"DeleteTenant","data":{"tenant_id":"ctl_tenant"}})).await;
        assert!(!server.read().await.tenants.contains_key("ctl_tenant"));
    }


    #[tokio::test]
    async fn e_other_tenant_cannot_use_a_service_it_does_not_own() {
        let ta = format!("a_{}", uuid::Uuid::new_v4().simple());
        let tb = format!("b_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = authed(&server, &ta).await;
        let sid = make_service(&server, &mut a, &ta).await;
        let mut b = authed(&server, &tb).await;
        let r = send(&server, &mut b, serde_json::json!({"type":"GetServiceFilePath","data":{"request_id":"r","service_id":sid}})).await;
        let _ = std::fs::remove_dir_all(format!("tenant_{}", ta));
        let _ = std::fs::remove_dir_all(format!("tenant_{}", tb));
        assert_eq!(r["type"], "Error", "tenant B reached tenant A's service: {r}");
    }

    #[tokio::test]
    async fn f_authenticate_with_no_credentials_is_refused() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(false)));
        let mut s = ClientSession::new();
        let r = send(&server, &mut s, serde_json::json!({"type":"Authenticate","data":{}})).await;
        assert_ne!(r["data"]["success"], true, "no-credential Authenticate succeeded: {r}");
    }

    #[tokio::test]
    async fn g_tenant_id_cannot_escape_the_working_directory() {
        let marker = format!("ca_escape_{}", uuid::Uuid::new_v4().simple());
        let tenant = format!("a/../../{}", marker);
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        // The id is refused at Authenticate.
        let mut s = ClientSession::new();
        let r = send(&server, &mut s, serde_json::json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}})).await;
        assert_eq!(r["type"], "Error", "{r}");
        // And even a session that somehow holds the bad id cannot build a path from it.
        s.tenant_id = Some(tenant.clone());
        s.token = Some("t".into());
        s.permissions = Some(crate::auth::PermissionSet::admin());
        s.state = ClientState::Active;
        let r = send(&server, &mut s, serde_json::json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":65536}})).await;
        assert_eq!(r["type"], "Error", "{r}");
        let outside = std::env::current_dir().unwrap().parent().unwrap().join(&marker);
        let escaped = outside.exists();
        let _ = std::fs::remove_dir_all(&outside);
        let _ = std::fs::remove_dir_all("tenant_a");
        assert!(!escaped, "tenant_id escaped the working directory: {}", outside.display());
    }
}

#[cfg(test)]
mod dev_auth_off_tests {
    use super::*;

    #[tokio::test]
    async fn authenticate_is_refused_by_default_on_both_paths() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(false)));
        let mut s = ClientSession::new();
        let r = handle_sdk_message(
            serde_json::json!({"type":"Authenticate","data":{"tenant_id":"t","api_key":"k"}}),
            &mut s,
            Arc::clone(&server),
        )
        .await
        .unwrap();
        assert_eq!(r["type"], "Error", "{r}");
        assert!(s.tenant_id.is_none() && s.permissions.is_none());
    }

    #[tokio::test]
    async fn unauthenticated_session_cannot_do_anything_but_heartbeat() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(false)));
        let mut s = ClientSession::new();
        for t in ["CreateService", "GetService", "GetServiceFilePath", "Resync", "CreateTenant", "SomethingNew"] {
            let r = handle_sdk_message(
                serde_json::json!({"type": t, "data": {"tenant_id": "t", "service_id": "x", "tenant_name": "n", "service_name": "s"}}),
                &mut s,
                Arc::clone(&server),
            )
            .await
            .unwrap();
            assert_eq!(r["type"], "Error", "{t}: {r}");
        }
        let r = handle_sdk_message(serde_json::json!({"type":"Heartbeat"}), &mut s, server)
            .await
            .unwrap();
        assert_eq!(r["type"], "Heartbeat");
    }

    #[tokio::test]
    async fn bad_tenant_ids_are_refused_before_any_arm_runs() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        for bad in ["..", "a/../b", "a\\b", "", &"x".repeat(65), "a b"] {
            let mut s = ClientSession::new();
            let r = handle_sdk_message(
                serde_json::json!({"type":"Authenticate","data":{"tenant_id":bad}}),
                &mut s,
                Arc::clone(&server),
            )
            .await
            .unwrap();
            assert_eq!(r["type"], "Error", "{bad:?}: {r}");
            assert!(s.tenant_id.is_none());
        }
    }
}

#[cfg(test)]
mod removed_variable_support_tests {
    use super::*;
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    async fn authed(server: &Arc<RwLock<Server>>, t: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    /// Plan C6: named variables are gone. Each old message is now just an unknown type, even for
    /// an authenticated admin on a service it owns.
    #[tokio::test]
    async fn the_named_variable_messages_are_gone() {
        let t = format!("rv_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = authed(&server, &t).await;
        let r = call(&server, &mut a, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        for ty in [
            "AllocateVariable",
            "ReadVariable",
            "WriteVariable",
            "DeallocateVariable",
            "Subscribe",
            "Unsubscribe",
            "ReportVariableChanges",
        ] {
            let r = call(&server, &mut a, json!({"type":ty,"data":{"service_id":id,"variable_name":"x","size":4}})).await;
            assert_eq!(r["type"], "Error", "{ty}: {r}");
            let msg = r["data"]["message"].as_str().unwrap_or("").to_lowercase();
            assert!(msg.contains("unknown"), "{ty} must be an unknown message type: {r}");
        }
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    /// Plan C6: named variables are gone. Each old message is now just an unknown type, even for
    /// an authenticated admin on a service it owns.

    /// The guard still refuses any message that names a service the client has no access to,
    /// whatever its type, before a handler sees it.
    #[tokio::test]
    async fn the_guard_refuses_any_message_naming_a_service_without_access() {
        let t = format!("rv_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = authed(&server, &t).await;
        admin.client_id = Some("admin".into());
        let r = call(&server, &mut admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let mut bob = authed(&server, &t).await;
        bob.client_id = Some("bob".into());
        bob.permissions = Some(crate::auth::PermissionSet::new());
        let r = call(&server, &mut bob, json!({"type":"SomeFutureMessage","data":{"service_id":id}})).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        // the admin gets past the guard (and hits the unknown-type error instead)
        let r = call(&server, &mut admin, json!({"type":"SomeFutureMessage","data":{"service_id":id}})).await;
        assert_ne!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }
}

#[cfg(test)]
mod service_size_tests {
    use super::*;

    async fn send(
        server: &Arc<RwLock<Server>>,
        session: &mut ClientSession,
        v: serde_json::Value,
    ) -> serde_json::Value {
        handle_sdk_message(v, session, Arc::clone(server)).await.unwrap()
    }

    async fn authed(server: &Arc<RwLock<Server>>, tenant: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = send(
            server,
            &mut s,
            serde_json::json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}}),
        )
        .await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    fn create(tenant: &str, size: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":size}})
    }

    fn file_len(tenant: &str, service_id: &str) -> u64 {
        std::fs::metadata(format!("tenant_{tenant}/service_{service_id}.mem"))
            .unwrap()
            .len()
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    fn tenant() -> String {
        format!("sz_{}", uuid::Uuid::new_v4().simple())
    }

    #[test]
    fn service_file_size_rounds_up_to_whole_pages() {
        let max = 10 * 1024 * 1024;
        assert_eq!(crate::service_file_size(1, max), Ok(4096));
        assert_eq!(crate::service_file_size(4096, max), Ok(4096));
        assert_eq!(crate::service_file_size(4097, max), Ok(8192));
        assert_eq!(crate::service_file_size(5000, max), Ok(8192));
    }

    #[test]
    fn service_file_size_refuses_zero_and_too_large() {
        let max = 10 * 1024 * 1024;
        assert_eq!(crate::service_file_size(0, max), Err(crate::ServiceSizeError::Zero));
        assert_eq!(crate::service_file_size(max, max), Ok(max));
        assert_eq!(crate::service_file_size(max + 1, max), Err(crate::ServiceSizeError::TooLarge));
        assert_eq!(crate::service_file_size(u64::MAX, max), Err(crate::ServiceSizeError::TooLarge));
    }

    #[test]
    fn default_maximum_is_ten_mebibytes() {
        assert_eq!(crate::DEFAULT_MAX_SERVICE_SIZE, 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn create_service_requires_a_size() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let r = send(
            &server,
            &mut s,
            serde_json::json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc"}}),
        )
        .await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn create_service_refuses_zero_negative_and_non_integer_sizes() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        for bad in [
            serde_json::json!(0),
            serde_json::json!(-5),
            serde_json::json!(1.5),
            serde_json::json!("4096"),
            serde_json::json!(null),
        ] {
            let r = send(&server, &mut s, create(&t, bad.clone())).await;
            assert_eq!(r["type"], "Error", "size {bad} must be refused: {r}");
            assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        }
        assert!(server.read().await.service_registry.is_empty());
        cleanup(&t);
    }

    #[tokio::test]
    async fn create_service_sets_the_file_size_rounded_to_pages() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        for (asked, got) in [(1u64, 4096u64), (4096, 4096), (5000, 8192), (65536, 65536)] {
            let r = send(&server, &mut s, create(&t, serde_json::json!(asked))).await;
            assert_eq!(r["type"], "Service", "{r}");
            assert_eq!(r["data"]["size"], got, "reply size for {asked}");
            let id = r["data"]["service_id"].as_str().unwrap();
            assert_eq!(file_len(&t, id), got, "file size for {asked}");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn create_service_refuses_sizes_above_the_maximum_and_accepts_the_maximum() {
        let t = tenant();
        let server = Arc::new(RwLock::new(
            Server::with_dev_auth(true).with_max_service_size(8192),
        ));
        let mut s = authed(&server, &t).await;
        let r = send(&server, &mut s, create(&t, serde_json::json!(8193))).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        let r = send(&server, &mut s, create(&t, serde_json::json!(8192))).await;
        assert_eq!(r["type"], "Service", "{r}");
        assert_eq!(server.read().await.service_registry.len(), 1);
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_refused_size_leaves_no_file_behind() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let r = send(&server, &mut s, create(&t, serde_json::json!(u64::MAX))).await;
        assert_eq!(r["type"], "Error", "{r}");
        let files = std::fs::read_dir(format!("tenant_{t}"))
            .map(|d| d.count())
            .unwrap_or(0);
        assert_eq!(files, 0, "no service file may exist after a refusal");
        cleanup(&t);
    }
}

#[cfg(test)]
mod local_mapping_tests {
    use super::*;
    use serde_json::json;

    async fn send(
        server: &Arc<RwLock<Server>>,
        session: &mut ClientSession,
        v: serde_json::Value,
    ) -> serde_json::Value {
        handle_sdk_message(v, session, Arc::clone(server)).await.unwrap()
    }

    async fn authed(server: &Arc<RwLock<Server>>, tenant: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = send(
            server,
            &mut s,
            json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}}),
        )
        .await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    async fn make_service(server: &Arc<RwLock<Server>>, s: &mut ClientSession, tenant: &str) -> String {
        let r = send(
            server,
            s,
            json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":65536}}),
        )
        .await;
        assert_eq!(r["type"], "Service", "{r}");
        r["data"]["service_id"].as_str().unwrap().to_string()
    }

    fn get_path(service_id: &str) -> serde_json::Value {
        json!({"type":"GetServiceFilePath","data":{"request_id":"r1","service_id":service_id}})
    }

    fn confirm(file_id: u64, nonce: &str) -> serde_json::Value {
        json!({"type":"ConfirmLocal","data":{"request_id":"r2","file_id":file_id,"nonce":nonce}})
    }

    fn tenant() -> String {
        format!("lm_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    fn nonce_files(tenant: &str) -> usize {
        std::fs::read_dir(format!("tenant_{tenant}"))
            .map(|d| {
                d.flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with(".nonce-"))
                    .count()
            })
            .unwrap_or(0)
    }

    /// A server with one trusted-local service and a read-write session on it.
    async fn setup() -> (Arc<RwLock<Server>>, ClientSession, String, String) {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let id = make_service(&server, &mut s, &t).await;
        assert!(server.write().await.set_trusted_local(&id, true));
        (server, s, t, id)
    }

    #[tokio::test]
    async fn service_that_is_not_trusted_local_gets_a_file_id_and_no_path() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let id = make_service(&server, &mut s, &t).await;
        let r = send(&server, &mut s, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert_eq!(r["data"]["request_id"], "r1");
        assert!(r["data"]["file_id"].as_u64().unwrap() >= 1);
        assert!(r["data"].get("path").is_none(), "{r}");
        assert!(r["data"].get("nonce_path").is_none(), "{r}");
        assert_eq!(nonce_files(&t), 0);
        cleanup(&t);
    }

    #[tokio::test]
    async fn trusted_local_service_with_a_write_grant_is_offered_path_and_nonce() {
        let (server, mut s, t, id) = setup().await;
        let r = send(&server, &mut s, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        let path = std::path::PathBuf::from(r["data"]["path"].as_str().unwrap());
        let nonce_path = std::path::PathBuf::from(r["data"]["nonce_path"].as_str().unwrap());
        assert!(path.is_absolute() && nonce_path.is_absolute());
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 65536);
        let nonce = std::fs::read_to_string(&nonce_path).unwrap();
        assert_eq!(nonce.len(), 64);
        assert!(nonce.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(nonce_path.parent(), path.parent(), "nonce lives in the tenant directory");
        assert!(!nonce_path.to_string_lossy().contains(&id), "nonce path is not the service file");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_read_only_session_is_never_offered_a_path() {
        let (server, _owner, t, id) = setup().await;
        let mut s = authed(&server, &t).await;
        s.client_id = Some("viewer".to_string());
        s.permissions = Some(crate::auth::PermissionSet::read_only());
        let r = send(&server, &mut s, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert!(r["data"].get("path").is_none(), "{r}");
        assert_eq!(nonce_files(&t), 0, "no nonce file may be written for a read-only caller");
        cleanup(&t);
    }

    #[tokio::test]
    async fn file_ids_are_stable_per_service_and_distinct_between_services() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let a = make_service(&server, &mut s, &t).await;
        let b = make_service(&server, &mut s, &t).await;
        let ra = send(&server, &mut s, get_path(&a)).await;
        let ra2 = send(&server, &mut s, get_path(&a)).await;
        let rb = send(&server, &mut s, get_path(&b)).await;
        assert_eq!(ra["data"]["file_id"], ra2["data"]["file_id"]);
        assert_ne!(ra["data"]["file_id"], rb["data"]["file_id"]);
        cleanup(&t);
    }

    #[tokio::test]
    async fn unknown_service_is_not_found_and_uses_the_wire_spelling() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let r = send(&server, &mut s, get_path("no-such-service")).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        assert_eq!(r["data"]["request_id"], "r1");
        cleanup(&t);
    }

    #[tokio::test]
    async fn another_tenants_service_is_refused_and_writes_no_nonce() {
        let (server, _owner, t, id) = setup().await;
        let other = tenant();
        let mut s2 = authed(&server, &other).await;
        let r = send(&server, &mut s2, get_path(&id)).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert!(r["data"].get("path").is_none(), "{r}");
        assert_eq!(nonce_files(&t), 0);
        cleanup(&t);
        cleanup(&other);
    }

    #[tokio::test]
    async fn a_request_without_request_id_is_invalid() {
        let (server, mut s, t, id) = setup().await;
        let r = send(
            &server,
            &mut s,
            json!({"type":"GetServiceFilePath","data":{"service_id":id}}),
        )
        .await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn correct_nonce_is_acknowledged_and_the_nonce_file_is_deleted() {
        let (server, mut s, t, id) = setup().await;
        let r = send(&server, &mut s, get_path(&id)).await;
        let file_id = r["data"]["file_id"].as_u64().unwrap();
        let nonce_path = r["data"]["nonce_path"].as_str().unwrap().to_string();
        let nonce = std::fs::read_to_string(&nonce_path).unwrap();
        let ack = send(&server, &mut s, confirm(file_id, &nonce)).await;
        assert_eq!(ack["type"], "ConfirmLocalAck", "{ack}");
        assert_eq!(ack["data"]["file_id"], file_id);
        assert!(!std::path::Path::new(&nonce_path).exists(), "nonce file must be deleted");
        assert!(s.local_files.contains(&(file_id as u32)));
        cleanup(&t);
    }

    #[tokio::test]
    async fn wrong_nonce_is_denied_and_cannot_be_retried() {
        let (server, mut s, t, id) = setup().await;
        let r = send(&server, &mut s, get_path(&id)).await;
        let file_id = r["data"]["file_id"].as_u64().unwrap();
        let nonce_path = r["data"]["nonce_path"].as_str().unwrap().to_string();
        let nonce = std::fs::read_to_string(&nonce_path).unwrap();
        let bad = send(&server, &mut s, confirm(file_id, "0".repeat(64).as_str())).await;
        assert_eq!(bad["type"], "Error", "{bad}");
        assert_eq!(bad["data"]["code"], "PERMISSION_DENIED", "{bad}");
        assert!(!std::path::Path::new(&nonce_path).exists());
        // The right nonce no longer works: one attempt per nonce.
        let again = send(&server, &mut s, confirm(file_id, &nonce)).await;
        assert_eq!(again["type"], "Error", "{again}");
        assert!(!s.local_files.contains(&(file_id as u32)));
        cleanup(&t);
    }

    #[tokio::test]
    async fn confirm_with_nothing_pending_is_invalid() {
        let (server, mut s, t, _id) = setup().await;
        let r = send(&server, &mut s, confirm(1, "abc")).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_nonce_for_one_file_cannot_confirm_another_file() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let a = make_service(&server, &mut s, &t).await;
        let b = make_service(&server, &mut s, &t).await;
        {
            let mut g = server.write().await;
            assert!(g.set_trusted_local(&a, true));
            assert!(g.set_trusted_local(&b, true));
        }
        let ra = send(&server, &mut s, get_path(&a)).await;
        let rb = send(&server, &mut s, get_path(&b)).await;
        let nonce_a = std::fs::read_to_string(ra["data"]["nonce_path"].as_str().unwrap()).unwrap();
        let fid_b = rb["data"]["file_id"].as_u64().unwrap();
        let r = send(&server, &mut s, confirm(fid_b, &nonce_a)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_session_cannot_pile_up_unconfirmed_nonces() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let mut ids = vec![];
        for _ in 0..(crate::server::local_mapping::MAX_PENDING_NONCES + 1) {
            let id = make_service(&server, &mut s, &t).await;
            assert!(server.write().await.set_trusted_local(&id, true));
            ids.push(id);
        }
        let mut limited = false;
        for id in &ids {
            let r = send(&server, &mut s, get_path(id)).await;
            if r["type"] == "Error" {
                assert_eq!(r["data"]["code"], "RATE_LIMITED", "{r}");
                limited = true;
            }
        }
        assert!(limited, "the {}th request must be rate limited", ids.len());
        assert!(nonce_files(&t) <= crate::server::local_mapping::MAX_PENDING_NONCES);
        cleanup(&t);
    }

    #[tokio::test]
    async fn disconnect_deletes_unconfirmed_nonce_files() {
        let (server, mut s, t, id) = setup().await;
        let r = send(&server, &mut s, get_path(&id)).await;
        assert!(r["data"]["nonce_path"].is_string());
        assert_eq!(nonce_files(&t), 1);
        send(&server, &mut s, json!({"type":"Disconnect","data":{}})).await;
        assert_eq!(nonce_files(&t), 0);
        cleanup(&t);
    }

    #[test]
    fn sweep_removes_only_nonce_files() {
        let d = tempfile::tempdir().unwrap();
        let keep = d.path().join("service_x.mem");
        std::fs::write(&keep, b"data").unwrap();
        let p = crate::server::local_mapping::create_nonce_file(d.path()).unwrap();
        crate::server::local_mapping::sweep_stale(d.path(), std::time::Duration::from_secs(3600));
        assert!(p.path.exists(), "a fresh nonce is not stale");
        crate::server::local_mapping::sweep_stale(d.path(), std::time::Duration::ZERO);
        assert!(!p.path.exists());
        assert!(keep.exists(), "non-nonce files are never touched");
    }

    #[test]
    fn secrets_match_compares_whole_values() {
        use crate::server::local_mapping::secrets_match;
        assert!(secrets_match("abc", "abc"));
        assert!(!secrets_match("abc", "abd"));
        assert!(!secrets_match("abc", "ab"));
        assert!(!secrets_match("", "a"));
        assert!(secrets_match("", ""));
    }

    #[cfg(unix)]
    #[test]
    fn nonce_file_is_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let p = crate::server::local_mapping::create_nonce_file(d.path()).unwrap();
        let mode = std::fs::metadata(&p.path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[cfg(test)]
mod resync_tests {
    use super::*;
    use serde_json::json;

    async fn send(
        server: &Arc<RwLock<Server>>,
        session: &mut ClientSession,
        v: serde_json::Value,
    ) -> serde_json::Value {
        handle_sdk_message(v, session, Arc::clone(server)).await.unwrap()
    }

    async fn authed(server: &Arc<RwLock<Server>>, tenant: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = send(
            server,
            &mut s,
            json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}}),
        )
        .await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    /// Create a service of `size` bytes; returns (service_id, file_id).
    async fn make_service(
        server: &Arc<RwLock<Server>>,
        s: &mut ClientSession,
        tenant: &str,
        size: u64,
    ) -> (String, u32) {
        let r = send(
            server,
            s,
            json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":size}}),
        )
        .await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let file_id = server.read().await.service_registry[&id].file_id;
        (id, file_id)
    }

    fn resync(file_id: u32) -> serde_json::Value {
        json!({"type":"Resync","data":{"request_id":"rs1","file_id":file_id}})
    }

    fn tenant() -> String {
        format!("rs_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    fn file_path(tenant: &str, service_id: &str) -> String {
        format!("tenant_{tenant}/service_{service_id}.mem")
    }

    /// Overwrite the file's bytes in place. `std::fs::write` truncates, which Windows refuses
    /// while the server holds the file mapped.
    fn fill(path: &str, content: &[u8]) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.write_all(content).unwrap();
        f.sync_all().unwrap();
    }

    /// Decode every queued frame and reassemble the file image they describe.
    fn reassemble(s: &mut ClientSession, size: usize) -> (Vec<u8>, Vec<commy_wire::RangeFrame>) {
        let frames: Vec<_> = s
            .outbound_frames
            .drain(..)
            .map(|b| commy_wire::RangeFrame::decode(&b).expect("a valid range frame"))
            .collect();
        let mut image = vec![0u8; size];
        for f in &frames {
            image[f.offset as usize..f.offset as usize + f.data.len()].copy_from_slice(&f.data);
        }
        (image, frames)
    }

    #[tokio::test]
    async fn resync_sends_the_whole_file_then_resync_done() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let (id, fid) = make_service(&server, &mut s, &t, 8192).await;
        let mut content = vec![0u8; 8192];
        for (i, b) in content.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        fill(&file_path(&t, &id), &content);

        let r = send(&server, &mut s, resync(fid)).await;
        assert_eq!(r["type"], "ResyncDone", "{r}");
        assert_eq!(r["data"]["request_id"], "rs1");
        assert_eq!(r["data"]["file_id"], fid);
        assert_eq!(r["data"]["size"], 8192);
        let (image, frames) = reassemble(&mut s, 8192);
        assert_eq!(image, content, "frames must reproduce the file exactly");
        assert!(frames.iter().all(|f| f.file_id == fid));
        assert!(frames.iter().all(|f| f.seq == r["data"]["seq"].as_u64().unwrap()));
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_file_over_one_frame_is_split_at_the_maximum_in_offset_order() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let size = commy_wire::MAX_PAYLOAD as u64 * 2 + 4096;
        let (id, fid) = make_service(&server, &mut s, &t, size).await;
        let mut content = vec![0u8; size as usize];
        content[commy_wire::MAX_PAYLOAD] = 9;
        content[size as usize - 1] = 7;
        fill(&file_path(&t, &id), &content);

        let r = send(&server, &mut s, resync(fid)).await;
        assert_eq!(r["type"], "ResyncDone", "{r}");
        let (image, frames) = reassemble(&mut s, size as usize);
        assert_eq!(frames.len(), 3);
        assert!(frames.iter().all(|f| f.data.len() <= commy_wire::MAX_PAYLOAD));
        assert!(frames.windows(2).all(|w| w[0].offset < w[1].offset));
        assert_eq!(image, content);
        cleanup(&t);
    }

    #[tokio::test]
    async fn unknown_file_is_not_found_and_queues_nothing() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let r = send(&server, &mut s, resync(424242)).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        assert_eq!(r["data"]["request_id"], "rs1");
        assert!(s.outbound_frames.is_empty());
        cleanup(&t);
    }

    #[tokio::test]
    async fn another_tenants_file_is_refused_and_queues_nothing() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut owner = authed(&server, &t).await;
        let (_id, fid) = make_service(&server, &mut owner, &t, 4096).await;
        let other = tenant();
        let mut s2 = authed(&server, &other).await;
        let r = send(&server, &mut s2, resync(fid)).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        assert!(s2.outbound_frames.is_empty());
        cleanup(&t);
        cleanup(&other);
    }

    #[tokio::test]
    async fn a_session_without_a_read_grant_is_refused() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let (_id, fid) = make_service(&server, &mut s, &t, 4096).await;
        // A second client of the same tenant with no permissions at all. (Permissions are filed
        // under the client's identity at first use, so it must not share the creator's.)
        let mut nobody = authed(&server, &t).await;
        nobody.client_id = Some("nobody".to_string());
        nobody.permissions = Some(crate::auth::PermissionSet::new());
        let r = send(&server, &mut nobody, resync(fid)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        assert!(nobody.outbound_frames.is_empty());
        cleanup(&t);
    }

    #[tokio::test]
    async fn unauthenticated_session_cannot_resync() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = ClientSession::new();
        let r = send(&server, &mut s, resync(1)).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert!(s.outbound_frames.is_empty());
    }

    #[tokio::test]
    async fn resync_without_request_id_is_invalid() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let (_id, fid) = make_service(&server, &mut s, &t, 4096).await;
        let r = send(
            &server,
            &mut s,
            json!({"type":"Resync","data":{"file_id":fid}}),
        )
        .await;
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        assert!(s.outbound_frames.is_empty());
        cleanup(&t);
    }

    #[tokio::test]
    async fn the_final_seq_is_the_services_current_seq() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = authed(&server, &t).await;
        let (id, fid) = make_service(&server, &mut s, &t, 4096).await;
        server.write().await.service_registry.get_mut(&id).unwrap().seq = 41;
        let r = send(&server, &mut s, resync(fid)).await;
        assert_eq!(r["data"]["seq"], 41, "{r}");
        let (_, frames) = reassemble(&mut s, 4096);
        assert!(frames.iter().all(|f| f.seq == 41));
        cleanup(&t);
    }
}

#[cfg(test)]
mod forwarding_tests {
    use super::*;
    use crate::server::forwarding::{merge_ranges, Outbound};
    use commy_wire::{ByteRange, RangeFrame};
    use serde_json::json;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    struct Peer {
        s: ClientSession,
        rx: UnboundedReceiver<Outbound>,
    }

    async fn send(
        server: &Arc<RwLock<Server>>,
        session: &mut ClientSession,
        v: serde_json::Value,
    ) -> serde_json::Value {
        handle_sdk_message(v, session, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("fw_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    async fn peer(server: &Arc<RwLock<Server>>, tenant: &str) -> Peer {
        let mut s = ClientSession::new();
        let r = send(
            server,
            &mut s,
            json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}}),
        )
        .await;
        assert_eq!(r["data"]["success"], true, "{r}");
        let (tx, rx) = unbounded_channel();
        s.outbound = Some(tx);
        Peer { s, rx }
    }

    /// A 4 KiB trusted-local service; returns (service_id, file_id, path).
    async fn service(server: &Arc<RwLock<Server>>, p: &mut Peer, tenant: &str) -> (String, u32, String) {
        let r = send(
            server,
            &mut p.s,
            json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":4096}}),
        )
        .await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let mut g = server.write().await;
        assert!(g.set_trusted_local(&id, true));
        let rec = &g.service_registry[&id];
        (id.clone(), rec.file_id, rec.file_path.clone())
    }

    async fn make_local(server: &Arc<RwLock<Server>>, p: &mut Peer, service_id: &str) {
        let r = send(
            server,
            &mut p.s,
            json!({"type":"GetServiceFilePath","data":{"request_id":"g","service_id":service_id}}),
        )
        .await;
        let nonce = std::fs::read_to_string(r["data"]["nonce_path"].as_str().unwrap()).unwrap();
        let fid = r["data"]["file_id"].as_u64().unwrap();
        let ack = send(
            server,
            &mut p.s,
            json!({"type":"ConfirmLocal","data":{"request_id":"c","file_id":fid,"nonce":nonce}}),
        )
        .await;
        assert_eq!(ack["type"], "ConfirmLocalAck", "{ack}");
    }

    async fn make_remote(server: &Arc<RwLock<Server>>, p: &mut Peer, file_id: u32) {
        let r = send(
            server,
            &mut p.s,
            json!({"type":"Resync","data":{"request_id":"rs","file_id":file_id}}),
        )
        .await;
        assert_eq!(r["type"], "ResyncDone", "{r}");
        p.s.outbound_frames.clear();
    }

    fn poke(path: &str, offset: u64, bytes: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        f.write_all(bytes).unwrap();
        f.sync_all().unwrap();
    }

    fn peek(path: &str, offset: usize, len: usize) -> Vec<u8> {
        std::fs::read(path).unwrap()[offset..offset + len].to_vec()
    }

    fn changed(file_id: u32, ranges: serde_json::Value) -> serde_json::Value {
        json!({"type":"RangesChanged","data":{"request_id":"f","file_id":file_id,"ranges":ranges}})
    }

    fn flush_end(file_id: u32) -> serde_json::Value {
        json!({"type":"FlushEnd","data":{"request_id":"fe","file_id":file_id}})
    }

    fn frame(file_id: u32, offset: u64, data: &[u8]) -> Vec<u8> {
        RangeFrame { file_id, seq: 0, offset, data: data.to_vec() }.encode()
    }

    fn next_text(p: &mut Peer) -> serde_json::Value {
        match p.rx.try_recv() {
            Ok(Outbound::Text(v)) => v,
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    fn next_binary(p: &mut Peer) -> RangeFrame {
        match p.rx.try_recv() {
            Ok(Outbound::Binary(b)) => RangeFrame::decode(&b).unwrap(),
            other => panic!("expected a binary frame, got {other:?}"),
        }
    }

    fn nothing(p: &mut Peer) {
        assert!(p.rx.try_recv().is_err(), "nothing should have been forwarded");
    }

    #[tokio::test]
    async fn a_local_flush_reaches_local_receivers_as_ranges_and_remote_receivers_with_bytes() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, path) = service(&server, &mut a, &t).await;
        let mut b = peer(&server, &t).await;
        let mut c = peer(&server, &t).await;
        make_local(&server, &mut a, &id).await;
        make_local(&server, &mut b, &id).await;
        make_remote(&server, &mut c, fid).await;

        poke(&path, 100, &[9, 8, 7, 6]);
        let ack = send(&server, &mut a.s, changed(fid, json!([{"offset":100,"len":4}]))).await;
        assert_eq!(ack["type"], "FlushAck", "{ack}");
        assert_eq!(ack["data"]["seq"], 1);
        assert_eq!(ack["data"]["request_id"], "f");

        let upd = next_text(&mut b);
        assert_eq!(upd["type"], "RangesUpdated", "{upd}");
        assert_eq!(upd["data"]["seq"], 1);
        assert_eq!(upd["data"]["ranges"], json!([{"offset":100,"len":4}]));
        let f = next_binary(&mut c);
        assert_eq!((f.file_id, f.seq, f.offset, f.data), (fid, 1, 100, vec![9, 8, 7, 6]));
        nothing(&mut a);
        nothing(&mut b);
        nothing(&mut c);
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_remote_flush_writes_the_file_and_forwards_at_flush_end() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, path) = service(&server, &mut a, &t).await;
        let mut b = peer(&server, &t).await;
        let mut c = peer(&server, &t).await;
        let mut d = peer(&server, &t).await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut c, fid).await;
        make_remote(&server, &mut d, fid).await;
        let _ = &mut b;

        let err = handle_range_frame(&frame(fid, 200, &[1, 2, 3]), &mut c.s, &server).await;
        assert!(err.is_none(), "{err:?}");
        assert_eq!(peek(&path, 200, 3), vec![1, 2, 3], "bytes land in the file at once");
        nothing(&mut a);
        nothing(&mut d);

        let ack = send(&server, &mut c.s, flush_end(fid)).await;
        assert_eq!(ack["type"], "FlushAck", "{ack}");
        assert_eq!(ack["data"]["seq"], 1);
        let upd = next_text(&mut a);
        assert_eq!(upd["data"]["ranges"], json!([{"offset":200,"len":3}]));
        let f = next_binary(&mut d);
        assert_eq!((f.offset, f.seq, f.data), (200, 1, vec![1, 2, 3]));
        nothing(&mut c);
        cleanup(&t);
    }

    #[tokio::test]
    async fn adjacent_frames_of_one_flush_merge_into_one_range() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, _path) = service(&server, &mut a, &t).await;
        let mut c = peer(&server, &t).await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut c, fid).await;
        assert!(handle_range_frame(&frame(fid, 4, &[5; 4]), &mut c.s, &server).await.is_none());
        assert!(handle_range_frame(&frame(fid, 0, &[1; 4]), &mut c.s, &server).await.is_none());
        send(&server, &mut c.s, flush_end(fid)).await;
        let upd = next_text(&mut a);
        assert_eq!(upd["data"]["ranges"], json!([{"offset":0,"len":8}]));
        cleanup(&t);
    }

    #[tokio::test]
    async fn each_flush_gets_the_next_sequence_number() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, _p) = service(&server, &mut a, &t).await;
        make_local(&server, &mut a, &id).await;
        for want in 1..=3u64 {
            let ack = send(&server, &mut a.s, changed(fid, json!([{"offset":0,"len":1}]))).await;
            assert_eq!(ack["data"]["seq"], want, "{ack}");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn refused_flushes_forward_nothing_and_do_not_spend_a_sequence_number() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, _p) = service(&server, &mut a, &t).await;
        let mut b = peer(&server, &t).await;
        make_local(&server, &mut a, &id).await;
        make_local(&server, &mut b, &id).await;

        // out of bounds, empty list, out of order, zero length
        for bad in [
            json!([{"offset":4090,"len":100}]),
            json!([]),
            json!([{"offset":8,"len":4},{"offset":0,"len":4}]),
            json!([{"offset":0,"len":0}]),
        ] {
            let r = send(&server, &mut a.s, changed(fid, bad.clone())).await;
            assert_eq!(r["type"], "Error", "{bad}: {r}");
            assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{bad}: {r}");
        }
        nothing(&mut b);
        let ack = send(&server, &mut a.s, changed(fid, json!([{"offset":0,"len":1}]))).await;
        assert_eq!(ack["data"]["seq"], 1, "refusals must not advance seq: {ack}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn access_rules_apply_to_both_flush_paths() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, _p) = service(&server, &mut a, &t).await;
        make_local(&server, &mut a, &id).await;

        // another tenant never learns the file exists
        let other = tenant();
        let mut x = peer(&server, &other).await;
        let r = send(&server, &mut x.s, changed(fid, json!([{"offset":0,"len":1}]))).await;
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let r = send(&server, &mut x.s, flush_end(fid)).await;
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let r = handle_range_frame(&frame(fid, 0, &[1]), &mut x.s, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");

        // same tenant, read-only grant
        let mut ro = peer(&server, &t).await;
        ro.s.permissions = Some(crate::auth::PermissionSet::read_only());
        let r = send(&server, &mut ro.s, flush_end(fid)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let r = handle_range_frame(&frame(fid, 0, &[1]), &mut ro.s, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");

        // unauthenticated
        let mut anon = ClientSession::new();
        let r = handle_range_frame(&frame(fid, 0, &[1]), &mut anon, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "UNAUTHENTICATED", "{r}");
        cleanup(&t);
        cleanup(&other);
    }

    #[tokio::test]
    async fn sender_kinds_cannot_be_mixed() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, path) = service(&server, &mut a, &t).await;
        make_local(&server, &mut a, &id).await;
        let mut c = peer(&server, &t).await;
        make_remote(&server, &mut c, fid).await;

        // a session that never proved it maps the file cannot send data-less ranges
        let r = send(&server, &mut c.s, changed(fid, json!([{"offset":0,"len":1}]))).await;
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        // a local mapper cannot send range frames
        let before = peek(&path, 0, 4);
        let r = handle_range_frame(&frame(fid, 0, &[0xAA; 4]), &mut a.s, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        assert_eq!(peek(&path, 0, 4), before, "a refused frame must not touch the file");
        // FlushEnd with no frames
        let r = send(&server, &mut c.s, flush_end(fid)).await;
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        // a malformed frame (zero-length) and an out-of-bounds frame
        let mut bad = frame(fid, 0, &[1]);
        bad[21..25].copy_from_slice(&0u32.to_le_bytes());
        bad.truncate(25);
        let r = handle_range_frame(&bad, &mut c.s, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        let r = handle_range_frame(&frame(fid, 4095, &[1, 2]), &mut c.s, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_disconnected_or_dropped_member_is_no_longer_forwarded_to() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut a = peer(&server, &t).await;
        let (id, fid, _p) = service(&server, &mut a, &t).await;
        let mut b = peer(&server, &t).await;
        let mut c = peer(&server, &t).await;
        make_local(&server, &mut a, &id).await;
        make_local(&server, &mut b, &id).await;
        make_remote(&server, &mut c, fid).await;
        assert_eq!(server.read().await.forwarder.member_count(fid), 3);

        send(&server, &mut b.s, json!({"type":"Disconnect","data":{}})).await;
        assert_eq!(server.read().await.forwarder.member_count(fid), 2);
        drop(c.rx); // connection gone without a Disconnect
        let ack = send(&server, &mut a.s, changed(fid, json!([{"offset":0,"len":1}]))).await;
        assert_eq!(ack["type"], "FlushAck", "{ack}");
        nothing(&mut b);
        assert_eq!(server.read().await.forwarder.member_count(fid), 1, "closed queue pruned");
        cleanup(&t);
    }

    #[test]
    fn merge_ranges_sorts_merges_overlaps_and_touching_and_drops_empty() {
        let r = |offset, len| ByteRange { offset, len };
        assert_eq!(merge_ranges(vec![]), vec![]);
        assert_eq!(merge_ranges(vec![r(10, 5), r(0, 5)]), vec![r(0, 5), r(10, 5)]);
        assert_eq!(merge_ranges(vec![r(0, 5), r(5, 5)]), vec![r(0, 10)]);
        assert_eq!(merge_ranges(vec![r(0, 10), r(2, 3)]), vec![r(0, 10)]);
        assert_eq!(merge_ranges(vec![r(0, 5), r(3, 10)]), vec![r(0, 13)]);
        assert_eq!(merge_ranges(vec![r(7, 0), r(0, 1)]), vec![r(0, 1)]);
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    use crate::server::forwarding::Outbound;
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    struct Peer {
        s: ClientSession,
        rx: UnboundedReceiver<Outbound>,
    }

    fn tenant() -> String {
        format!("ro_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    fn new_server() -> Arc<RwLock<Server>> {
        Arc::new(RwLock::new(
            Server::with_dev_auth(true)
                .with_drain_timeout(Duration::from_millis(400))
                .with_fill_timeout(Duration::from_secs(5))
                .with_rotation_min_interval(Duration::ZERO),
        ))
    }

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> Option<serde_json::Value> {
        handle_sdk_message(v, s, Arc::clone(server)).await
    }

    async fn peer(server: &Arc<RwLock<Server>>, tenant: &str, client_id: &str) -> Peer {
        let mut s = ClientSession::new();
        let r = call(
            server,
            &mut s,
            json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}}),
        )
        .await
        .unwrap();
        assert_eq!(r["data"]["success"], true, "{r}");
        s.client_id = Some(client_id.to_string());
        let (tx, rx) = unbounded_channel();
        s.outbound = Some(tx);
        Peer { s, rx }
    }

    /// A service of `size` bytes, open to local mapping; returns (service_id, file_id, path).
    async fn service(server: &Arc<RwLock<Server>>, p: &mut Peer, tenant: &str, size: u64) -> (String, u32, String) {
        let r = call(
            server,
            &mut p.s,
            json!({"type":"CreateService","data":{"tenant_id":tenant,"service_name":"svc","size":size}}),
        )
        .await
        .unwrap();
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let mut g = server.write().await;
        assert!(g.set_trusted_local(&id, true));
        let rec = &g.service_registry[&id];
        (id.clone(), rec.file_id, rec.file_path.clone())
    }

    async fn make_local(server: &Arc<RwLock<Server>>, p: &mut Peer, service_id: &str) {
        let r = call(server, &mut p.s, json!({"type":"GetServiceFilePath","data":{"request_id":"g","service_id":service_id}}))
            .await
            .unwrap();
        let nonce = std::fs::read_to_string(r["data"]["nonce_path"].as_str().unwrap()).unwrap();
        let fid = r["data"]["file_id"].as_u64().unwrap();
        let ack = call(server, &mut p.s, json!({"type":"ConfirmLocal","data":{"request_id":"c","file_id":fid,"nonce":nonce}}))
            .await
            .unwrap();
        assert_eq!(ack["type"], "ConfirmLocalAck", "{ack}");
    }

    async fn make_remote(server: &Arc<RwLock<Server>>, p: &mut Peer, file_id: u32) {
        let r = call(server, &mut p.s, json!({"type":"Resync","data":{"request_id":"rs","file_id":file_id}}))
            .await
            .unwrap();
        assert_eq!(r["type"], "ResyncDone", "{r}");
        p.s.outbound_frames.clear();
    }

    fn resize(service_id: &str, new_size: u64) -> serde_json::Value {
        json!({"type":"RotateRequest","data":{"request_id":"q","service_id":service_id,"reason":"RESIZE","new_size":new_size}})
    }

    fn revoke(service_id: &str, client: &str) -> serde_json::Value {
        json!({"type":"RotateRequest","data":{"request_id":"q","service_id":service_id,"reason":"REVOCATION","revoke_client_id":client}})
    }

    async fn recv(p: &mut Peer) -> serde_json::Value {
        match tokio::time::timeout(Duration::from_secs(3), p.rx.recv()).await {
            Ok(Some(Outbound::Text(v))) => v,
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    fn quiet(p: &mut Peer) {
        assert!(p.rx.try_recv().is_err(), "no message expected");
    }

    async fn drain(server: &Arc<RwLock<Server>>, p: &mut Peer, pending: &serde_json::Value) {
        assert_eq!(pending["type"], "RotationPending", "{pending}");
        let d = &pending["data"];
        let r = call(
            server,
            &mut p.s,
            json!({"type":"RotationDrained","data":{"request_id":"d","rotation_id":d["rotation_id"],"file_id":d["file_id"],"seq":0}}),
        )
        .await;
        assert!(r.is_none(), "a drain acknowledgement has no reply: {r:?}");
    }

    async fn filled(server: &Arc<RwLock<Server>>, p: &mut Peer, rotation_id: &serde_json::Value) -> Option<serde_json::Value> {
        call(server, &mut p.s, json!({"type":"RotationFilled","data":{"request_id":"fl","rotation_id":rotation_id}})).await
    }

    fn poke(path: &str, offset: u64, bytes: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        f.write_all(bytes).unwrap();
        f.sync_all().unwrap();
    }

    /// Resize driven by a local requester `a` (member) with an optional second member `b`.
    /// Returns the Prepared message received by `a` after every drain.
    async fn resize_to_prepared(server: &Arc<RwLock<Server>>, a: &mut Peer, id: &str, new_size: u64) -> serde_json::Value {
        let acc = call(server, &mut a.s, resize(id, new_size)).await.unwrap();
        assert_eq!(acc["type"], "RotationAccepted", "{acc}");
        let pend = recv(a).await;
        drain(server, a, &pend).await;
        recv(a).await
    }

    #[tokio::test]
    async fn resize_prepares_a_zeroed_file_for_the_requester_who_fills_it_then_commits() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, path) = service(&server, &mut a, &t, 4096).await;
        let mut b = peer(&server, &t, "bob").await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut b, fid).await;
        poke(&path, 10, &[7, 7, 7]);

        let acc = call(&server, &mut a.s, resize(&id, 8192)).await.unwrap();
        assert_eq!(acc["type"], "RotationAccepted", "{acc}");
        let pa = recv(&mut a).await;
        let pb = recv(&mut b).await;
        assert_eq!(pa["data"]["deadline_ms"], 400);
        assert_eq!(pa["data"]["fill_ms"], 5000);
        assert_eq!(pb["data"]["rotation_id"], acc["data"]["rotation_id"]);
        drain(&server, &mut a, &pa).await;
        drain(&server, &mut b, &pb).await;

        // Only the requester hears about the new file, with a path because it is a local mapper.
        let prep = recv(&mut a).await;
        assert_eq!(prep["type"], "RotationPrepared", "{prep}");
        quiet(&mut b);
        let d = &prep["data"];
        assert_eq!(d["old_file_id"], fid);
        assert_eq!(d["size"], 8192);
        let new_fid = d["new_file_id"].as_u64().unwrap() as u32;
        let new_path = d["path"].as_str().unwrap().to_string();
        assert!(std::fs::read(&new_path).unwrap().iter().all(|b| *b == 0), "zero filled");
        assert_eq!(server.read().await.service_registry[&id].file_id, fid, "not live yet");

        // The requester migrates in its own shape: here, only 2 bytes, at a different offset.
        poke(&new_path, 100, &[1, 2]);
        assert!(filled(&server, &mut a, &d["rotation_id"]).await.is_none());

        let ca = recv(&mut a).await;
        let cb = recv(&mut b).await;
        assert_eq!(ca["type"], "RotationCommit", "{ca}");
        assert_eq!(ca, cb, "every member gets the same commit");
        assert_eq!(ca["data"]["new_file_id"], new_fid);
        assert_eq!(ca["data"]["size"].as_u64(), Some(8192));
        assert!(ca["data"].get("generation").is_none(), "commy-wire 0.5.0 has no generation: {ca}");

        let (live_path, seq, retired, prepared_left) = {
            let g = server.read().await;
            let r = &g.service_registry[&id];
            (r.file_path.clone(), r.seq, g.retired_files.contains(&fid), g.prepared.len())
        };
        let bytes = std::fs::read(&live_path).unwrap();
        assert_eq!(&bytes[100..102], &[1, 2]);
        assert_eq!(&bytes[10..13], &[0, 0, 0], "the server did not copy the old bytes");
        assert_eq!(seq, 0);
        assert!(retired);
        assert_eq!(prepared_left, 0);
        let events = server.read().await.audit_events().join("\n");
        for word in ["REQUESTED", "GRANTED", "COMMITTED"] {
            assert!(events.contains(word), "{word} must be audited: {events}");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_remote_requester_fills_the_prepared_file_with_range_frames() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (_id, fid, _path) = service(&server, &mut a, &t, 4096).await;
        // a is a remote mirror, so it gets no path
        make_remote(&server, &mut a, fid).await;
        let id = server.read().await.service_registry.keys().next().unwrap().clone();
        let prep = resize_to_prepared(&server, &mut a, &id, 8192).await;
        assert_eq!(prep["type"], "RotationPrepared", "{prep}");
        assert!(prep["data"].get("path").is_none(), "{prep}");
        let new_fid = prep["data"]["new_file_id"].as_u64().unwrap() as u32;

        let frame = commy_wire::RangeFrame { file_id: new_fid, seq: 0, offset: 5000, data: vec![4, 5, 6] };
        assert!(handle_range_frame(&frame.encode(), &mut a.s, &server).await.is_none());
        let ack = call(&server, &mut a.s, json!({"type":"FlushEnd","data":{"request_id":"fe","file_id":new_fid}})).await.unwrap();
        assert_eq!(ack["type"], "FlushAck", "{ack}");
        assert_eq!(ack["data"]["seq"], 0, "a prepared file's first sequence number");
        assert_eq!(ack["data"]["request_id"], "fe");

        assert!(filled(&server, &mut a, &prep["data"]["rotation_id"]).await.is_none());
        let commit = recv(&mut a).await;
        assert_eq!(commit["type"], "RotationCommit", "{commit}");
        let live = server.read().await.service_registry[&id].file_path.clone();
        assert_eq!(&std::fs::read(&live).unwrap()[5000..5003], &[4, 5, 6]);
        cleanup(&t);
    }

    #[tokio::test]
    async fn the_prepared_file_is_invisible_to_everyone_but_the_requester() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, _fid, _p) = service(&server, &mut a, &t, 4096).await;
        make_local(&server, &mut a, &id).await;
        let prep = resize_to_prepared(&server, &mut a, &id, 8192).await;
        let new_fid = prep["data"]["new_file_id"].as_u64().unwrap() as u32;

        let mut b = peer(&server, &t, "bob").await;
        let frame = commy_wire::RangeFrame { file_id: new_fid, seq: 0, offset: 0, data: vec![9] };
        let r = handle_range_frame(&frame.encode(), &mut b.s, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let r = call(&server, &mut b.s, json!({"type":"Resync","data":{"request_id":"r","file_id":new_fid}})).await.unwrap();
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let r = filled(&server, &mut b, &prep["data"]["rotation_id"]).await.unwrap();
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "only the requester may answer: {r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn the_requester_cannot_answer_before_the_file_is_prepared() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, _fid, _p) = service(&server, &mut a, &t, 4096).await;
        make_local(&server, &mut a, &id).await;
        let acc = call(&server, &mut a.s, resize(&id, 8192)).await.unwrap();
        let _pending = recv(&mut a).await; // barrier open, not drained yet
        let r = filled(&server, &mut a, &acc["data"]["rotation_id"]).await.unwrap();
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_requester_abort_keeps_the_old_file_live_and_drops_no_one() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, _path) = service(&server, &mut a, &t, 4096).await;
        let mut b = peer(&server, &t, "bob").await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut b, fid).await;

        let acc = call(&server, &mut a.s, resize(&id, 8192)).await.unwrap();
        let pa = recv(&mut a).await;
        let _pb = recv(&mut b).await; // b never drains: a straggler
        drain(&server, &mut a, &pa).await;
        let prep = recv(&mut a).await;
        let new_path = prep["data"]["path"].as_str().unwrap().to_string();
        let r = call(&server, &mut a.s, json!({"type":"RotationAbort","data":{"request_id":"ab","rotation_id":acc["data"]["rotation_id"]}})).await;
        assert!(r.is_none());

        let aa = recv(&mut a).await;
        let ab = recv(&mut b).await;
        assert_eq!(aa["type"], "RotationAborted", "{aa}");
        assert_eq!(aa["data"]["reason"], "REQUESTER_ABORT");
        assert_eq!(aa, ab, "stragglers hear the abort too, and are not dropped");
        quiet(&mut a);
        quiet(&mut b);
        assert_eq!(server.read().await.service_registry[&id].file_id, fid, "old file stays live");
        assert!(server.read().await.prepared.is_empty());
        assert!(!std::path::Path::new(&new_path).exists(), "prepared file discarded");
        // the old file still works and both are still members
        let ack = call(&server, &mut a.s, json!({"type":"RangesChanged","data":{"request_id":"f","file_id":fid,"ranges":[{"offset":0,"len":1}]}})).await.unwrap();
        assert_eq!(ack["type"], "FlushAck", "{ack}");
        assert_eq!(recv_binary_or_text(&mut b).await, "frame");
        // and a new rotation may start
        let again = call(&server, &mut a.s, resize(&id, 8192)).await.unwrap();
        assert_eq!(again["type"], "RotationAccepted", "{again}");
        cleanup(&t);
    }

    async fn recv_binary_or_text(p: &mut Peer) -> &'static str {
        match tokio::time::timeout(Duration::from_secs(3), p.rx.recv()).await {
            Ok(Some(Outbound::Binary(_))) => "frame",
            Ok(Some(Outbound::Text(_))) => "text",
            other => panic!("expected a message, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_requester_that_never_fills_times_out_and_the_rotation_is_aborted() {
        let t = tenant();
        let server = Arc::new(RwLock::new(
            Server::with_dev_auth(true)
                .with_drain_timeout(Duration::from_millis(200))
                .with_fill_timeout(Duration::from_millis(300))
                .with_rotation_min_interval(Duration::ZERO),
        ));
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, _p) = service(&server, &mut a, &t, 4096).await;
        make_local(&server, &mut a, &id).await;
        let prep = resize_to_prepared(&server, &mut a, &id, 8192).await;
        let new_path = prep["data"]["path"].as_str().unwrap().to_string();
        let aborted = recv(&mut a).await; // the requester says nothing
        assert_eq!(aborted["type"], "RotationAborted", "{aborted}");
        assert_eq!(aborted["data"]["reason"], "FILL_TIMEOUT");
        assert_eq!(server.read().await.service_registry[&id].file_id, fid);
        assert!(!std::path::Path::new(&new_path).exists());
        assert!(server.read().await.rotations.active(&id).is_none(), "the rotation is over");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_member_that_does_not_drain_is_dropped_with_a_loss_at_the_commit() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, _p) = service(&server, &mut a, &t, 4096).await;
        let mut b = peer(&server, &t, "bob").await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut b, fid).await;

        let acc = call(&server, &mut a.s, resize(&id, 8192)).await.unwrap();
        let pa = recv(&mut a).await;
        let _pb = recv(&mut b).await;
        drain(&server, &mut a, &pa).await;
        let prep = recv(&mut a).await;
        assert!(filled(&server, &mut a, &acc["data"]["rotation_id"]).await.is_none());
        assert_eq!(recv(&mut a).await["type"], "RotationCommit");
        let lb = recv(&mut b).await;
        assert_eq!(lb["type"], "Loss", "{lb}");
        assert_eq!(lb["data"]["kind"], "ROTATION_STRAGGLER");
        quiet(&mut b);
        let _ = prep;
        cleanup(&t);
    }

    #[tokio::test]
    async fn after_a_commit_the_old_file_is_refused_and_the_new_one_gets_a_fresh_handshake() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, _p) = service(&server, &mut a, &t, 4096).await;
        make_local(&server, &mut a, &id).await;
        let prep = resize_to_prepared(&server, &mut a, &id, 8192).await;
        assert!(filled(&server, &mut a, &prep["data"]["rotation_id"]).await.is_none());
        let commit = recv(&mut a).await;
        let new_fid = commit["data"]["new_file_id"].as_u64().unwrap();

        let old = call(&server, &mut a.s, json!({"type":"RangesChanged","data":{"request_id":"f","file_id":fid,"ranges":[{"offset":0,"len":1}]}})).await.unwrap();
        assert_eq!(old["data"]["code"], "INVALID_REQUEST", "{old}");
        let old = call(&server, &mut a.s, json!({"type":"Resync","data":{"request_id":"r","file_id":fid}})).await.unwrap();
        assert_eq!(old["data"]["code"], "INVALID_REQUEST", "{old}");
        let r = call(&server, &mut a.s, json!({"type":"GetServiceFilePath","data":{"request_id":"g2","service_id":id}})).await.unwrap();
        assert_eq!(r["data"]["file_id"], new_fid, "{r}");
        assert!(r["data"]["nonce_path"].is_string(), "a fresh nonce is offered for the new file");
        cleanup(&t);
    }

    #[tokio::test]
    async fn revocation_keeps_the_layout_tells_the_rest_and_drops_the_revoked_client() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, path) = service(&server, &mut a, &t, 4096).await;
        let mut b = peer(&server, &t, "bob").await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut b, fid).await;
        poke(&path, 20, &[5, 5]);

        let acc = call(&server, &mut a.s, revoke(&id, "bob")).await.unwrap();
        assert_eq!(acc["type"], "RotationAccepted", "{acc}");
        // no drain barrier and no prepare step: the server keeps the old bytes itself
        let ca = recv(&mut a).await;
        assert_eq!(ca["type"], "RotationCommit", "{ca}");
        assert_eq!(ca["data"]["reason"], "REVOCATION");
        assert_eq!(ca["data"]["size"], 4096);
        let lb = recv(&mut b).await;
        assert_eq!(lb["type"], "Loss", "{lb}");
        assert_eq!(lb["data"]["kind"], "REVOKED");
        quiet(&mut a);
        quiet(&mut b);
        let new_path = server.read().await.service_registry[&id].file_path.clone();
        assert_eq!(&std::fs::read(&new_path).unwrap()[20..22], &[5, 5], "old bytes kept");
        cleanup(&t);
    }

    #[tokio::test]
    async fn permissions_are_checked_before_anything_is_sent() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, fid, _p) = service(&server, &mut a, &t, 4096).await;
        let mut b = peer(&server, &t, "bob").await;
        make_local(&server, &mut a, &id).await;
        make_remote(&server, &mut b, fid).await;

        let mut ro = peer(&server, &t, "ro").await;
        ro.s.permissions = Some(crate::auth::PermissionSet::read_only());
        for req in [resize(&id, 8192), revoke(&id, "bob")] {
            let r = call(&server, &mut ro.s, req).await.unwrap();
            assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        }
        let mut rz = peer(&server, &t, "rz").await;
        let mut only_resize = crate::auth::PermissionSet::new();
        only_resize.grant(crate::auth::Permission::ServiceResize);
        rz.s.permissions = Some(only_resize);
        let r = call(&server, &mut rz.s, revoke(&id, "bob")).await.unwrap();
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        quiet(&mut a);
        quiet(&mut b);
        let r = call(&server, &mut rz.s, resize(&id, 8192)).await.unwrap();
        assert_eq!(r["type"], "RotationAccepted", "{r}");
        let events = server.read().await.audit_events().join("\n");
        assert!(events.contains("REFUSED"), "{events}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn another_tenant_cannot_rotate_the_service() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, _fid, _p) = service(&server, &mut a, &t, 4096).await;
        let other = tenant();
        let mut x = peer(&server, &other, "mallory").await;
        let r = call(&server, &mut x.s, resize(&id, 8192)).await.unwrap();
        assert_eq!(r["type"], "Error", "{r}");
        assert!(server.read().await.rotations.active(&id).is_none());
        cleanup(&t);
        cleanup(&other);
    }

    #[tokio::test]
    async fn bad_requests_are_refused_and_start_nothing() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, _fid, _p) = service(&server, &mut a, &t, 4096).await;
        let max = server.read().await.max_service_size;
        let cases = [
            (json!({"type":"RotateRequest","data":{"request_id":"q","service_id":id,"reason":"RESIZE"}}), "INVALID_REQUEST"),
            (resize(&id, 0), "INVALID_REQUEST"),
            (resize(&id, max + 1), "INVALID_REQUEST"),
            (revoke(&id, "nobody"), "NOT_FOUND"),
            (resize("no-such-service", 4096), "NOT_FOUND"),
        ];
        for (req, code) in cases {
            let r = call(&server, &mut a.s, req.clone()).await.unwrap();
            assert_eq!(r["data"]["code"], code, "{req}: {r}");
        }
        assert!(server.read().await.rotations.active(&id).is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn only_one_rotation_runs_at_a_time_and_drain_acks_are_checked() {
        let t = tenant();
        let server = new_server();
        let mut a = peer(&server, &t, "alice").await;
        let (id, _fid, _p) = service(&server, &mut a, &t, 4096).await;
        make_local(&server, &mut a, &id).await;
        let acc = call(&server, &mut a.s, resize(&id, 8192)).await.unwrap();
        assert_eq!(acc["type"], "RotationAccepted", "{acc}");
        let second = call(&server, &mut a.s, resize(&id, 16384)).await.unwrap();
        assert_eq!(second["data"]["code"], "RATE_LIMITED", "{second}");

        let pend = recv(&mut a).await;
        let d = &pend["data"];
        let wrong = call(&server, &mut a.s, json!({"type":"RotationDrained","data":{"request_id":"d","rotation_id":"nope","file_id":d["file_id"],"seq":0}})).await.unwrap();
        assert_eq!(wrong["data"]["code"], "INVALID_REQUEST", "{wrong}");
        let mut stranger = peer(&server, &t, "eve").await;
        let r = call(&server, &mut stranger.s, json!({"type":"RotationDrained","data":{"request_id":"d","rotation_id":d["rotation_id"],"file_id":d["file_id"],"seq":0}})).await.unwrap();
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "a session that was not asked to drain cannot drain: {r}");
        cleanup(&t);
    }
}

#[cfg(test)]
mod scoped_access_tests {
    use super::*;
    use crate::server::grants::{Action, Spec};
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("sa_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    /// An authenticated session of `client_id` holding no tenant-level permissions at all, so only
    /// grants in the store count.
    async fn bare_client(server: &Arc<RwLock<Server>>, tenant: &str, client_id: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = call(
            server,
            &mut s,
            json!({"type":"Authenticate","data":{"tenant_id":tenant,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}}),
        )
        .await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s.client_id = Some(client_id.to_string());
        s.permissions = Some(crate::auth::PermissionSet::new());
        s
    }

    /// Two services created by an admin; returns (admin session, (service_id, file_id) x2).
    async fn two_services(server: &Arc<RwLock<Server>>, t: &str) -> (ClientSession, (String, u32), (String, u32)) {
        let mut admin = ClientSession::new();
        let r = call(server, &mut admin, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}})).await;
        assert_eq!(r["data"]["success"], true);
        admin.client_id = Some("admin".to_string());
        let mut out = vec![];
        for name in ["one", "two"] {
            let r = call(
                server,
                &mut admin,
                json!({"type":"CreateService","data":{"tenant_id":t,"service_name":name,"size":4096}}),
            )
            .await;
            assert_eq!(r["type"], "Service", "{r}");
            let id = r["data"]["service_id"].as_str().unwrap().to_string();
            let fid = server.read().await.service_registry[&id].file_id;
            out.push((id, fid));
        }
        let b = out.pop().unwrap();
        let a = out.pop().unwrap();
        (admin, a, b)
    }

    fn get_path(service_id: &str) -> serde_json::Value {
        json!({"type":"GetServiceFilePath","data":{"request_id":"g","service_id":service_id}})
    }

    #[tokio::test]
    async fn a_client_with_no_grant_reaches_no_service_of_its_own_tenant() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (_admin, (one, one_fid), _two) = two_services(&server, &t).await;
        let mut bob = bare_client(&server, &t, "bob").await;
        let r = call(&server, &mut bob, get_path(&one)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let r = call(&server, &mut bob, json!({"type":"Resync","data":{"request_id":"r","file_id":one_fid}})).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let r = call(&server, &mut bob, json!({"type":"FlushEnd","data":{"request_id":"f","file_id":one_fid}})).await;
        assert_eq!(r["type"], "Error", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_service_grant_opens_that_service_and_no_other() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (_admin, (one, one_fid), (two, two_fid)) = two_services(&server, &t).await;
        server.read().await.grants.grant(&t, "bob", Some(&one), &[Spec::Service(Action::Read)]).unwrap();
        let mut bob = bare_client(&server, &t, "bob").await;

        let r = call(&server, &mut bob, get_path(&one)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert_eq!(r["data"]["file_id"], one_fid);
        let r = call(&server, &mut bob, json!({"type":"Resync","data":{"request_id":"r","file_id":one_fid}})).await;
        assert_eq!(r["type"], "ResyncDone", "{r}");

        let r = call(&server, &mut bob, get_path(&two)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "the other service stays closed: {r}");
        let r = call(&server, &mut bob, json!({"type":"Resync","data":{"request_id":"r","file_id":two_fid}})).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn read_access_does_not_allow_writing_but_write_access_does() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (_admin, (one, fid), _two) = two_services(&server, &t).await;
        server.read().await.grants.grant(&t, "bob", Some(&one), &[Spec::Service(Action::Read)]).unwrap();
        let mut bob = bare_client(&server, &t, "bob").await;
        let frame = commy_wire::RangeFrame { file_id: fid, seq: 0, offset: 0, data: vec![1] }.encode();
        let r = handle_range_frame(&frame, &mut bob, &server).await.unwrap();
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "read only: {r}");

        server.read().await.grants.grant(&t, "bob", Some(&one), &[Spec::Service(Action::Write)]).unwrap();
        assert!(handle_range_frame(&frame, &mut bob, &server).await.is_none(), "now allowed");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_tenant_wide_grant_reaches_every_service() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (_admin, (one, _f1), (two, _f2)) = two_services(&server, &t).await;
        server.read().await.grants.grant(&t, "bob", None, &[Spec::Service(Action::Read)]).unwrap();
        let mut bob = bare_client(&server, &t, "bob").await;
        for id in [&one, &two] {
            let r = call(&server, &mut bob, get_path(id)).await;
            assert_eq!(r["type"], "ServiceFilePath", "{r}");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn revoking_one_service_closes_it_even_for_a_tenant_admin_and_leaves_the_other() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (mut admin, (one, _f1), (two, _f2)) = two_services(&server, &t).await;
        let r = call(&server, &mut admin, get_path(&one)).await;
        assert_eq!(r["type"], "ServiceFilePath", "admin starts with access: {r}");
        server.read().await.grants.revoke(&t, "admin", Some(&one), &[]).unwrap();
        let r = call(&server, &mut admin, get_path(&one)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let r = call(&server, &mut admin, get_path(&two)).await;
        assert_eq!(r["type"], "ServiceFilePath", "the other service is untouched: {r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn the_creator_of_a_service_administers_it() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        // A creator that holds only ServiceCreate at tenant level.
        let mut c = bare_client(&server, &t, "creator").await;
        let mut perms = crate::auth::PermissionSet::new();
        perms.grant(crate::auth::Permission::ServiceCreate);
        c.permissions = Some(perms);
        let r = call(&server, &mut c, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"mine","size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        assert!(server.read().await.grants.allows(&t, "creator", &id, Action::Admin));
        let r = call(&server, &mut c, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        // someone else in the tenant has nothing on it
        let mut other = bare_client(&server, &t, "other").await;
        let r = call(&server, &mut other, get_path(&id)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn another_tenants_grants_do_not_count() {
        let t = tenant();
        let other = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (_admin, (one, _f), _two) = two_services(&server, &t).await;
        // bob is granted on the same service id, but in the other tenant's role system
        server.read().await.grants.grant(&other, "bob", Some(&one), &[Spec::Service(Action::Admin)]).unwrap();
        let mut bob = bare_client(&server, &other, "bob").await;
        let r = call(&server, &mut bob, get_path(&one)).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert!(r["data"].get("path").is_none());
        cleanup(&t);
        cleanup(&other);
    }

}

#[cfg(test)]
mod grant_message_tests {
    use super::*;
    use crate::server::forwarding::Outbound;
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    struct Peer {
        s: ClientSession,
        rx: UnboundedReceiver<Outbound>,
    }

    fn tenant() -> String {
        format!("gm_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(tenant: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{tenant}"));
    }

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    /// An authenticated peer. `admin` keeps the tenant-admin permissions dev auth gives; otherwise
    /// the peer holds no tenant-level permissions and only grants count.
    async fn peer(server: &Arc<RwLock<Server>>, t: &str, client_id: &str, admin: bool) -> Peer {
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s.client_id = Some(client_id.to_string());
        if !admin {
            s.permissions = Some(crate::auth::PermissionSet::new());
        }
        let (tx, rx) = unbounded_channel();
        s.outbound = Some(tx);
        Peer { s, rx }
    }

    async fn new_service(server: &Arc<RwLock<Server>>, p: &mut Peer, t: &str, name: &str) -> (String, u32) {
        let r = call(server, &mut p.s, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":name,"size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let fid = server.read().await.service_registry[&id].file_id;
        (id, fid)
    }

    fn grant(client: &str, service: Option<&str>, perms: &[&str]) -> serde_json::Value {
        let mut d = json!({"request_id":"g","client_id":client,"permissions":perms});
        if let Some(s) = service {
            d["service_id"] = json!(s);
        }
        json!({"type":"Grant","data":d})
    }

    fn revoke(client: &str, service: Option<&str>, perms: &[&str]) -> serde_json::Value {
        let mut d = json!({"request_id":"v","client_id":client});
        if !perms.is_empty() {
            d["permissions"] = json!(perms);
        }
        if let Some(s) = service {
            d["service_id"] = json!(s);
        }
        json!({"type":"Revoke","data":d})
    }

    fn get_path(service_id: &str) -> serde_json::Value {
        json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":service_id}})
    }

    async fn join_remote(server: &Arc<RwLock<Server>>, p: &mut Peer, file_id: u32) {
        let r = call(server, &mut p.s, json!({"type":"Resync","data":{"request_id":"rs","file_id":file_id}})).await;
        assert_eq!(r["type"], "ResyncDone", "{r}");
        p.s.outbound_frames.clear();
    }

    async fn recv(p: &mut Peer) -> serde_json::Value {
        match tokio::time::timeout(Duration::from_secs(3), p.rx.recv()).await {
            Ok(Some(Outbound::Text(v))) => v,
            other => panic!("expected a text message, got {other:?}"),
        }
    }

    async fn file_id_of(server: &Arc<RwLock<Server>>, id: &str) -> u32 {
        server.read().await.service_registry[id].file_id
    }

    /// Wait until the service has moved to a file other than `from` (a rotation committed).
    async fn wait_rotated(server: &Arc<RwLock<Server>>, id: &str, from: u32) -> u32 {
        for _ in 0..100 {
            let now = file_id_of(server, id).await;
            if now != from {
                return now;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        panic!("service never left file {from}");
    }

    #[tokio::test]
    async fn an_admin_grants_a_service_and_the_client_reaches_that_service_only() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, _f1) = new_service(&server, &mut admin, &t, "one").await;
        let (two, _f2) = new_service(&server, &mut admin, &t, "two").await;
        let mut bob = peer(&server, &t, "bob", false).await;
        assert_eq!(call(&server, &mut bob.s, get_path(&one)).await["data"]["code"], "PERMISSION_DENIED");

        let ack = call(&server, &mut admin.s, grant("bob", Some(&one), &["service:write"])).await;
        assert_eq!(ack["type"], "GrantAck", "{ack}");
        assert_eq!(ack["data"]["request_id"], "g");
        let r = call(&server, &mut bob.s, get_path(&one)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert_eq!(call(&server, &mut bob.s, get_path(&two)).await["data"]["code"], "PERMISSION_DENIED");
        let events = server.read().await.audit_events().join("\n");
        assert!(events.contains("grant GRANTED"), "{events}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn only_the_right_administrators_may_change_grants() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, _f1) = new_service(&server, &mut admin, &t, "one").await;
        let (two, _f2) = new_service(&server, &mut admin, &t, "two").await;
        call(&server, &mut admin.s, grant("writer", Some(&one), &["service:write"])).await;
        call(&server, &mut admin.s, grant("svcadmin", Some(&one), &["service:admin"])).await;

        let mut writer = peer(&server, &t, "writer", false).await;
        let r = call(&server, &mut writer.s, grant("x", Some(&one), &["service:read"])).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "write is not admin: {r}");

        let mut sa = peer(&server, &t, "svcadmin", false).await;
        let r = call(&server, &mut sa.s, grant("x", Some(&one), &["service:read"])).await;
        assert_eq!(r["type"], "GrantAck", "a service admin grants on its service: {r}");
        let r = call(&server, &mut sa.s, grant("x", Some(&two), &["service:read"])).await;
        assert_eq!(r["type"], "Error", "not on another service: {r}");
        let r = call(&server, &mut sa.s, grant("x", None, &["service:read"])).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "not tenant-wide: {r}");
        let r = call(&server, &mut sa.s, revoke("x", None, &[])).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let events = server.read().await.audit_events().join("\n");
        assert!(events.contains("grant REFUSED"), "{events}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn bad_grant_requests_are_refused() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, _f) = new_service(&server, &mut admin, &t, "one").await;
        let cases = [
            (grant("bob", Some(&one), &["service:delete"]), "INVALID_REQUEST"),
            (grant("bob", Some(&one), &["tenant:admin"]), "INVALID_REQUEST"),
            (grant("bob", Some(&one), &[]), "INVALID_REQUEST"),
            (grant("", Some(&one), &["service:read"]), "INVALID_REQUEST"),
            (grant("bob", Some("no-such-service"), &["service:read"]), "NOT_FOUND"),
        ];
        for (req, code) in cases {
            let r = call(&server, &mut admin.s, req.clone()).await;
            assert_eq!(r["data"]["code"], code, "{req}: {r}");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn listing_shows_each_caller_only_what_it_may_see() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, _f) = new_service(&server, &mut admin, &t, "one").await;
        call(&server, &mut admin.s, grant("bob", Some(&one), &["service:read"])).await;
        call(&server, &mut admin.s, grant("carol", Some(&one), &["service:write"])).await;

        let list = json!({"type":"ListGrants","data":{"request_id":"l"}});
        let all = call(&server, &mut admin.s, list.clone()).await;
        assert_eq!(all["type"], "Grants", "{all}");
        let clients: Vec<&str> = all["data"]["entries"].as_array().unwrap().iter().map(|e| e["client_id"].as_str().unwrap()).collect();
        for c in ["admin", "bob", "carol"] {
            assert!(clients.contains(&c), "{c} missing: {all}");
        }

        let mut bob = peer(&server, &t, "bob", false).await;
        let mine = call(&server, &mut bob.s, list).await;
        let entries = mine["data"]["entries"].as_array().unwrap();
        assert!(!entries.is_empty() && entries.iter().all(|e| e["client_id"] == "bob"), "{mine}");
        let other = call(&server, &mut bob.s, json!({"type":"ListGrants","data":{"request_id":"l2","client_id":"carol"}})).await;
        assert_eq!(other["data"]["code"], "PERMISSION_DENIED", "{other}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn revoking_a_client_from_a_service_moves_the_file_and_drops_the_client() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, fid) = new_service(&server, &mut admin, &t, "one").await;
        call(&server, &mut admin.s, grant("bob", Some(&one), &["service:write"])).await;
        let mut bob = peer(&server, &t, "bob", false).await;
        join_remote(&server, &mut admin, fid).await;
        join_remote(&server, &mut bob, fid).await;

        let ack = call(&server, &mut admin.s, revoke("bob", Some(&one), &[])).await;
        assert_eq!(ack["type"], "GrantAck", "{ack}");
        wait_rotated(&server, &one, fid).await;

        let loss = recv(&mut bob).await;
        assert_eq!(loss["type"], "Loss", "{loss}");
        assert_eq!(loss["data"]["kind"], "REVOKED");
        let commit = recv(&mut admin).await;
        assert_eq!(commit["type"], "RotationCommit", "{commit}");
        assert_eq!(commit["data"]["reason"], "REVOCATION");
        let new_fid = commit["data"]["new_file_id"].as_u64().unwrap();
        assert_ne!(new_fid, fid as u64);

        // bob can reach neither the old file nor the new one
        assert_eq!(call(&server, &mut bob.s, get_path(&one)).await["data"]["code"], "PERMISSION_DENIED");
        let r = call(&server, &mut bob.s, json!({"type":"Resync","data":{"request_id":"r","file_id":new_fid}})).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        let events = server.read().await.audit_events().join("\n");
        assert!(events.contains("grant REVOKED"), "{events}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_revoked_client_that_only_held_a_path_still_loses_it() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, fid) = new_service(&server, &mut admin, &t, "one").await;
        call(&server, &mut admin.s, grant("bob", Some(&one), &["service:write"])).await;
        // bob is not a member of the file (never confirmed or resynced), revoke still moves the file
        let ack = call(&server, &mut admin.s, revoke("bob", Some(&one), &[])).await;
        assert_eq!(ack["type"], "GrantAck", "{ack}");
        wait_rotated(&server, &one, fid).await;
        cleanup(&t);
    }

    #[tokio::test]
    async fn revoking_someone_with_no_access_moves_nothing() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, f) = new_service(&server, &mut admin, &t, "one").await;
        let ack = call(&server, &mut admin.s, revoke("nobody", Some(&one), &[])).await;
        assert_eq!(ack["type"], "GrantAck", "{ack}");
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(file_id_of(&server, &one).await, f);
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_tenant_wide_revoke_moves_every_service_the_client_lost_and_only_those() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, f1) = new_service(&server, &mut admin, &t, "one").await;
        let (two, f2) = new_service(&server, &mut admin, &t, "two").await;
        let (three, f3) = new_service(&server, &mut admin, &t, "three").await;
        call(&server, &mut admin.s, grant("bob", None, &["service:read"])).await;
        call(&server, &mut admin.s, grant("bob", Some(&three), &["service:write"])).await;
        let ack = call(&server, &mut admin.s, revoke("bob", None, &[])).await;
        assert_eq!(ack["type"], "GrantAck", "{ack}");
        wait_rotated(&server, &one, f1).await;
        wait_rotated(&server, &two, f2).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(file_id_of(&server, &three).await, f3, "bob still has a service grant on three");
        let mut bob = peer(&server, &t, "bob", false).await;
        assert_eq!(call(&server, &mut bob.s, get_path(&one)).await["data"]["code"], "PERMISSION_DENIED");
        assert_eq!(call(&server, &mut bob.s, get_path(&three)).await["type"], "ServiceFilePath");
        cleanup(&t);
    }

    #[tokio::test]
    async fn revocation_is_not_held_up_by_the_rotation_rate_limits() {
        let t = tenant();
        let server = Arc::new(RwLock::new(
            Server::with_dev_auth(true).with_rotation_min_interval(Duration::from_secs(3600)),
        ));
        let mut admin = peer(&server, &t, "admin", true).await;
        let (one, f) = new_service(&server, &mut admin, &t, "one").await;
        call(&server, &mut admin.s, grant("bob", Some(&one), &["service:read"])).await;
        call(&server, &mut admin.s, grant("carol", Some(&one), &["service:read"])).await;
        call(&server, &mut admin.s, revoke("bob", Some(&one), &[])).await;
        let second = wait_rotated(&server, &one, f).await;
        // an hour-long spacing would refuse a normal rotation now; a revocation goes through
        call(&server, &mut admin.s, revoke("carol", Some(&one), &[])).await;
        wait_rotated(&server, &one, second).await;
        cleanup(&t);
    }
}

#[cfg(test)]
mod dev_auth_identity_tests {
    use super::*;
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("di_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(t: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    async fn login(server: &Arc<RwLock<Server>>, t: &str, extra: serde_json::Value) -> (ClientSession, serde_json::Value) {
        let mut data = json!({"tenant_id": t, "request_id":"a","protocol_version":1,"method":"api_key","key":"x"});
        for (k, v) in extra.as_object().unwrap() {
            data[k] = v.clone();
        }
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":data})).await;
        (s, r)
    }

    async fn make_service(server: &Arc<RwLock<Server>>, admin: &mut ClientSession, t: &str) -> String {
        let r = call(server, admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        r["data"]["service_id"].as_str().unwrap().to_string()
    }

    fn get_path(id: &str) -> serde_json::Value {
        json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":id}})
    }

    #[tokio::test]
    async fn authenticate_honours_the_client_id_and_echoes_it() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (s, r) = login(&server, &t, json!({"client_id":"alice@example.com"})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        assert_eq!(r["data"]["client_id"], "alice@example.com");
        assert_eq!(s.client_id.as_deref(), Some("alice@example.com"));
        cleanup(&t);
    }

    #[tokio::test]
    async fn without_a_client_id_the_session_id_is_returned_so_grants_can_name_it() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (s, r) = login(&server, &t, json!({})).await;
        assert_eq!(r["data"]["client_id"].as_str(), Some(s.session_id.as_str()), "{r}");
        assert_eq!(s.client_id.as_deref(), Some(s.session_id.as_str()));
        cleanup(&t);
    }

    #[tokio::test]
    async fn bad_client_ids_are_refused() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let long = "a".repeat(129);
        for bad in [json!(""), json!("a b"), json!("x|y"), json!("tab\there"), json!(long), json!(5), json!(["a"])] {
            let (s, r) = login(&server, &t, json!({"client_id": bad.clone()})).await;
            assert_eq!(r["type"], "Error", "{bad}: {r}");
            assert!(s.client_id.is_none() && s.token.is_none(), "{bad}: session must stay unauthenticated");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn dev_scopes_let_a_client_hold_no_grant_until_an_admin_gives_it_one_over_the_wire() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (mut admin, _) = login(&server, &t, json!({"client_id":"admin"})).await;
        let id = make_service(&server, &mut admin, &t).await;

        let (mut bob, r) = login(&server, &t, json!({"client_id":"bob","dev_scopes":[]})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        let r = call(&server, &mut bob, get_path(&id)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "a client with no grant is denied: {r}");

        let ack = call(&server, &mut admin, json!({"type":"Grant","data":{"request_id":"g","client_id":"bob","service_id":id,"permissions":["service:write"]}})).await;
        assert_eq!(ack["type"], "GrantAck", "{ack}");
        let r = call(&server, &mut bob, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");

        // a fresh session of the same client id shares the grant
        let (mut bob2, _) = login(&server, &t, json!({"client_id":"bob","dev_scopes":[]})).await;
        let r = call(&server, &mut bob2, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");

        // and it is not an administrator
        let r = call(&server, &mut bob, json!({"type":"Grant","data":{"request_id":"g2","client_id":"eve","service_id":id,"permissions":["service:read"]}})).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_session_without_a_client_id_can_be_granted_by_the_id_it_was_given() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (mut admin, _) = login(&server, &t, json!({"client_id":"admin"})).await;
        let id = make_service(&server, &mut admin, &t).await;
        let (mut anon, r) = login(&server, &t, json!({"dev_scopes":[]})).await;
        let given = r["data"]["client_id"].as_str().unwrap().to_string();
        assert_eq!(call(&server, &mut anon, get_path(&id)).await["data"]["code"], "PERMISSION_DENIED");
        call(&server, &mut admin, json!({"type":"Grant","data":{"request_id":"g","client_id":given,"service_id":id,"permissions":["service:read"]}})).await;
        assert_eq!(call(&server, &mut anon, get_path(&id)).await["type"], "ServiceFilePath");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_read_scope_reads_but_is_not_offered_a_path_and_default_is_still_admin() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let (mut admin, r) = login(&server, &t, json!({"client_id":"admin"})).await;
        assert_eq!(r["data"]["permissions"], json!(["all"]), "no dev_scopes still means admin: {r}");
        let id = make_service(&server, &mut admin, &t).await;
        assert!(server.write().await.set_trusted_local(&id, true));
        let (mut viewer, r) = login(&server, &t, json!({"client_id":"viewer","dev_scopes":["read"]})).await;
        assert_eq!(r["data"]["permissions"], json!(["read"]));
        let r = call(&server, &mut viewer, get_path(&id)).await;
        // "read" is a tenant-level scope: it opens every service for reading, not for writing
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert!(r["data"].get("path").is_none(), "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn dev_scopes_must_be_a_list_of_strings() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        for bad in [json!("admin"), json!([1, 2]), json!({"a": 1})] {
            let (s, r) = login(&server, &t, json!({"dev_scopes": bad.clone()})).await;
            assert_eq!(r["type"], "Error", "{bad}: {r}");
            assert!(s.token.is_none());
        }
        cleanup(&t);
    }

}

#[cfg(test)]
mod set_local_mapping_tests {
    use super::*;
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("sl_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(t: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    async fn login(server: &Arc<RwLock<Server>>, t: &str, id: &str, scopes: Option<serde_json::Value>) -> ClientSession {
        let mut data = json!({"tenant_id": t, "request_id":"a","protocol_version":1,"method":"api_key","key":"x", "client_id": id});
        if let Some(sc) = scopes {
            data["dev_scopes"] = sc;
        }
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":data})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    fn set(id: &str, enabled: bool) -> serde_json::Value {
        json!({"type":"SetLocalMapping","data":{"request_id":"m","service_id":id,"enabled":enabled}})
    }

    fn get_path(id: &str) -> serde_json::Value {
        json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":id}})
    }

    async fn setup() -> (Arc<RwLock<Server>>, String, ClientSession, String) {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = login(&server, &t, "admin", None).await;
        let r = call(&server, &mut admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        (server, t, admin, id)
    }

    #[tokio::test]
    async fn a_service_starts_closed_and_an_admin_opens_it_over_the_wire() {
        let (server, t, mut admin, id) = setup().await;
        let r = call(&server, &mut admin, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert!(r["data"].get("path").is_none(), "closed by default, even for an administrator: {r}");

        let ack = call(&server, &mut admin, set(&id, true)).await;
        assert_eq!(ack["type"], "LocalMappingAck", "{ack}");
        assert_eq!((ack["data"]["enabled"].as_bool(), ack["data"]["request_id"].as_str()), (Some(true), Some("m")));
        let r = call(&server, &mut admin, get_path(&id)).await;
        assert!(r["data"]["path"].is_string() && r["data"]["nonce_path"].is_string(), "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn closing_stops_the_offers() {
        let (server, t, mut admin, id) = setup().await;
        call(&server, &mut admin, set(&id, true)).await;
        assert!(call(&server, &mut admin, get_path(&id)).await["data"]["path"].is_string());
        let ack = call(&server, &mut admin, set(&id, false)).await;
        assert_eq!(ack["data"]["enabled"], false, "{ack}");
        assert!(call(&server, &mut admin, get_path(&id)).await["data"].get("path").is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn only_a_service_admin_may_change_it() {
        let (server, t, mut admin, id) = setup().await;
        call(&server, &mut admin, json!({"type":"Grant","data":{"request_id":"g","client_id":"writer","service_id":id,"permissions":["service:write"]}})).await;
        let mut writer = login(&server, &t, "writer", Some(json!([]))).await;
        let r = call(&server, &mut writer, set(&id, true)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        assert!(!server.read().await.service_registry[&id].trusted_local, "a refused request changes nothing");
        // a client with no grant at all is stopped by the guard
        let mut nobody = login(&server, &t, "nobody", Some(json!([]))).await;
        let r = call(&server, &mut nobody, set(&id, true)).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "{r}");
        assert!(!server.read().await.service_registry[&id].trusted_local);
        let events = server.read().await.audit_events().join("\n");
        assert!(events.contains("local mapping REFUSED"), "{events}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn unknown_services_and_other_tenants_are_refused() {
        let (server, t, mut admin, id) = setup().await;
        let r = call(&server, &mut admin, set("no-such-service", true)).await;
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let other = tenant();
        let mut mallory = login(&server, &other, "mallory", None).await;
        let r = call(&server, &mut mallory, set(&id, true)).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert!(!server.read().await.service_registry[&id].trusted_local);
        cleanup(&t);
        cleanup(&other);
    }

    #[tokio::test]
    async fn opening_is_audited_and_the_offer_needs_a_write_grant_as_before() {
        let (server, t, mut admin, id) = setup().await;
        call(&server, &mut admin, set(&id, true)).await;
        call(&server, &mut admin, json!({"type":"Grant","data":{"request_id":"g","client_id":"viewer","service_id":id,"permissions":["service:read"]}})).await;
        let mut viewer = login(&server, &t, "viewer", Some(json!([]))).await;
        let r = call(&server, &mut viewer, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert!(r["data"].get("path").is_none(), "an open service still offers a path only to writers: {r}");
        let events = server.read().await.audit_events().join("\n");
        assert!(events.contains("local mapping OPENED"), "{events}");
        cleanup(&t);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::persist::{GrantOp, OpKind, Persistence};
    use crate::StateReport;
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("pe_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(t: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    async fn login(server: &Arc<RwLock<Server>>, t: &str, id: &str, scopes: Option<serde_json::Value>) -> ClientSession {
        let mut data = json!({"tenant_id": t, "request_id":"a","protocol_version":1,"method":"api_key","key":"x", "client_id": id});
        if let Some(sc) = scopes {
            data["dev_scopes"] = sc;
        }
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":data})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    fn get_path(id: &str) -> serde_json::Value {
        json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":id}})
    }

    /// A server whose state lives in `dir`.
    fn server_on(dir: &std::path::Path) -> (Arc<RwLock<Server>>, StateReport) {
        let mut s = Server::with_dev_auth(true);
        let report = s.enable_persistence(dir).unwrap();
        (Arc::new(RwLock::new(s)), report)
    }

    async fn create(server: &Arc<RwLock<Server>>, admin: &mut ClientSession, t: &str, name: &str) -> String {
        let r = call(server, admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":name,"size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        r["data"]["service_id"].as_str().unwrap().to_string()
    }

    async fn grant(server: &Arc<RwLock<Server>>, admin: &mut ClientSession, client: &str, svc: &str, perm: &str) {
        let r = call(server, admin, json!({"type":"Grant","data":{"request_id":"g","client_id":client,"service_id":svc,"permissions":[perm]}})).await;
        assert_eq!(r["type"], "GrantAck", "{r}");
    }

    #[tokio::test]
    async fn services_and_grants_survive_a_restart_that_never_shut_down_cleanly() {
        let t = tenant();
        let state = tempfile::tempdir().unwrap();
        let (id, file_id) = {
            let (server, report) = server_on(state.path());
            assert_eq!(report, StateReport::default(), "a fresh state directory loads nothing");
            let mut admin = login(&server, &t, "admin", None).await;
            let id = create(&server, &mut admin, &t, "svc").await;
            grant(&server, &mut admin, "bob", &id, "service:write").await;
            let ack = call(&server, &mut admin, json!({"type":"SetLocalMapping","data":{"request_id":"m","service_id":id,"enabled":true}})).await;
            assert_eq!(ack["type"], "LocalMappingAck");
            let file_id = server.read().await.service_registry[&id].file_id;
            (id, file_id)
            // `server` is dropped here: no shutdown hook runs, as in a power cut
        };

        let (server, report) = server_on(state.path());
        assert_eq!((report.services_loaded, report.services_missing_file, report.grant_ops_skipped), (1, 0, 0), "{report:?}");
        assert!(report.grant_ops_replayed >= 2, "{report:?}");
        {
            let g = server.read().await;
            let rec = &g.service_registry[&id];
            assert_eq!((rec.file_id, rec.trusted_local), (file_id, true));
            assert!(g.next_file_id > file_id, "new files never reuse an id");
        }
        // bob (no tenant-level permissions) still has exactly the grant he was given
        let mut bob = login(&server, &t, "bob", Some(json!([]))).await;
        let r = call(&server, &mut bob, get_path(&id)).await;
        assert_eq!(r["type"], "ServiceFilePath", "{r}");
        assert!(r["data"]["path"].is_string(), "bob had write on an open service: {r}");
        // and a stranger still has none
        let mut eve = login(&server, &t, "eve", Some(json!([]))).await;
        assert_eq!(call(&server, &mut eve, get_path(&id)).await["data"]["code"], "PERMISSION_DENIED");
        // the creator is still its administrator
        let mut admin = login(&server, &t, "admin", Some(json!([]))).await;
        assert_eq!(call(&server, &mut admin, get_path(&id)).await["type"], "ServiceFilePath");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_revoke_is_still_in_force_after_a_restart() {
        let t = tenant();
        let state = tempfile::tempdir().unwrap();
        let id = {
            let (server, _) = server_on(state.path());
            let mut admin = login(&server, &t, "admin", None).await;
            let id = create(&server, &mut admin, &t, "svc").await;
            grant(&server, &mut admin, "bob", &id, "service:read").await;
            let r = call(&server, &mut admin, json!({"type":"Revoke","data":{"request_id":"v","client_id":"bob","service_id":id}})).await;
            assert_eq!(r["type"], "GrantAck", "{r}");
            id
        };
        let (server, _) = server_on(state.path());
        let mut bob = login(&server, &t, "bob", Some(json!([]))).await;
        assert_eq!(call(&server, &mut bob, get_path(&id)).await["data"]["code"], "PERMISSION_DENIED", "revoked access must not come back");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_rotation_is_what_the_restarted_server_serves() {
        let t = tenant();
        let state = tempfile::tempdir().unwrap();
        let (id, old_path, new_path, new_fid) = {
            let (server, _) = server_on(state.path());
            let mut admin = login(&server, &t, "admin", None).await;
            let id = create(&server, &mut admin, &t, "svc").await;
            grant(&server, &mut admin, "bob", &id, "service:read").await;
            let old_path = server.read().await.service_registry[&id].file_path.clone();
            let old_fid = server.read().await.service_registry[&id].file_id;
            // revoking bob rotates the service
            call(&server, &mut admin, json!({"type":"Revoke","data":{"request_id":"v","client_id":"bob","service_id":id}})).await;
            for _ in 0..100 {
                if server.read().await.service_registry[&id].file_id != old_fid {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            }
            let g = server.read().await;
            let rec = &g.service_registry[&id];
            assert_ne!(rec.file_id, old_fid);
            (id.clone(), old_path, rec.file_path.clone(), rec.file_id)
        };
        assert_ne!(old_path, new_path);
        let (server, report) = server_on(state.path());
        assert_eq!(report.services_loaded, 1, "{report:?}");
        let g = server.read().await;
        let rec = &g.service_registry[&id];
        assert_eq!((rec.file_path.as_str(), rec.file_id), (new_path.as_str(), new_fid));
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_service_whose_file_vanished_is_skipped_and_counted() {
        let t = tenant();
        let state = tempfile::tempdir().unwrap();
        let path = {
            let (server, _) = server_on(state.path());
            let mut admin = login(&server, &t, "admin", None).await;
            let id = create(&server, &mut admin, &t, "svc").await;
            let p = server.read().await.service_registry[&id].file_path.clone();
            p
        };
        std::fs::remove_file(&path).unwrap();
        let (server, report) = server_on(state.path());
        assert_eq!((report.services_loaded, report.services_missing_file), (0, 1), "{report:?}");
        assert!(server.read().await.service_registry.is_empty());
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_stored_grant_that_cannot_be_applied_is_skipped_not_fatal() {
        let state = tempfile::tempdir().unwrap();
        {
            let p = Persistence::open(state.path()).unwrap();
            p.append_grant_op(&GrantOp {
                kind: OpKind::Grant,
                tenant: "t".into(),
                client: "bob".into(),
                service: Some("svc".into()),
                permissions: vec!["service:from-the-future".into()],
            })
            .unwrap();
        }
        let (_server, report) = server_on(state.path());
        assert_eq!((report.grant_ops_replayed, report.grant_ops_skipped), (0, 1), "{report:?}");
    }

    /// Fail closed: a revoke the server cannot record is refused and NOT applied.
    #[tokio::test]
    async fn a_revoke_that_cannot_be_recorded_is_refused_and_changes_nothing() {
        let t = tenant();
        let mut s = Server::with_dev_auth(true);
        // writes: 1 creator grant, 2 service row, 3 bob's grant; the 4th (the revoke) fails
        s.use_persistence(
            Persistence::open_with(persistant::Config::Faulty {
                inner: Box::new(persistant::Config::Memory),
                faults: persistant::Faults::fail_writes_after(3),
            })
            .unwrap(),
        )
        .unwrap();
        let server = Arc::new(RwLock::new(s));
        let mut admin = login(&server, &t, "admin", None).await;
        let id = create(&server, &mut admin, &t, "svc").await;
        grant(&server, &mut admin, "bob", &id, "service:read").await;

        let old_fid = server.read().await.service_registry[&id].file_id;
        let r = call(&server, &mut admin, json!({"type":"Revoke","data":{"request_id":"v","client_id":"bob","service_id":id}})).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INTERNAL_ERROR", "{r}");
        assert!(server.read().await.grants.allows(&t, "bob", &id, crate::server::grants::Action::Read), "the failed revoke must not take effect");
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert_eq!(server.read().await.service_registry[&id].file_id, old_fid, "and must not rotate the file");
        let events = server.read().await.audit_events().join("\\n");
        assert!(events.contains("grant REFUSED"), "{events}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn closing_local_mapping_that_cannot_be_recorded_leaves_it_open_and_says_so() {
        let t = tenant();
        let mut s = Server::with_dev_auth(true);
        // writes: 1 creator grant, 2 service row, 3 opening; closing (4th) fails
        s.use_persistence(
            Persistence::open_with(persistant::Config::Faulty {
                inner: Box::new(persistant::Config::Memory),
                faults: persistant::Faults::fail_writes_after(3),
            })
            .unwrap(),
        )
        .unwrap();
        let server = Arc::new(RwLock::new(s));
        let mut admin = login(&server, &t, "admin", None).await;
        let id = create(&server, &mut admin, &t, "svc").await;
        let open = call(&server, &mut admin, json!({"type":"SetLocalMapping","data":{"request_id":"m","service_id":id,"enabled":true}})).await;
        assert_eq!(open["type"], "LocalMappingAck", "{open}");
        let close = call(&server, &mut admin, json!({"type":"SetLocalMapping","data":{"request_id":"m2","service_id":id,"enabled":false}})).await;
        assert_eq!(close["type"], "Error", "{close}");
        assert!(server.read().await.service_registry[&id].trusted_local, "the in-memory state matches what is stored");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_service_that_cannot_be_recorded_is_not_created() {
        let t = tenant();
        let mut s = Server::with_dev_auth(true);
        // the first write (the creator's grant) fails
        s.use_persistence(
            Persistence::open_with(persistant::Config::Faulty {
                inner: Box::new(persistant::Config::Memory),
                faults: persistant::Faults::fail_writes_after(0),
            })
            .unwrap(),
        )
        .unwrap();
        let server = Arc::new(RwLock::new(s));
        let mut admin = login(&server, &t, "admin", None).await;
        let r = call(&server, &mut admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert!(server.read().await.service_registry.is_empty());
        let leftovers = std::fs::read_dir(format!("tenant_{t}")).map(|d| d.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("service_")).count()).unwrap_or(0);
        assert_eq!(leftovers, 0, "no orphan file");
        cleanup(&t);
    }

    #[tokio::test]
    async fn without_persistence_nothing_is_written_and_nothing_changes() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = login(&server, &t, "admin", None).await;
        let id = create(&server, &mut admin, &t, "svc").await;
        grant(&server, &mut admin, "bob", &id, "service:read").await;
        assert!(server.read().await.grants.allows(&t, "bob", &id, crate::server::grants::Action::Read));
        cleanup(&t);
    }
}

#[cfg(test)]
mod delete_service_tests {
    use super::*;
    use crate::persist::Persistence;
    use crate::server::forwarding::Outbound;
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    struct Peer {
        s: ClientSession,
        rx: UnboundedReceiver<Outbound>,
    }

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("ds_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(t: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    async fn peer(server: &Arc<RwLock<Server>>, t: &str, id: &str, scopes: Option<serde_json::Value>) -> Peer {
        let mut data = json!({"tenant_id": t, "request_id":"a","protocol_version":1,"method":"api_key","key":"x", "client_id": id});
        if let Some(sc) = scopes {
            data["dev_scopes"] = sc;
        }
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":data})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        let (tx, rx) = unbounded_channel();
        s.outbound = Some(tx);
        Peer { s, rx }
    }

    async fn create(server: &Arc<RwLock<Server>>, p: &mut Peer, t: &str, name: &str) -> (String, u32, String) {
        let r = call(server, &mut p.s, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":name,"size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let g = server.read().await;
        let rec = &g.service_registry[&id];
        (id.clone(), rec.file_id, rec.file_path.clone())
    }

    fn delete(t: &str, name: &str) -> serde_json::Value {
        json!({"type":"DeleteService","data":{"tenant_id":t,"service_name":name}})
    }

    async fn join_remote(server: &Arc<RwLock<Server>>, p: &mut Peer, file_id: u32) {
        let r = call(server, &mut p.s, json!({"type":"Resync","data":{"request_id":"rs","file_id":file_id}})).await;
        assert_eq!(r["type"], "ResyncDone", "{r}");
        p.s.outbound_frames.clear();
    }

    #[tokio::test]
    async fn deleting_a_service_removes_it_everywhere_and_says_so_truthfully() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", None).await;
        let (id, fid, path) = create(&server, &mut admin, &t, "doomed").await;
        call(&server, &mut admin.s, json!({"type":"Grant","data":{"request_id":"g","client_id":"bob","service_id":id,"permissions":["service:read"]}})).await;
        assert!(std::path::Path::new(&path).exists());

        let r = call(&server, &mut admin.s, delete(&t, "doomed")).await;
        assert_eq!(r["type"], "Result", "{r}");
        assert_eq!(r["data"]["success"], true, "{r}");

        let g = server.read().await;
        assert!(!g.service_registry.contains_key(&id), "gone from the registry");
        assert!(g.tenants[&t].get_service_by_name("doomed").is_none(), "gone from the tenant");
        assert!(g.retired_files.contains(&fid), "its file id is retired");
        assert!(!std::path::Path::new(&path).exists(), "its file is removed");
        assert!(g.grants.list(&t, None, Some(&id)).iter().all(|e| e.service_id.is_none()), "its grants are dropped");
        drop(g);
        // and the wire agrees
        let r = call(&server, &mut admin.s, json!({"type":"GetService","data":{"tenant_id":t,"service_name":"doomed"}})).await;
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let r = call(&server, &mut admin.s, json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":id}})).await;
        assert_eq!(r["data"]["code"], "NOT_FOUND", "{r}");
        let r = call(&server, &mut admin.s, json!({"type":"Resync","data":{"request_id":"r","file_id":fid}})).await;
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn members_of_a_deleted_service_are_told_and_dropped() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", None).await;
        let (_id, fid, _p) = create(&server, &mut admin, &t, "svc").await;
        join_remote(&server, &mut admin, fid).await;
        call(&server, &mut admin.s, delete(&t, "svc")).await;
        let m = tokio::time::timeout(Duration::from_secs(2), admin.rx.recv()).await;
        let Ok(Some(Outbound::Text(v))) = m else { panic!("expected a Loss, got {m:?}") };
        assert_eq!((v["type"].as_str(), v["data"]["kind"].as_str()), (Some("Loss"), Some("REVOKED")), "{v}");
        assert_eq!(v["data"]["file_id"], fid);
        assert_eq!(server.read().await.forwarder.member_count(fid), 0);
        cleanup(&t);
    }

    #[tokio::test]
    async fn only_a_service_admin_may_delete_and_only_that_service() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", None).await;
        let (a, _, _) = create(&server, &mut admin, &t, "a").await;
        let (b, _, _) = create(&server, &mut admin, &t, "b").await;
        for (who, svc, perm) in [("writer", &a, "service:write"), ("adminb", &b, "service:admin")] {
            call(&server, &mut admin.s, json!({"type":"Grant","data":{"request_id":"g","client_id":who,"service_id":svc,"permissions":[perm]}})).await;
        }
        let mut writer = peer(&server, &t, "writer", Some(json!([]))).await;
        assert_eq!(call(&server, &mut writer.s, delete(&t, "a")).await["data"]["code"], "PERMISSION_DENIED");
        let mut adminb = peer(&server, &t, "adminb", Some(json!([]))).await;
        assert_eq!(call(&server, &mut adminb.s, delete(&t, "a")).await["data"]["code"], "PERMISSION_DENIED", "admin of b cannot delete a");
        assert_eq!(call(&server, &mut adminb.s, delete(&t, "b")).await["type"], "Result", "admin of b deletes b");
        let g = server.read().await;
        assert!(g.service_registry.contains_key(&a) && !g.service_registry.contains_key(&b));
        drop(g);
        cleanup(&t);
    }

    #[tokio::test]
    async fn deleting_one_service_leaves_the_others_and_tenant_wide_grants() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", None).await;
        let (a, _, _) = create(&server, &mut admin, &t, "a").await;
        let (b, _, _) = create(&server, &mut admin, &t, "b").await;
        call(&server, &mut admin.s, json!({"type":"Grant","data":{"request_id":"g","client_id":"bob","permissions":["service:read"]}})).await;
        call(&server, &mut admin.s, json!({"type":"Grant","data":{"request_id":"g","client_id":"bob","service_id":a,"permissions":["service:write"]}})).await;
        call(&server, &mut admin.s, delete(&t, "a")).await;
        let g = server.read().await;
        assert!(g.service_registry.contains_key(&b));
        assert!(g.grants.allows(&t, "bob", &b, crate::server::grants::Action::Read), "tenant-wide grant remains");
        assert!(g.grants.allows(&t, "admin", &b, crate::server::grants::Action::Admin), "other services keep their admin");
        drop(g);
        cleanup(&t);
    }

    #[tokio::test]
    async fn deleting_twice_or_an_unknown_name_is_not_found() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = peer(&server, &t, "admin", None).await;
        create(&server, &mut admin, &t, "svc").await;
        assert_eq!(call(&server, &mut admin.s, delete(&t, "svc")).await["type"], "Result");
        assert_eq!(call(&server, &mut admin.s, delete(&t, "svc")).await["data"]["code"], "NOT_FOUND");
        assert_eq!(call(&server, &mut admin.s, delete(&t, "never")).await["data"]["code"], "NOT_FOUND");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_service_in_the_middle_of_a_rotation_is_not_deleted() {
        let t = tenant();
        let server = Arc::new(RwLock::new(
            Server::with_dev_auth(true).with_drain_timeout(Duration::from_secs(5)).with_rotation_min_interval(Duration::ZERO),
        ));
        let mut admin = peer(&server, &t, "admin", None).await;
        let (id, fid, _p) = create(&server, &mut admin, &t, "svc").await;
        join_remote(&server, &mut admin, fid).await;
        let acc = call(&server, &mut admin.s, json!({"type":"RotateRequest","data":{"request_id":"q","service_id":id,"reason":"RESIZE","new_size":8192}})).await;
        assert_eq!(acc["type"], "RotationAccepted", "{acc}");
        let r = call(&server, &mut admin.s, delete(&t, "svc")).await;
        assert_eq!(r["data"]["code"], "RATE_LIMITED", "{r}");
        assert!(server.read().await.service_registry.contains_key(&id));
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_deletion_survives_a_restart() {
        let t = tenant();
        let state = tempfile::tempdir().unwrap();
        {
            let mut s = Server::with_dev_auth(true);
            s.enable_persistence(state.path()).unwrap();
            let server = Arc::new(RwLock::new(s));
            let mut admin = peer(&server, &t, "admin", None).await;
            create(&server, &mut admin, &t, "keep").await;
            create(&server, &mut admin, &t, "doomed").await;
            assert_eq!(call(&server, &mut admin.s, delete(&t, "doomed")).await["type"], "Result");
        }
        let mut s = Server::with_dev_auth(true);
        let report = s.enable_persistence(state.path()).unwrap();
        assert_eq!(report.services_loaded, 1, "only the surviving service comes back: {report:?}");
        assert!(s.service_registry.values().all(|r| r.service_name == "keep"));
        cleanup(&t);
    }

    /// Fail closed: a deletion that cannot be recorded is refused and the service stays usable.
    #[tokio::test]
    async fn a_deletion_that_cannot_be_recorded_is_refused_and_changes_nothing() {
        let t = tenant();
        let mut s = Server::with_dev_auth(true);
        // writes: 1 creator grant, 2 service row; the deletion (3rd) fails
        s.use_persistence(
            Persistence::open_with(persistant::Config::Faulty {
                inner: Box::new(persistant::Config::Memory),
                faults: persistant::Faults::fail_writes_after(2),
            })
            .unwrap(),
        )
        .unwrap();
        let server = Arc::new(RwLock::new(s));
        let mut admin = peer(&server, &t, "admin", None).await;
        let (id, _fid, path) = create(&server, &mut admin, &t, "svc").await;
        let r = call(&server, &mut admin.s, delete(&t, "svc")).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INTERNAL_ERROR", "{r}");
        assert!(server.read().await.service_registry.contains_key(&id));
        assert!(std::path::Path::new(&path).exists(), "the file is still there");
        cleanup(&t);
    }
}

#[cfg(test)]
mod wire_error_code_tests {
    use super::*;
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn wire_spelling(code: &str) -> bool {
        commy_wire::ErrorCode::ALL.iter().any(|c| c.as_str() == code)
    }

    /// Every error the SDK protocol can produce uses the one spelling `commy-wire` allows, and the
    /// ones that came with a `request_id` echo it.
    #[tokio::test]
    async fn every_error_reply_uses_the_wire_spelling_and_echoes_the_request_id() {
        let t = format!("ec_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut anon = ClientSession::new();
        let mut admin = ClientSession::new();
        call(&server, &mut admin, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x","client_id":"admin"}})).await;
        let r = call(&server, &mut admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        let mut nobody = ClientSession::new();
        call(&server, &mut nobody, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x","client_id":"nobody","dev_scopes":[]}})).await;
        let mut other = ClientSession::new();
        let other_t = format!("ec_{}", uuid::Uuid::new_v4().simple());
        call(&server, &mut other, json!({"type":"Authenticate","data":{"tenant_id":other_t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x"}})).await;

        // 0 = anonymous, 1 = admin, 2 = a client with no grant, 3 = a client of another tenant
        let cases: Vec<(&str, usize, serde_json::Value)> = vec![
            ("unauthenticated", 0, json!({"type":"CreateService","data":{"request_id":"r1","tenant_id":t,"service_name":"x","size":1}})),
            ("unknown type", 1, json!({"type":"NoSuchMessage","data":{"request_id":"r2"}})),
            ("bad size", 1, json!({"type":"CreateService","data":{"request_id":"r3","tenant_id":t,"service_name":"x","size":0}})),
            ("bad tenant id", 1, json!({"type":"CreateService","data":{"request_id":"r4","tenant_id":"../x","service_name":"x","size":1}})),
            ("other tenant's service", 3, json!({"type":"GetServiceFilePath","data":{"request_id":"r5","service_id":id}})),
            ("no access", 2, json!({"type":"GetServiceFilePath","data":{"request_id":"r6","service_id":id}})),
            ("unknown service", 1, json!({"type":"GetService","data":{"request_id":"r7","tenant_id":t,"service_name":"nope"}})),
            ("cross tenant create", 1, json!({"type":"CreateService","data":{"request_id":"r8","tenant_id":other_t,"service_name":"x","size":1}})),
            ("delete unknown", 1, json!({"type":"DeleteService","data":{"request_id":"r9","tenant_id":t,"service_name":"nope"}})),
            ("delete tenant missing", 1, json!({"type":"DeleteTenant","data":{"request_id":"r10","tenant_id":"absent_tenant"}})),
        ];
        for (name, who, msg) in cases {
            let rid = msg["data"]["request_id"].as_str().unwrap().to_string();
            let session = match who {
                0 => &mut anon,
                1 => &mut admin,
                2 => &mut nobody,
                _ => &mut other,
            };
            let r = call(&server, session, msg).await;
            assert_eq!(r["type"], "Error", "{name}: {r}");
            let code = r["data"]["code"].as_str().unwrap_or("");
            assert!(wire_spelling(code), "{name}: '{code}' is not a commy-wire error code: {r}");
            assert_eq!(r["data"]["request_id"], rid, "{name}: the request_id must be echoed: {r}");
        }
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
        let _ = std::fs::remove_dir_all(format!("tenant_{other_t}"));
    }

    /// A success reply to an SDK-protocol request that carried a `request_id` echoes it too (a
    /// `Result` used to carry a fresh random id instead).
    #[tokio::test]
    async fn success_replies_echo_the_request_id() {
        let t = format!("er_{}", uuid::Uuid::new_v4().simple());
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut admin = ClientSession::new();
        call(&server, &mut admin, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x","client_id":"admin"}})).await;
        let steps = vec![
            ("CreateService", "Service", json!({"type":"CreateService","data":{"request_id":"s1","tenant_id":t,"service_name":"svc","size":4096}})),
            ("GetService", "Service", json!({"type":"GetService","data":{"request_id":"s2","tenant_id":t,"service_name":"svc"}})),
            ("DeleteService", "Result", json!({"type":"DeleteService","data":{"request_id":"s3","tenant_id":t,"service_name":"svc"}})),
            ("CreateTenant", "TenantResult", json!({"type":"CreateTenant","data":{"request_id":"s4","tenant_id":t,"tenant_name":"New"}})),
            ("DeleteTenant", "Result", json!({"type":"DeleteTenant","data":{"request_id":"s5","tenant_id":t}})),
            ("Disconnect", "Result", json!({"type":"Disconnect","data":{"request_id":"s6"}})),
        ];
        for (name, reply_type, msg) in steps {
            let rid = msg["data"]["request_id"].as_str().unwrap().to_string();
            let r = call(&server, &mut admin, msg).await;
            assert_eq!(r["type"], reply_type, "{name}: {r}");
            assert_eq!(r["data"]["request_id"], rid, "{name}: the request_id must be echoed: {r}");
        }
        // without a request_id nothing is invented for Service, and a Result keeps its own
        let r = call(&server, &mut admin, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc2","size":4096}})).await;
        assert!(r["data"].get("request_id").is_none(), "{r}");
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    #[tokio::test]
    async fn the_error_reply_parses_as_a_wire_control_message() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut anon = ClientSession::new();
        let r = call(&server, &mut anon, json!({"type":"GetService","data":{"request_id":"q","tenant_id":"t","service_name":"s"}})).await;
        let parsed: commy_wire::ControlMessage = serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("{r}: {e}"));
        match parsed {
            commy_wire::ControlMessage::Error(e) => {
                assert_eq!(e.code, commy_wire::ErrorCode::Unauthenticated);
                assert_eq!(e.request_id.as_deref(), Some("q"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn the_old_camel_case_spellings_are_gone_from_replies() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut anon = ClientSession::new();
        let r = call(&server, &mut anon, json!({"type":"NoSuchMessage","data":{}})).await;
        let code = r["data"]["code"].as_str().unwrap();
        for legacy in ["NotFound", "Unauthorized", "InvalidRequest", "InternalError", "PARSE_ERROR", "INVALID_JSON"] {
            assert_ne!(code, legacy);
        }
    }
}

#[cfg(test)]
mod audit_log_tests {
    use super::*;
    use crate::persist::Persistence;
    use serde_json::json;

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    fn tenant() -> String {
        format!("au_{}", uuid::Uuid::new_v4().simple())
    }

    fn cleanup(t: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    async fn admin(server: &Arc<RwLock<Server>>, t: &str) -> ClientSession {
        let mut s = ClientSession::new();
        let r = call(server, &mut s, json!({"type":"Authenticate","data":{"tenant_id":t,"request_id":"a","protocol_version":1,"method":"api_key","key":"x","client_id":"admin"}})).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        s
    }

    async fn create(server: &Arc<RwLock<Server>>, a: &mut ClientSession, t: &str, name: &str) -> String {
        let r = call(server, a, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":name,"size":4096}})).await;
        r["data"]["service_id"].as_str().unwrap().to_string()
    }

    fn lines(server: &Server) -> Vec<String> {
        server.audit_history(None, usize::MAX).unwrap().into_iter().map(|(_, e)| e.line).collect()
    }

    #[tokio::test]
    async fn admin_events_reach_the_durable_log_and_survive_a_restart() {
        let t = tenant();
        let state = tempfile::tempdir().unwrap();
        {
            let mut s = Server::with_dev_auth(true);
            s.enable_persistence(state.path()).unwrap();
            let server = Arc::new(RwLock::new(s));
            let mut a = admin(&server, &t).await;
            let id = create(&server, &mut a, &t, "svc").await;
            call(&server, &mut a, json!({"type":"Grant","data":{"request_id":"g","client_id":"bob","service_id":id,"permissions":["service:read"]}})).await;
            call(&server, &mut a, json!({"type":"SetLocalMapping","data":{"request_id":"m","service_id":id,"enabled":true}})).await;
            call(&server, &mut a, json!({"type":"Revoke","data":{"request_id":"v","client_id":"bob","service_id":id}})).await;
            call(&server, &mut a, json!({"type":"DeleteService","data":{"tenant_id":t,"service_name":"svc"}})).await;
            let l = lines(&*server.read().await);
            for want in ["server STARTED", "grant GRANTED", "local mapping OPENED", "grant REVOKED", "service DELETED"] {
                assert!(l.iter().any(|x| x.contains(want)), "{want} missing from {l:#?}");
            }
        }
        let mut s = Server::with_dev_auth(true);
        s.enable_persistence(state.path()).unwrap();
        let l = lines(&s);
        assert!(l.iter().any(|x| x.contains("grant REVOKED") && x.contains("bob") && x.contains("admin")), "who and what are kept: {l:#?}");
        assert_eq!(l.iter().filter(|x| x.contains("server STARTED")).count(), 2, "one per start: {l:#?}");
        let hist = s.audit_history(None, usize::MAX).unwrap();
        assert!(hist.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1.ts_ms <= w[1].1.ts_ms));
        cleanup(&t);
    }

    /// The audit log is best effort: its failure is counted and visible and never blocks the change.
    #[tokio::test]
    async fn a_failing_audit_store_is_counted_and_does_not_block_grant_or_revoke() {
        let t = tenant();
        let mut s = Server::with_dev_auth(true);
        s.use_persistence(
            Persistence::open_with_audit(
                persistant::Config::Memory,
                persistant::Config::Faulty {
                    inner: Box::new(persistant::Config::Memory),
                    faults: persistant::Faults::fail_writes_after(0),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(s.audit_failures() >= 1, "even the start-up entry could not be written");
        let before = s.audit_failures();
        let server = Arc::new(RwLock::new(s));
        let mut a = admin(&server, &t).await;
        let id = create(&server, &mut a, &t, "svc").await;
        let g = call(&server, &mut a, json!({"type":"Grant","data":{"request_id":"g","client_id":"bob","service_id":id,"permissions":["service:read"]}})).await;
        assert_eq!(g["type"], "GrantAck", "a grant is not blocked by a failing audit log: {g}");
        assert!(server.read().await.grants.allows(&t, "bob", &id, crate::server::grants::Action::Read));
        let v = call(&server, &mut a, json!({"type":"Revoke","data":{"request_id":"v","client_id":"bob","service_id":id}})).await;
        assert_eq!(v["type"], "GrantAck", "a revoke is not blocked either: {v}");
        assert!(!server.read().await.grants.allows(&t, "bob", &id, crate::server::grants::Action::Read));
        let g = server.read().await;
        assert!(g.audit_failures() > before, "every lost event is counted ({} -> {})", before, g.audit_failures());
        assert!(g.audit_events().iter().any(|l| l.contains("grant REVOKED")), "the in-memory trail still has it");
        assert!(g.audit_history(None, 10).unwrap().is_empty(), "nothing reached the failing store");
        drop(g);
        cleanup(&t);
    }

    #[tokio::test]
    async fn the_durable_log_keeps_only_the_configured_number_of_newest_events() {
        let t = tenant();
        let mut s = Server::with_dev_auth(true);
        s.use_persistence(Persistence::open_with(persistant::Config::Memory).unwrap()).unwrap();
        s.set_audit_limit(5);
        for i in 0..20 {
            s.audit(&format!("event {i}"));
        }
        let l = lines(&s);
        assert_eq!(l, ["event 15", "event 16", "event 17", "event 18", "event 19"]);
        cleanup(&t);
    }

    #[tokio::test]
    async fn without_persistence_there_is_no_durable_history_and_no_failures() {
        let s = Server::with_dev_auth(true);
        s.audit("something happened");
        assert!(s.audit_history(None, 10).unwrap().is_empty());
        assert_eq!(s.audit_failures(), 0);
        assert!(s.audit_events().iter().any(|l| l == "something happened"));
    }
}

#[cfg(test)]
mod credential_verifier_tests {
    use super::*;
    use crate::server::verify::StaticApiKeys;
    use serde_json::json;

    fn tenant() -> String {
        format!("cv_{}", uuid::Uuid::new_v4().simple())
    }

    async fn call(server: &Arc<RwLock<Server>>, s: &mut ClientSession, v: serde_json::Value) -> serde_json::Value {
        handle_sdk_message(v, s, Arc::clone(server)).await.unwrap()
    }

    /// A production-style server: no development authentication, two API keys.
    fn production(t: &str) -> Arc<RwLock<Server>> {
        Arc::new(RwLock::new(
            Server::with_dev_auth(false).with_verifier(Arc::new(
                StaticApiKeys::new()
                    .with_key("k-alice-secret", t, "alice@example.com", &["service:read"])
                    .with_key("k-root-secret", t, "root", &["admin", "service:create"])
                    .with_key("k-other-secret", "other_tenant", "mallory", &["admin"]),
            )),
        ))
    }

    fn auth(t: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut data = json!({"request_id":"a1","protocol_version":1,"tenant_id":t,"method":"api_key","key":"k-alice-secret"});
        for (k, v) in extra.as_object().unwrap() {
            data[k] = v.clone();
        }
        json!({"type":"Authenticate","data":data})
    }

    fn cleanup(t: &str) {
        let _ = std::fs::remove_dir_all(format!("tenant_{t}"));
    }

    #[tokio::test]
    async fn a_valid_key_authenticates_as_the_credentials_subject() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({}))).await;
        assert_eq!(r["type"], "AuthenticationResult", "{r}");
        assert_eq!(r["data"]["success"], true, "{r}");
        assert_eq!(r["data"]["request_id"], "a1");
        assert_eq!(r["data"]["client_id"], "alice@example.com");
        assert_eq!(r["data"]["permissions"], json!(["service:read"]));
        assert_eq!(s.client_id.as_deref(), Some("alice@example.com"));
        assert_eq!(s.tenant_id.as_deref(), Some(t.as_str()));
        assert!(s.token.is_some());
        let parsed: commy_wire::ControlMessage = serde_json::from_value(r.clone()).unwrap();
        assert!(matches!(parsed, commy_wire::ControlMessage::AuthenticationResult(_)));
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_self_claimed_client_id_is_not_believed() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        // alice's key, but the request claims to be root
        let r = call(&server, &mut s, auth(&t, json!({"client_id":"root"}))).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        assert_eq!(r["data"]["client_id"], "alice@example.com", "identity is the credential's subject: {r}");
        assert_eq!(s.client_id.as_deref(), Some("alice@example.com"));
        // and what root holds is not alice's
        let mut root = ClientSession::new();
        call(&server, &mut root, auth(&t, json!({"key":"k-root-secret"}))).await;
        let rr = call(&server, &mut root, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"svc","size":4096}})).await;
        assert_eq!(rr["type"], "Service", "{rr}");
        let id = rr["data"]["service_id"].as_str().unwrap().to_string();
        let g = server.read().await;
        assert!(g.grants.allows(&t, "root", &id, crate::server::grants::Action::Admin));
        assert!(!g.grants.allows(&t, "alice@example.com", &id, crate::server::grants::Action::Write));
        drop(g);
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_wrong_key_is_refused_and_leaves_the_session_unauthenticated() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({"key":"not-the-key"}))).await;
        assert_eq!(r["type"], "AuthenticationResult", "{r}");
        assert_eq!(r["data"]["success"], false, "{r}");
        assert_eq!(r["data"]["message"], "Authentication failed");
        assert!(r["data"].get("client_id").is_none() && r["data"].get("permissions").is_none(), "{r}");
        assert!(s.token.is_none() && s.client_id.is_none() && s.tenant_id.is_none());
        let again = call(&server, &mut s, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"x","size":1}})).await;
        assert_eq!(again["data"]["code"], "UNAUTHENTICATED", "{again}");
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_key_for_another_tenant_does_not_open_this_one() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({"key":"k-other-secret"}))).await;
        assert_eq!(r["data"]["success"], false, "{r}");
        assert!(s.token.is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn the_credentials_scopes_are_what_the_session_gets() {
        let t = tenant();
        let server = production(&t);
        let mut alice = ClientSession::new();
        call(&server, &mut alice, auth(&t, json!({}))).await;
        let r = call(&server, &mut alice, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"x","size":4096}})).await;
        assert_eq!(r["data"]["code"], "PERMISSION_DENIED", "alice has no service:create: {r}");
        let mut root = ClientSession::new();
        call(&server, &mut root, auth(&t, json!({"key":"k-root-secret"}))).await;
        let r = call(&server, &mut root, json!({"type":"CreateService","data":{"tenant_id":t,"service_name":"x","size":4096}})).await;
        assert_eq!(r["type"], "Service", "{r}");
        let id = r["data"]["service_id"].as_str().unwrap().to_string();
        // alice's tenant-wide read reaches the service, her missing write does not
        let p = call(&server, &mut alice, json!({"type":"GetServiceFilePath","data":{"request_id":"p","service_id":id}})).await;
        assert_eq!(p["type"], "ServiceFilePath", "{p}");
        assert!(p["data"].get("path").is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn development_hints_are_refused_by_a_production_server() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({"dev_scopes":["admin"]}))).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{r}");
        assert!(s.token.is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn an_unsupported_protocol_version_is_refused_before_the_credentials_are_looked_at() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({"protocol_version": 99}))).await;
        assert_eq!(r["data"]["code"], "UNSUPPORTED_VERSION", "{r}");
        assert_eq!(r["data"]["request_id"], "a1");
        assert!(s.token.is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_method_the_verifier_does_not_handle_is_refused() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        let r = call(
            &server,
            &mut s,
            json!({"type":"Authenticate","data":{"request_id":"j","protocol_version":1,"tenant_id":t,"method":"jwt","token":"a.b.c"}}),
        )
        .await;
        assert_eq!(r["data"]["success"], false, "{r}");
        assert!(s.token.is_none());
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_malformed_request_is_invalid_and_its_content_is_never_echoed() {
        let t = tenant();
        let server = production(&t);
        for (name, bad) in [
            ("key is a number", json!({"request_id":"m","protocol_version":1,"tenant_id":t,"method":"api_key","key":123456789})),
            ("no method", json!({"request_id":"m","protocol_version":1,"tenant_id":t,"key":"k-alice-secret"})),
            ("unknown method", json!({"request_id":"m","protocol_version":1,"tenant_id":t,"method":"kerberos","key":"k-alice-secret"})),
            ("no protocol_version", json!({"request_id":"m","tenant_id":t,"method":"api_key","key":"k-alice-secret"})),
            ("bad tenant id", json!({"request_id":"m","protocol_version":1,"tenant_id":"../x","method":"api_key","key":"k-alice-secret"})),
            ("empty key", json!({"request_id":"m","protocol_version":1,"tenant_id":t,"method":"api_key","key":""})),
        ] {
            let mut s = ClientSession::new();
            let r = call(&server, &mut s, json!({"type":"Authenticate","data":bad})).await;
            assert_eq!(r["type"], "Error", "{name}: {r}");
            assert_eq!(r["data"]["code"], "INVALID_REQUEST", "{name}: {r}");
            let text = r.to_string();
            assert!(!text.contains("k-alice-secret") && !text.contains("123456789"), "{name}: the reply echoes a credential");
            assert!(s.token.is_none(), "{name}");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn no_credential_ever_reaches_the_audit_trail() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        call(&server, &mut s, auth(&t, json!({}))).await;
        call(&server, &mut s, auth(&t, json!({"key":"k-guess-one"}))).await;
        let trail = server.read().await.audit_events().join("\n");
        assert!(trail.contains("authentication ACCEPTED") && trail.contains("authentication REFUSED"), "{trail}");
        for secret in ["k-alice-secret", "k-root-secret", "k-guess-one"] {
            assert!(!trail.contains(secret), "the audit trail holds a credential");
        }
        cleanup(&t);
    }

    #[tokio::test]
    async fn a_failed_reauthentication_does_not_change_who_the_session_is() {
        let t = tenant();
        let server = production(&t);
        let mut s = ClientSession::new();
        call(&server, &mut s, auth(&t, json!({}))).await;
        let r = call(&server, &mut s, auth(&t, json!({"key":"k-root-secret-but-wrong"}))).await;
        assert_eq!(r["data"]["success"], false);
        assert_eq!(s.client_id.as_deref(), Some("alice@example.com"));
        cleanup(&t);
    }

    #[tokio::test]
    async fn without_a_verifier_and_without_development_authentication_nobody_gets_in() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(false)));
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({}))).await;
        assert_eq!(r["type"], "Error", "{r}");
        assert_eq!(r["data"]["code"], "UNAUTHENTICATED", "{r}");
        assert!(s.token.is_none());
    }

    #[tokio::test]
    async fn development_authentication_still_takes_the_clients_word_when_it_is_on() {
        let t = tenant();
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = ClientSession::new();
        let r = call(&server, &mut s, auth(&t, json!({"client_id":"anyone","key":"whatever","dev_scopes":[]}))).await;
        assert_eq!(r["data"]["success"], true, "{r}");
        assert_eq!(r["data"]["client_id"], "anyone");
        assert_eq!(r["data"]["permissions"], json!([]));
        cleanup(&t);
    }

}

#[cfg(test)]
mod removed_protocol_tests {
    use super::*;

    async fn call(msg: serde_json::Value) -> serde_json::Value {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = ClientSession::new();
        handle_sdk_message(msg, &mut s, server).await.expect("every message gets a reply")
    }

    /// Plan C6: the named-variable messages are gone from the wire. Moved here from the removed
    /// `WssMessage` decode test (R12): a client that still sends one gets an error, not a handler.
    #[tokio::test]
    async fn the_named_variable_messages_get_an_error_reply() {
        for name in [
            "GetVariables", "VariablesData", "SetVariables", "VariablesUpdated",
            "VariableChanged", "Subscribe", "SubscriptionAck",
        ] {
            let r = call(serde_json::json!({ "type": name, "data": {} })).await;
            assert_eq!(r["type"], "Error", "{name}: {r}");
        }
    }

    /// The JSON heartbeat sets the field `ClientSession::is_healthy` reads (the removed MessagePack
    /// heartbeat did; R12).
    #[tokio::test]
    async fn the_json_heartbeat_records_the_acknowledgement() {
        let server = Arc::new(RwLock::new(Server::with_dev_auth(true)));
        let mut s = ClientSession::new();
        assert!(s.last_heartbeat_ack.is_none());
        let r = handle_sdk_message(serde_json::json!({ "type": "Heartbeat", "data": {} }), &mut s, server)
            .await
            .unwrap();
        assert_eq!(r["type"], "Heartbeat");
        assert!(s.last_heartbeat_ack.is_some());
    }
}
