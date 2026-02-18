# WebSocket Secure (WSS) Protocol Implementation

## Overview

This document describes the implementation of RFC 6455 (WebSocket Protocol) with RFC 5246 (TLS) support for Commy remote client connectivity.

## Protocol Stack

### Layer 1: TLS (RFC 5246)
- **Purpose**: Secure transport layer with encryption and authentication
- **Implementation**: tokio-rustls with Rustls TLS library
- **Certificate Format**: PEM format (.pem, .crt files)
- **Key Format**: PKCS#8 Private Key format (.pem, .key files)

### Layer 2: WebSocket (RFC 6455)
- **Purpose**: Bidirectional communication over TCP with frame-based protocol
- **Implementation**: tokio-tungstenite with automatic frame handling
- **Message Format**: Binary frames containing MessagePack-serialized messages

### Layer 3: Application Protocol
- **Message Format**: MessagePack (binary serialization)
- **Message Types**: Authentication, GetVariables, SetVariables, Subscribe, Heartbeat, etc.

## Architecture Diagram

```
Remote Client
    ↓
TLS Connection (RFC 5246)
    ↓
TCP Socket
    ↓
Server Network Interface
    ↓
WebSocket Handshake (HTTP Upgrade)
    ↓
WebSocket Stream (RFC 6455)
    ↓
Message Router
    ↓
Session Manager
    ↓
Tenant Authorization
    ↓
Service Access
```

## Implementation Components

### 1. TLS Configuration (`src/server/tls.rs`)

**Purpose**: Load and validate TLS certificates and private keys

**Key Features**:
- Loads certificates from PEM files using rustls-pemfile
- Validates private key format (PKCS#8)
- Creates ServerConfig for TLS acceptor
- Comprehensive error handling for certificate issues

**API**:
```rust
pub struct TlsConfiguration {
    pub config: Arc<ServerConfig>,
}

impl TlsConfiguration {
    pub fn from_files<P: AsRef<Path>>(
        cert_path: P,
        key_path: P,
    ) -> Result<Self, TlsError>
}
```

**Error Types**:
- `CertificateNotFound`: File not found or not readable
- `KeyNotFound`: Key file not found or not readable
- `InvalidCertificate`: Certificate parsing or format error
- `InvalidPrivateKey`: Private key parsing or format error
- `PemParseError`: PEM format parsing error
- `IoError`: General I/O errors

### 2. WebSocket Handler (`src/server/ws_handler.rs`)

**Purpose**: Handle individual WebSocket connections from remote clients

**Connection Lifecycle**:
1. Accept TLS-wrapped TCP connection
2. Perform WebSocket handshake (HTTP upgrade to WebSocket)
3. Read/write WebSocket frames in binary format
4. Route messages through message handler
5. Gracefully close connection with Close frame

**Key Features**:
- RFC 6455 compliant frame handling
- Automatic ping/pong keepalive
- Binary frame support for MessagePack messages
- Comprehensive error handling and logging
- Session state tracking

**Frame Types Handled**:
- **Binary**: Application messages (MessagePack serialized)
- **Text**: Rejected with error response
- **Ping**: Automatic response with Pong
- **Pong**: Updates last activity timestamp
- **Close**: Gracefully closes connection

**Message Handler**:
```rust
async fn handle_message(
    message: WssMessage,
    session: &mut ClientSession,
    server: Arc<RwLock<Server>>,
) -> Option<WssMessage>
```

Handles:
- `Authenticate`: Sets tenant_id and marks session as active
- `GetVariables`: Retrieves variables from service (checks authentication)
- `SetVariables`: Updates variables (checks authentication)
- `Subscribe`: Registers subscriptions to variables
- `Heartbeat`: Responds with HeartbeatAck
- `Error`: Logs error details

### 3. WSS Server (`src/server/mod.rs`)

**Purpose**: Main server entry point for accepting and managing WebSocket connections

**Responsibilities**:
- Bind to network address and port
- Accept TCP connections
- Perform TLS handshake
- Spawn per-connection handler tasks
- Manage client sessions
- Route messages to appropriate handlers

**Configuration**:
```rust
pub struct WssServerConfig {
    pub bind_addr: String,              // e.g., "127.0.0.1"
    pub port: u16,                      // e.g., 8443
    pub cert_path: Option<String>,      // Path to TLS certificate
    pub key_path: Option<String>,       // Path to TLS private key
    pub max_connections: usize,         // Concurrent connection limit
    pub buffer_size: usize,             // I/O buffer size
}
```

**Server Initialization**:
```rust
let mut server = WssServer::new(config, Arc::clone(&server));
server.initialize_tls()?;  // Load TLS certificates
server.run().await?;       // Start listening
```

**Session Management**:
- Track active client sessions
- Maintain per-client authentication state
- Broadcast updates to subscribed clients
- Clean up on disconnect

## Message Flow

### Authentication Flow
```
1. Client establishes WebSocket connection (TLS handshake + HTTP Upgrade)
2. Client sends first message: Authenticate { tenant_id, client_version, credentials }
3. Server validates credentials through Tenant
4. Server updates ClientSession { tenant_id, state = Active }
5. Server responds: AuthenticationResult { success, message, server_version, permissions }
```

**Note**: The WebSocket handshake (HTTP Upgrade) already establishes the connection, so there is no separate "Connect" message. Authentication is the first application-level message sent after the WebSocket connection is established.

### Variable Access Flow
```
1. Client must be authenticated (ClientSession.state == Active)
2. Client sends: GetVariables { tenant_id, service_name, variable_names }
3. Server checks permission (returns PermissionRevoked if unauthorized)
4. Server fetches variables from Service
5. Server responds: VariablesData { variables }
```

### Subscription Flow
```
1. Client sends: Subscribe { service_name, variable_names }
2. Server registers subscription in ClientSession
3. Server responds: SubscriptionAck { success }
4. When variables change: Server broadcasts VariableChanged to subscribers
```

### Heartbeat Flow (Keep-Alive)
```
1. Client sends: Heartbeat every 30 seconds (configurable)
2. Server updates: session.last_activity = now()
3. Server responds: HeartbeatAck { timestamp }
4. Server detects dead clients: If no activity for timeout period
```

## Protocol Details

### TLS Handshake (RFC 5246)

**Parameters**:
- Minimum TLS 1.2 (enforced by Rustls)
- No client certificates required
- Server certificate validation by client

**Certificate Requirements**:
- X.509 format
- Valid for intended hostname
- Not expired
- Properly signed (self-signed OK for development)

### WebSocket Handshake (RFC 6455)

**HTTP Upgrade Request** (Client sends):
```
GET / HTTP/1.1
Host: server.example.com:8443
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: [base64-encoded random key]
Sec-WebSocket-Version: 13
```

**HTTP Upgrade Response** (Server responds):
```
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: [computed from Sec-WebSocket-Key]
```

### WebSocket Frame Format (RFC 6455)

**Binary Frame** (application messages):
- FIN bit: 1 (final frame)
- Opcode: 0x02 (binary)
- Mask bit: 1 (client-to-server masked)
- Payload: MessagePack serialized WssMessage

**Frame Structure**:
```
[FIN|RSV|Opcode] [Mask|Length] [Extended Length] [Masking Key] [Payload]
    2 bytes          2 bytes      0-8 bytes       4 bytes      variable
```

### Message Serialization

**Format**: MessagePack (binary format)
- Compact binary encoding
- Language-independent
- Fast serialization/deserialization

**Example Message** (GetVariables):
```rust
WssMessage::GetVariables {
    session_id: "sess_123".to_string(),
    tenant_id: "org_a".to_string(),
    service_name: "config".to_string(),
    variable_names: vec!["user_count", "version"],
}
```

Serialized to binary MessagePack format and sent in WebSocket binary frame.

## Concurrency Model

### Connection Handling
- Each client connection runs in separate tokio task
- Non-blocking async/await throughout
- Multiple connections handled concurrently
- Task is spawned: `tokio::spawn(async move { ... })`

### Session Management
- ClientSession stored in Arc<RwLock<HashMap>>
- Read lock for checking authentication state
- Write lock for updating activity/subscriptions
- Minimal lock contention

### Thread Safety
- All types Send + Sync (required by tokio)
- No unsafe code except TLS library internals
- MessagePack serialization is thread-safe

## Client Access Patterns

### Remote Client (WSS)
- Connect to server via WSS (TLS + WebSocket)
- Authenticate with credentials
- Send/receive MessagePack messages
- Subscribe to variable changes
- Receive broadcasts asynchronously

### Local Client (Direct Memory-Mapping)
- On same machine as server
- Request filename from server (after authentication)
- Direct memory-map .mem file
- Zero-copy access to shared memory
- Falls back to WSS if not available

## Error Handling

### TLS Errors
- Certificate file not found → `TlsError::CertificateNotFound`
- Invalid certificate format → `TlsError::InvalidCertificate`
- Private key issues → `TlsError::InvalidPrivateKey`
- TLS handshake failure → Connection dropped, error logged

### WebSocket Errors
- Invalid UTF-8 in text frames → `INVALID_MESSAGE_TYPE` error response
- MessagePack parse errors → `PARSE_ERROR` error response
- Connection close by client → Graceful shutdown, session cleanup
- Network errors → Connection dropped, session cleaned up

### Application Errors
- Not authenticated → `PermissionRevoked` response
- Invalid tenant → `PermissionRevoked` response
- Service not found → Service created on first access
- Variable not found → Returns empty for GetVariables

## Security Considerations

### Encryption in Transit
- TLS 1.2+ encryption between client and server
- Prevents eavesdropping
- Prevents man-in-the-middle attacks

### Authentication
- Tenant-level authentication context
- Credentials validated by Tenant's auth provider
- Session token issued on successful authentication
- Tokens can expire (configurable)

### Authorization
- Per-tenant permission model
- Same client can have different permissions across tenants
- Permissions checked before every operation

### Session Isolation
- Each session has unique session_id
- Session data stored per-client
- Sessions cleaned up on disconnect
- Prevents cross-session interference

## Deployment

### Certificate Generation

**Development (self-signed)**:
```bash
# Generate private key (RSA 2048-bit)
openssl genrsa -out key.pem 2048

# Generate self-signed certificate
openssl req -new -x509 -key key.pem -out cert.pem -days 365
```

**Production**:
- Use certificates from trusted Certificate Authority
- Proper hostname/SAN configuration
- Regular renewal (before expiration)

### Server Configuration

**Example**:
```rust
let config = WssServerConfig {
    bind_addr: "0.0.0.0".to_string(),
    port: 8443,
    cert_path: Some("cert.pem".to_string()),
    key_path: Some("key.pem".to_string()),
    max_connections: 10000,
    buffer_size: 65536,
};

let mut server = WssServer::new(config, Arc::clone(&commy_server));
server.initialize_tls()?;
server.run().await?;
```

### Network Configuration

**Firewall Rules**:
- Open port 8443 for client connections
- Restrict to known client IP ranges if possible
- Enable rate limiting to prevent DoS

**Performance Tuning**:
- Adjust `buffer_size` based on message volume
- Monitor active session count
- Scale horizontally with multiple server instances

## Testing

### Unit Tests
```rust
#[test]
fn test_missing_certificate_error() {
    let result = TlsConfiguration::from_files("/nonexistent/cert.pem", "/nonexistent/key.pem");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_server_creation() {
    let config = WssServerConfig::default();
    let server = Arc::new(RwLock::new(Server::new()));
    let wss_server = WssServer::new(config, server);
    assert_eq!(wss_server.active_sessions().await, 0);
}
```

### Integration Tests
1. Create TLS certificates
2. Start WSS server
3. Connect client via WSS
4. Perform authentication
5. Access variables
6. Subscribe to changes
7. Verify broadcasts
8. Verify session cleanup on disconnect

## Compliance

### RFC 6455 Compliance
- ✅ WebSocket handshake
- ✅ Binary frame support
- ✅ Ping/Pong keepalive
- ✅ Graceful close
- ✅ Frame masking (client-to-server)

### RFC 5246 Compliance (TLS 1.2)
- ✅ TLS handshake
- ✅ Record protocol
- ✅ Cipher suites negotiation
- ✅ Certificate validation

### MessagePack Compliance
- ✅ Binary format serialization
- ✅ Type preservation
- ✅ Streaming support

## Performance Characteristics

### Latency
- TLS handshake: ~50-100ms (one-time)
- WebSocket handshake: ~10-20ms (one-time)
- Message round-trip: 1-10ms (network dependent)

### Throughput
- Binary frame handling: CPU-bound
- MessagePack serialization: ~100ns-1µs per message
- Network I/O: Limited by network bandwidth

### Scalability
- Per-connection task spawning: O(1)
- Session storage: O(n) where n = active connections
- Memory per connection: ~1KB + message buffers

## Future Enhancements

1. **Compression**: Add WebSocket Per-Message Deflate extension (RFC 7692)
2. **Connection Pooling**: Client-side connection reuse
3. **Batch Messages**: Send multiple messages in single frame
4. **Streaming**: Support for large variable transfers
5. **Metrics**: Instrumentation for performance monitoring
6. **Resilience**: Automatic reconnection with exponential backoff
