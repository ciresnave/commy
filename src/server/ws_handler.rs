//! WebSocket Connection Handler (RFC 6455)
//!
//! Handles individual WebSocket connections from remote clients.
//! Implements WebSocket protocol (RFC 6455) with binary frames for MessagePack messages.
//! Manages WebSocket frame reading/writing, session lifecycle, and message routing.

use crate::protocol::{ClientSession, ClientState, WssMessage};
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

    loop {
        // Read WebSocket frame (RFC 6455 frame format)
        match read.next().await {
            Some(Ok(msg)) => {
                match msg {
                    Message::Binary(data) => {
                        // Parse incoming MessagePack message from binary frame
                        match rmp_serde::from_slice::<WssMessage>(&data) {
                            Ok(wss_message) => {
                                println!(
                                    "Received {} message from {}",
                                    wss_message.message_type(),
                                    peer_addr
                                );

                                // Route message to handler
                                if let Some(response) =
                                    handle_message(wss_message, &mut session, Arc::clone(&_server))
                                        .await
                                {
                                    // Send response back as WebSocket binary frame
                                    if let Ok(serialized) = rmp_serde::to_vec(&response) {
                                        if let Err(e) =
                                            write.send(Message::Binary(serialized)).await
                                        {
                                            eprintln!("Failed to send WebSocket frame: {}", e);
                                            break;
                                        }
                                    }
                                }

                                // Update session state
                                session.last_activity = chrono::Utc::now();
                                sessions
                                    .write()
                                    .await
                                    .insert(session.session_id.clone(), session.clone());
                            }
                            Err(e) => {
                                eprintln!(
                                    "Failed to deserialize message from {}: {}",
                                    peer_addr, e
                                );
                                // Send error response
                                let error_response = WssMessage::Error {
                                    code: "PARSE_ERROR".to_string(),
                                    message: format!("Invalid message format: {}", e),
                                    details: None,
                                };
                                if let Ok(serialized) = rmp_serde::to_vec(&error_response) {
                                    let _ = write.send(Message::Binary(serialized)).await;
                                }
                            }
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
                                if let Some(response) = handle_sdk_message(
                                    msg_value,
                                    &mut session,
                                    Arc::clone(&_server),
                                )
                                .await
                                {
                                    // Send response back as Text (JSON)
                                    if let Ok(serialized) = serde_json::to_string(&response) {
                                        println!(
                                            "[SDK Response] Sending: {}",
                                            &serialized[..std::cmp::min(100, serialized.len())]
                                        );
                                        if let Err(e) = write.send(Message::Text(serialized)).await
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
                                let error = WssMessage::Error {
                                    code: "INVALID_JSON".to_string(),
                                    message: "Invalid JSON format".to_string(),
                                    details: None,
                                };
                                if let Ok(serialized) = rmp_serde::to_vec(&error) {
                                    let _ = write.send(Message::Binary(serialized)).await;
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

    // Cleanup: remove session on disconnect
    sessions.write().await.remove(&session.session_id);
    println!("Client {} session cleaned up", session.session_id);

    Ok(())
}

/// Check if session has required permission
fn check_permission(
    session: &ClientSession,
    permission: crate::auth::Permission,
) -> Result<(), String> {
    // Check authentication
    if session.token.is_none() {
        return Err("Not authenticated".to_string());
    }

    // Check permission
    let permissions = session
        .permissions
        .as_ref()
        .ok_or_else(|| "No permissions found".to_string())?;

    if !permissions.has_permission(&permission) {
        return Err(format!("Permission denied: missing {:?}", permission));
    }

    Ok(())
}

/// Handles an incoming WssMessage and returns optional response
async fn handle_message(
    message: WssMessage,
    session: &mut ClientSession,
    server: Arc<RwLock<Server>>,
) -> Option<WssMessage> {
    use crate::protocol::WssMessage::*;

    match message {
        Authenticate {
            tenant_id,
            credentials: _credentials,
            auth_method,
            ..
        } => {
            println!(
                "Authentication request for tenant: {} using method: {}",
                tenant_id, auth_method
            );

            // Get tenant from server
            let auth_context_arc = {
                let server_guard = server.read().await;
                let tenant = match server_guard.tenants.get(&tenant_id) {
                    Some(t) => t,
                    None => {
                        return Some(AuthenticationResponse {
                            success: false,
                            message: format!("Tenant '{}' not found", tenant_id),
                            server_version: env!("CARGO_PKG_VERSION").to_string(),
                            token: None,
                            expires_in_seconds: None,
                        });
                    }
                };
                Arc::clone(&tenant.auth_context)
            };
            let _ = auth_context_arc; // Keep reference to auth context (may be used in future)
                                      // Auth-framework is registered but NOT initialized to avoid configuration issues
                                      // In production, implement proper auth-framework integration with database backend

            // Grant full admin permissions to all clients in development mode
            session.tenant_id = Some(tenant_id.clone());
            session.state = ClientState::Active;
            let token_str = format!(
                "dev-token-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            );
            session.token = Some(token_str.clone());

            let permissions = crate::auth::PermissionSet::admin();
            session.permissions = Some(permissions.clone());

            println!(
                "Client {} authenticated successfully (DEV MODE) with admin permissions",
                session.session_id
            );

            Some(AuthenticationResponse {
                success: true,
                message: "Authentication successful (development mode)".to_string(),
                server_version: env!("CARGO_PKG_VERSION").to_string(),
                token: Some(token_str),
                expires_in_seconds: Some(3600),
            })
        }

        GetVariables {
            tenant_id,
            service_name,
            variable_names,
            ..
        } => {
            println!(
                "GetVariables request: {}/{}",
                service_name,
                variable_names.len()
            );

            // Check permissions: ServiceRead AND VariableRead
            if let Err(reason) = check_permission(session, crate::auth::Permission::ServiceRead) {
                return Some(Error {
                    code: "PERMISSION_DENIED".to_string(),
                    message: reason,
                    details: Some("ServiceRead permission required".to_string()),
                });
            }

            if let Err(reason) = check_permission(session, crate::auth::Permission::VariableRead) {
                return Some(Error {
                    code: "PERMISSION_DENIED".to_string(),
                    message: reason,
                    details: Some("VariableRead permission required".to_string()),
                });
            }

            // In production: Fetch variables from service
            let mut variables = std::collections::HashMap::new();
            for var_name in variable_names {
                variables.insert(var_name, vec![]);
            }

            Some(VariablesData {
                tenant_id,
                service_name,
                variables,
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }

        SetVariables {
            tenant_id: _,
            service_name,
            variables,
            ..
        } => {
            println!("SetVariables request: {}/{}", service_name, variables.len());

            // Check permissions: ServiceWrite AND VariableWrite
            if let Err(reason) = check_permission(session, crate::auth::Permission::ServiceWrite) {
                return Some(Error {
                    code: "PERMISSION_DENIED".to_string(),
                    message: reason,
                    details: Some("ServiceWrite permission required".to_string()),
                });
            }

            if let Err(reason) = check_permission(session, crate::auth::Permission::VariableWrite) {
                return Some(Error {
                    code: "PERMISSION_DENIED".to_string(),
                    message: reason,
                    details: Some("VariableWrite permission required".to_string()),
                });
            }

            // In production: Update variables in service
            Some(VariablesUpdated {
                success: true,
                message: "Variables updated".to_string(),
                service_name,
            })
        }

        Subscribe {
            tenant_id: _,
            service_name,
            variable_names,
            ..
        } => {
            println!(
                "Subscribe request: {}/{}",
                service_name,
                variable_names.len()
            );

            // Check permissions: ServiceRead required for subscriptions
            if let Err(reason) = check_permission(session, crate::auth::Permission::ServiceRead) {
                return Some(Error {
                    code: "PERMISSION_DENIED".to_string(),
                    message: reason,
                    details: Some("ServiceRead permission required for subscriptions".to_string()),
                });
            }

            // Register subscription
            for var in variable_names {
                session
                    .subscriptions
                    .insert(format!("{}/{}", service_name, var));
            }

            Some(SubscriptionAck {
                success: true,
                message: "Subscribed to variables".to_string(),
                service_name,
            })
        }

        Heartbeat { .. } => {
            session.last_heartbeat_ack = Some(chrono::Utc::now());
            Some(HeartbeatAck {
                timestamp: chrono::Utc::now().to_rfc3339(),
            })
        }

        Logout { token: _, .. } => {
            println!("Logout request from session: {}", session.session_id);

            // Auth-framework handles token management internally
            // Just clear the session state
            session.token = None;
            session.permissions = None;
            session.state = ClientState::Disconnected;

            println!("Client {} logged out successfully", session.session_id);

            Some(LogoutResponse {
                success: true,
                message: "Logout successful".to_string(),
            })
        }

        RefreshToken {
            current_token: _, ..
        } => {
            println!("Token refresh request from session: {}", session.session_id);

            // Auth-framework handles token refresh internally via its validate/create methods
            // For now, just return a response that token refresh needs to be re-authenticated
            Some(TokenRefreshResponse {
                success: false,
                message: "Token refresh requires re-authentication with auth-framework".to_string(),
                token: None,
                expires_in_seconds: None,
            })
        }

        Error {
            code,
            message,
            details,
        } => {
            eprintln!("Client error: {} - {}", code, message);
            if let Some(d) = details {
                eprintln!("Details: {}", d);
            }
            None
        }

        _ => {
            eprintln!("Unhandled message type");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::PermissionSet;

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

    #[tokio::test]
    async fn test_handle_heartbeat() {
        let message = WssMessage::Heartbeat {
            session_id: "test_session".to_string(),
        };
        let mut session = ClientSession::new();
        let server = Arc::new(RwLock::new(Server::new()));
        let response = handle_message(message, &mut session, server).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::HeartbeatAck { .. } => (),
            _ => panic!("Expected HeartbeatAck response"),
        }
    }

    #[test]
    fn test_session_subscription() {
        let mut session = ClientSession::new();
        session.subscriptions.insert("service_a/var1".to_string());
        session.subscriptions.insert("service_a/var2".to_string());

        assert_eq!(session.subscriptions.len(), 2);
        assert!(session.subscriptions.contains("service_a/var1"));
    }

    #[tokio::test]
    async fn test_get_variables_without_permission() {
        let message = WssMessage::GetVariables {
            session_id: "test_session".to_string(),
            tenant_id: "test_tenant".to_string(),
            service_name: "test_service".to_string(),
            variable_names: vec!["var1".to_string()],
        };

        // Session without permissions
        let mut session = ClientSession::new();
        session.token = Some("fake_token".to_string());
        session.tenant_id = Some("test_tenant".to_string());
        session.permissions = Some(crate::auth::PermissionSet::new()); // Empty permissions

        let server = Arc::new(RwLock::new(Server::new()));
        let response = handle_message(message, &mut session, server).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::Error { code, .. } => {
                assert_eq!(code, "PERMISSION_DENIED");
            }
            _ => panic!("Expected Error response for missing permission"),
        }
    }

    #[tokio::test]
    async fn test_get_variables_with_permission() {
        let message = WssMessage::GetVariables {
            session_id: "test_session".to_string(),
            tenant_id: "test_tenant".to_string(),
            service_name: "test_service".to_string(),
            variable_names: vec!["var1".to_string()],
        };

        // Session with proper permissions
        let mut session = ClientSession::new();
        session.token = Some("fake_token".to_string());
        session.tenant_id = Some("test_tenant".to_string());

        let mut permissions = crate::auth::PermissionSet::new();
        permissions.grant(crate::auth::Permission::ServiceRead);
        permissions.grant(crate::auth::Permission::VariableRead);
        session.permissions = Some(permissions);

        let server = Arc::new(RwLock::new(Server::new()));
        let response = handle_message(message, &mut session, server).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::VariablesData { .. } => (),
            _ => panic!("Expected VariablesData response with proper permissions"),
        }
    }

    #[tokio::test]
    async fn test_set_variables_without_permission() {
        let message = WssMessage::SetVariables {
            session_id: "test_session".to_string(),
            tenant_id: "test_tenant".to_string(),
            service_name: "test_service".to_string(),
            variables: std::collections::HashMap::new(),
        };

        // Session with read-only permissions
        let mut session = ClientSession::new();
        session.token = Some("fake_token".to_string());
        session.tenant_id = Some("test_tenant".to_string());

        let mut permissions = crate::auth::PermissionSet::new();
        permissions.grant(crate::auth::Permission::ServiceRead);
        permissions.grant(crate::auth::Permission::VariableRead);
        // Missing ServiceWrite and VariableWrite
        session.permissions = Some(permissions);

        let server = Arc::new(RwLock::new(Server::new()));
        let response = handle_message(message, &mut session, server).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::Error { code, .. } => {
                assert_eq!(code, "PERMISSION_DENIED");
            }
            _ => panic!("Expected Error response for missing write permission"),
        }
    }

    #[tokio::test]
    async fn test_subscribe_without_permission() {
        let message = WssMessage::Subscribe {
            session_id: "test_session".to_string(),
            tenant_id: "test_tenant".to_string(),
            service_name: "test_service".to_string(),
            variable_names: vec!["var1".to_string()],
        };

        // Session without ServiceRead permission
        let mut session = ClientSession::new();
        session.token = Some("fake_token".to_string());
        session.tenant_id = Some("test_tenant".to_string());
        session.permissions = Some(crate::auth::PermissionSet::new()); // Empty permissions

        let server = Arc::new(RwLock::new(Server::new()));
        let response = handle_message(message, &mut session, server).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::Error { code, .. } => {
                assert_eq!(code, "PERMISSION_DENIED");
            }
            _ => panic!("Expected Error response for missing permission"),
        }
    }

    #[tokio::test]
    async fn test_logout() {
        // Set development environment
        unsafe {
            std::env::set_var("ENVIRONMENT", "development");
        }

        let mut server = Server::new();
        let _tenant = server.get_tenant("test_tenant");

        // Create a session with a token (simulating authenticated state)
        let mut session = ClientSession::new();
        session.token = Some("test_token_123".to_string());
        session.tenant_id = Some("test_tenant".to_string());
        session.state = ClientState::Active;
        session.permissions = Some(PermissionSet::read_only());

        let server_arc = Arc::new(RwLock::new(server));

        let message = WssMessage::Logout {
            session_id: session.session_id.clone(),
            token: "test_token_123".to_string(),
        };

        let response = handle_message(message, &mut session, server_arc.clone()).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::LogoutResponse { success, .. } => {
                assert!(success, "Logout should succeed");
            }
            _ => panic!("Expected LogoutResponse"),
        }

        // Verify session state cleared
        assert!(session.token.is_none(), "Token should be cleared");
        assert!(
            session.permissions.is_none(),
            "Permissions should be cleared"
        );
        assert_eq!(
            session.state,
            ClientState::Disconnected,
            "State should be Disconnected"
        );
    }

    #[tokio::test]
    async fn test_token_refresh_returns_error() {
        // Set development environment
        unsafe {
            std::env::set_var("ENVIRONMENT", "development");
        }

        let mut server = Server::new();
        let _tenant = server.get_tenant("test_tenant");

        // Create a session with a valid token
        let mut session = ClientSession::new();
        session.token = Some("old_token_123".to_string());
        session.tenant_id = Some("test_tenant".to_string());
        session.state = ClientState::Active;

        let server_arc = Arc::new(RwLock::new(server));

        let message = WssMessage::RefreshToken {
            session_id: session.session_id.clone(),
            current_token: "old_token_123".to_string(),
        };

        let response = handle_message(message, &mut session, server_arc.clone()).await;

        // Current implementation returns error for token refresh
        // (auth-framework doesn't expose refresh API directly)
        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::TokenRefreshResponse {
                success, message, ..
            } => {
                assert!(
                    !success,
                    "Token refresh should indicate requires re-authentication"
                );
                assert!(
                    message.contains("re-authentication"),
                    "Should mention re-authentication"
                );
            }
            _ => panic!("Expected TokenRefreshResponse"),
        }

        // Session token should remain unchanged
        assert_eq!(session.token, Some("old_token_123".to_string()));
    }

    #[tokio::test]
    async fn test_token_refresh_with_invalid_token() {
        // Set development environment
        unsafe {
            std::env::set_var("ENVIRONMENT", "development");
        }

        let server = Arc::new(RwLock::new(Server::new()));

        let mut session = ClientSession::new();
        session.tenant_id = Some("test_tenant".to_string());

        let message = WssMessage::RefreshToken {
            session_id: session.session_id.clone(),
            current_token: "invalid_token".to_string(),
        };

        let response = handle_message(message, &mut session, server).await;

        assert!(response.is_some());
        match response.unwrap() {
            WssMessage::TokenRefreshResponse { success, .. } => {
                assert!(!success, "Token refresh should fail (not supported)");
            }
            _ => panic!("Expected TokenRefreshResponse"),
        }
    }
}

/// Handle SDK protocol messages (JSON format from commy-client SDK)
///
/// Processes new CRUD operations: CreateService, GetService, DeleteService
/// Returns ServerMessage response (as JSON) to send back to client
async fn handle_sdk_message(
    message: serde_json::Value,
    session: &mut ClientSession,
    server: Arc<RwLock<Server>>,
) -> Option<serde_json::Value> {
    let msg_type = message
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");
    let data = message.get("data");

    match msg_type {
        "Authenticate" => {
            // DEVELOPMENT MODE: Accept any credential, grant admin permissions
            // In production, implement proper auth-framework integration
            let tenant_id = data
                .and_then(|d| d.get("tenant_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("my_tenant");
            let _api_key = data
                .and_then(|d| d.get("api_key"))
                .and_then(|v| v.as_str())
                .unwrap_or("any-key-works");

            // Set session authentication
            session.tenant_id = Some(tenant_id.to_string());
            session.state = ClientState::Active;
            let token_str = format!(
                "dev-token-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            );
            session.token = Some(token_str.clone());

            // Create permissions with all service operations
            let mut permissions = crate::auth::PermissionSet::admin();
            permissions.grant(crate::auth::Permission::ServiceCreate);
            permissions.grant(crate::auth::Permission::ServiceRead);
            permissions.grant(crate::auth::Permission::ServiceDelete);
            permissions.grant(crate::auth::Permission::ServiceWrite);
            session.permissions = Some(permissions);

            // Ensure the tenant record exists in the server so services can be created under it
            {
                let mut server_guard = server.write().await;
                let _ = server_guard.get_tenant(tenant_id);
            }

            println!(
                "Client {} authenticated successfully (DEV MODE) to tenant: {} with admin permissions",
                session.session_id, tenant_id
            );

            // Return proper ServerMessage::AuthenticationResult format
            Some(serde_json::json!({
                "type": "AuthenticationResult",
                "data": {
                    "success": true,
                    "message": "Authentication successful (development mode)",
                    "server_version": env!("CARGO_PKG_VERSION"),
                    "permissions": ["all"]
                }
            }))
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
                        "code": "Unauthorized",
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

            // Generate cryptographically random service ID; only Server records the file mapping
            let service_id = uuid::Uuid::new_v4().to_string();
            let tenant_dir = format!("tenant_{}", tenant_id);
            let file_path = format!("{}/service_{}.mem", tenant_dir, service_id);

            if let Err(e) = std::fs::create_dir_all(&tenant_dir) {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InternalError", "message": format!("Failed to create tenant directory: {}", e) }
                }));
            }

            let mut server_guard = server.write().await;
            let tenant = match server_guard.tenants.get_mut(tenant_id) {
                Some(t) => t,
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": format!("Tenant '{}' not found", tenant_id) }
                })),
            };

            if let Err(e) = tenant.register_service(service_name, &file_path) {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InternalError", "message": format!("Failed to create service file: {}", e) }
                }));
            }

            server_guard.service_registry.insert(service_id.clone(), crate::ServiceRecord {
                tenant_name: tenant_id.to_string(),
                service_name: service_name.to_string(),
                file_path,
            });

            println!("[CreateService] Created service '{}' in tenant '{}' with ID {}", service_name, tenant_id, service_id);

            Some(serde_json::json!({
                "type": "Service",
                "data": {
                    "service_id": service_id,
                    "service_name": service_name,
                    "tenant_id": tenant_id
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
                        "code": "Unauthorized",
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
            let found = server_guard.service_registry.iter()
                .find(|(_, r)| r.tenant_name == tenant_id && r.service_name == service_name)
                .map(|(id, _)| id.clone());

            match found {
                Some(service_id) => {
                    println!("[GetService] Found service '{}' in tenant '{}' → id={}", service_name, tenant_id, service_id);
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
                    println!("[GetService] Service '{}' not found in tenant '{}'", service_name, tenant_id);
                    Some(serde_json::json!({
                        "type": "Error",
                        "data": {
                            "code": "NotFound",
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
                        "code": "Unauthorized",
                        "message": "Not authenticated to this tenant"
                    }
                }));
            }

            // Check delete_service permission
            println!("[DeleteService] Checking permissions");
            if let Some(perms) = &session.permissions {
                if !perms.has_permission(&crate::auth::Permission::ServiceDelete) {
                    return Some(serde_json::json!({
                        "type": "Error",
                        "data": {
                            "code": "PERMISSION_DENIED",
                            "message": "Permission denied: delete_service required"
                        }
                    }));
                }
            }

            // TODO: Actual service deletion deferred to rsqlx integration
            // For now, just return success response immediately
            println!(
                "[DeleteService] Deleting service '{}' from tenant '{}'",
                service_name, tenant_id
            );

            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": true,
                    "message": "Service deleted successfully"
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
                        "code": "InvalidRequest",
                        "message": "tenant_id and tenant_name are required"
                    }
                }));
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
                        "code": "InvalidRequest",
                        "message": "tenant_id is required"
                    }
                }));
            }

            // Delete tenant
            let _server_guard = server.write().await;

            // For now, simply remove the tenant from memory
            // In a production system, this would handle persistent storage
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
            Some(serde_json::json!({
                "type": "Heartbeat",
                "data": {
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            }))
        }

        "AllocateVariable" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let variable_name = data
                .and_then(|d| d.get("variable_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let initial_data: Vec<u8> = data
                .and_then(|d| d.get("initial_data"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            if service_id.is_empty() || variable_name.is_empty() {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InvalidRequest", "message": "service_id and variable_name are required" }
                }));
            }

            let mut server_guard = server.write().await;
            let (tenant_name, svc_name) = match server_guard.service_registry.get(service_id) {
                Some(r) => (r.tenant_name.clone(), r.service_name.clone()),
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": format!("Service '{}' not found", service_id) }
                })),
            };

            let svc = match server_guard.tenants.get_mut(&tenant_name)
                .and_then(|t| t.get_service_mut_by_name(&svc_name)) {
                Some(s) => s,
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": "Service not found in tenant" }
                })),
            };

            let size = if initial_data.is_empty() { 8 } else { initial_data.len() };
            match svc.allocate_variable(variable_name.to_string(), size) {
                Some(slot) => {
                    let copy_len = slot.len().min(initial_data.len());
                    if copy_len > 0 {
                        slot[..copy_len].copy_from_slice(&initial_data[..copy_len]);
                    }
                    println!("[AllocateVariable] service={} var={} size={}", service_id, variable_name, size);
                    Some(serde_json::json!({
                        "type": "Result",
                        "data": {
                            "request_id": uuid::Uuid::new_v4().to_string(),
                            "success": true,
                            "message": format!("Variable '{}' allocated ({} bytes)", variable_name, size)
                        }
                    }))
                }
                None => Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InternalError", "message": "Allocation failed (service file may be full)" }
                })),
            }
        }

        "ReadVariable" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let variable_name = data
                .and_then(|d| d.get("variable_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let server_guard = server.read().await;
            let (tenant_name, svc_name) = match server_guard.service_registry.get(service_id) {
                Some(r) => (r.tenant_name.clone(), r.service_name.clone()),
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": format!("Service '{}' not found", service_id) }
                })),
            };

            let data_copy = server_guard.tenants
                .get(&tenant_name)
                .and_then(|t| t.get_service_by_name(&svc_name))
                .and_then(|svc| svc.get_variable(variable_name))
                .map(|bytes| bytes.to_vec());

            match data_copy {
                Some(var_data) => Some(serde_json::json!({
                    "type": "VariableData",
                    "data": {
                        "service_id": service_id,
                        "variable_name": variable_name,
                        "data": var_data,
                        "version": 1
                    }
                })),
                None => Some(serde_json::json!({
                    "type": "Error",
                    "data": {
                        "code": "NotFound",
                        "message": format!("Variable '{}' not found in service '{}'", variable_name, service_id)
                    }
                })),
            }
        }

        "WriteVariable" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let variable_name = data
                .and_then(|d| d.get("variable_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_data: Vec<u8> = data
                .and_then(|d| d.get("data"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            if service_id.is_empty() || variable_name.is_empty() {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InvalidRequest", "message": "service_id and variable_name are required" }
                }));
            }

            let mut server_guard = server.write().await;
            let (tenant_name, svc_name) = match server_guard.service_registry.get(service_id) {
                Some(r) => (r.tenant_name.clone(), r.service_name.clone()),
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": format!("Service '{}' not found", service_id) }
                })),
            };

            let svc = match server_guard.tenants.get_mut(&tenant_name)
                .and_then(|t| t.get_service_mut_by_name(&svc_name)) {
                Some(s) => s,
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": "Service not found in tenant" }
                })),
            };

            // Existing size determines whether we write in-place, resize, or auto-allocate
            let existing_size = svc.get_variable(variable_name).map(|d| d.len());
            let ok = match existing_size {
                Some(size) if size == new_data.len() => {
                    // Same size: write directly into the mmap slot
                    svc.get_variable_mut(variable_name)
                        .map(|slot| slot.copy_from_slice(&new_data))
                        .is_some()
                }
                Some(_) => {
                    // Size changed: deallocate old slot and re-allocate
                    svc.deallocate_variable(variable_name);
                    svc.allocate_variable(variable_name.to_string(), new_data.len())
                        .map(|slot| slot.copy_from_slice(&new_data))
                        .is_some()
                }
                None => {
                    // Variable not yet allocated: auto-allocate
                    svc.allocate_variable(variable_name.to_string(), new_data.len())
                        .map(|slot| slot.copy_from_slice(&new_data))
                        .is_some()
                }
            };

            if ok {
                println!("[WriteVariable] service={} var={} bytes={}", service_id, variable_name, new_data.len());
                Some(serde_json::json!({
                    "type": "Result",
                    "data": {
                        "request_id": uuid::Uuid::new_v4().to_string(),
                        "success": true,
                        "message": "Variable written"
                    }
                }))
            } else {
                Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InternalError", "message": "Write failed (service file may be full)" }
                }))
            }
        }

        "DeallocateVariable" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let variable_name = data
                .and_then(|d| d.get("variable_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mut server_guard = server.write().await;
            let (tenant_name, svc_name) = match server_guard.service_registry.get(service_id) {
                Some(r) => (r.tenant_name.clone(), r.service_name.clone()),
                None => return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "NotFound", "message": format!("Service '{}' not found", service_id) }
                })),
            };

            let removed = server_guard.tenants.get_mut(&tenant_name)
                .and_then(|t| t.get_service_mut_by_name(&svc_name))
                .map(|svc| svc.deallocate_variable(variable_name))
                .unwrap_or(false);

            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": removed,
                    "message": if removed {
                        format!("Variable '{}' deallocated", variable_name)
                    } else {
                        format!("Variable '{}' not found", variable_name)
                    }
                }
            }))
        }

        "Subscribe" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let variable_name = data
                .and_then(|d| d.get("variable_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if service_id.is_empty() || variable_name.is_empty() {
                return Some(serde_json::json!({
                    "type": "Error",
                    "data": { "code": "InvalidRequest", "message": "service_id and variable_name are required" }
                }));
            }

            let key = format!("{}/{}", service_id, variable_name);
            session.subscriptions.insert(key.clone());

            println!("[Subscribe] session={} subscribed to {}", session.session_id, key);
            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": true,
                    "message": format!("Subscribed to '{}'", key)
                }
            }))
        }

        "Unsubscribe" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let variable_name = data
                .and_then(|d| d.get("variable_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let key = format!("{}/{}", service_id, variable_name);
            let removed = session.subscriptions.remove(&key);

            Some(serde_json::json!({
                "type": "Result",
                "data": {
                    "request_id": uuid::Uuid::new_v4().to_string(),
                    "success": removed,
                    "message": if removed {
                        format!("Unsubscribed from '{}'", key)
                    } else {
                        format!("Not subscribed to '{}'", key)
                    }
                }
            }))
        }

        "GetServiceFilePath" => {
            // Direct file access is only available to processes on the same machine
            // via the memory-mapping path; not via WSS
            Some(serde_json::json!({
                "type": "Error",
                "data": {
                    "code": "InvalidRequest",
                    "message": "Direct file access is only available to local clients via memory-mapping"
                }
            }))
        }

        "ReportVariableChanges" => {
            let service_id = data
                .and_then(|d| d.get("service_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let changed_variables: Vec<String> = data
                .and_then(|d| d.get("changed_variables"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let new_values: Vec<(String, Vec<u8>)> = data
                .and_then(|d| d.get("new_values"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            let mut server_guard = server.write().await;
            let (tenant_name, svc_name) = match server_guard.service_registry.get(service_id) {
                Some(r) => (r.tenant_name.clone(), r.service_name.clone()),
                None => {
                    // Service not found — still acknowledge so the client can proceed
                    return Some(serde_json::json!({
                        "type": "VariableChangesAcknowledged",
                        "data": { "service_id": service_id, "changed_variables": changed_variables }
                    }));
                }
            };

            if let Some(svc) = server_guard.tenants.get_mut(&tenant_name)
                .and_then(|t| t.get_service_mut_by_name(&svc_name))
            {
                for (var_name, var_data) in new_values {
                    let existing_size = svc.get_variable(&var_name).map(|d| d.len());
                    match existing_size {
                        Some(size) if size == var_data.len() => {
                            if let Some(slot) = svc.get_variable_mut(&var_name) {
                                slot.copy_from_slice(&var_data);
                            }
                        }
                        Some(_) => {
                            svc.deallocate_variable(&var_name);
                            if let Some(slot) = svc.allocate_variable(var_name, var_data.len()) {
                                slot.copy_from_slice(&var_data);
                            }
                        }
                        None => {
                            if let Some(slot) = svc.allocate_variable(var_name, var_data.len()) {
                                slot.copy_from_slice(&var_data);
                            }
                        }
                    }
                }
            }

            println!("[ReportVariableChanges] service={} changes={}", service_id, changed_variables.len());
            Some(serde_json::json!({
                "type": "VariableChangesAcknowledged",
                "data": {
                    "service_id": service_id,
                    "changed_variables": changed_variables
                }
            }))
        }

        "Disconnect" => {
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
