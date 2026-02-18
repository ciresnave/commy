# WebSocket Server Example - Complete Working Code

This document provides a complete, runnable example of the Commy WebSocket Secure (WSS) server implementation.

## Prerequisites

1. Rust toolchain installed
2. TLS certificates generated (see certificate generation section)
3. Commy library built and available

## Step 1: Generate TLS Certificates (Development)

Save as `generate_certs.sh`:

```bash
#!/bin/bash

# Generate a 2048-bit RSA private key
openssl genrsa -out key.pem 2048

# Generate a self-signed certificate (valid for 365 days)
openssl req -new -x509 -key key.pem -out cert.pem -days 365 \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"

echo "✅ Generated cert.pem and key.pem"
echo "Location: $(pwd)"
```

Run:
```bash
chmod +x generate_certs.sh
./generate_certs.sh
```

Output:
```
✅ Generated cert.pem and key.pem
Location: /your/project/directory
```

## Step 2: Main Server Code

File: `src/bin/wss_server.rs`

```rust
//! Complete WSS Server Example
//!
//! This example demonstrates:
//! - Creating a Commy Server
//! - Configuring TLS with certificates
//! - Starting the WebSocket Secure server
//! - Accepting remote client connections

use commy::server::{WssServer, WssServerConfig};
use commy::Server;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Commy WebSocket Secure Server");
    println!("==================================\n");

    // Create the Commy server (shared state for all clients)
    println!("📦 Creating Commy server...");
    let commy_server = Arc::new(RwLock::new(Server::new()));
    println!("   ✅ Commy server created\n");

    // Configure WebSocket Secure server
    println!("⚙️  Configuring WSS server...");
    let config = WssServerConfig {
        bind_addr: "0.0.0.0".to_string(),
        port: 8443,
        cert_path: Some("cert.pem".to_string()),
        key_path: Some("key.pem".to_string()),
        max_connections: 1000,
        buffer_size: 65536,
    };
    println!("   - Bind address: {}:{}", config.bind_addr, config.port);
    println!("   - Certificate: {}", config.cert_path.as_ref().unwrap());
    println!("   - Private key: {}", config.key_path.as_ref().unwrap());
    println!("   - Max connections: {}", config.max_connections);
    println!("   - Buffer size: {} bytes\n", config.buffer_size);

    // Create WSS server instance
    println!("🔒 Initializing TLS...");
    let mut wss_server = WssServer::new(config, Arc::clone(&commy_server));

    // Load and parse TLS certificates
    match wss_server.initialize_tls() {
        Ok(_) => println!("   ✅ TLS initialized successfully\n"),
        Err(e) => {
            eprintln!("   ❌ TLS initialization failed: {}", e);
            eprintln!("   Make sure cert.pem and key.pem exist in the current directory");
            return Err(e);
        }
    }

    // Start listening for connections
    println!("🌐 Starting WebSocket Secure server...");
    println!("   Listening on wss://0.0.0.0:8443");
    println!("   Ready to accept remote clients\n");
    println!("📡 Connection Protocol:");
    println!("   1. TLS 1.2+ handshake (RFC 5246)");
    println!("   2. WebSocket upgrade (RFC 6455)");
    println!("   3. Binary frame messaging (MessagePack)");
    println!("   4. Automatic ping/pong keepalive\n");

    // Run the server (never returns, keeps listening)
    wss_server.run().await?;

    Ok(())
}
```

## Step 3: Testing Client (Bash + OpenSSL)

File: `test_client.sh`

```bash
#!/bin/bash

# Simple WebSocket client test using curl and websocat

echo "🧪 Testing WebSocket Connection"
echo "================================\n"

# Check if wscat is installed
if ! command -v wscat &> /dev/null; then
    echo "❌ wscat not found. Install with:"
    echo "   npm install -g wscat"
    exit 1
fi

echo "📡 Connecting to wss://localhost:8443..."
echo "(Press Ctrl+C to exit)\n"

# Connect to the server
# --ca cert.pem allows self-signed certificates
wscat -c wss://localhost:8443 --ca cert.pem

echo "\n✅ Connection closed"
```

## Step 4: Testing Client (Python)

File: `test_client.py`

```python
#!/usr/bin/env python3
"""
Simple WebSocket client to test the Commy WSS server
"""

import asyncio
import websockets
import ssl
import json
import sys

async def test_wss_connection():
    """Test WebSocket Secure connection to Commy server"""
    
    # Create SSL context for self-signed certificates
    ssl_context = ssl.create_default_context()
    ssl_context.check_hostname = False
    ssl_context.verify_mode = ssl.CERT_NONE
    
    uri = "wss://localhost:8443"
    
    print("🧪 Testing WebSocket Connection")
    print("================================\n")
    print(f"📡 Connecting to {uri}...")
    
    try:
        async with websockets.connect(uri, ssl=ssl_context) as websocket:
            print("✅ Connected to WSS server\n")
            
            # Read and print server messages
            while True:
                try:
                    message = await asyncio.wait_for(
                        websocket.recv(), 
                        timeout=5.0
                    )
                    print(f"📨 Received {len(message)} bytes")
                    
                except asyncio.TimeoutError:
                    print("⏱️  No messages received (timeout)")
                    break
                    
    except ConnectionRefusedError:
        print("❌ Connection refused. Is the server running?")
        sys.exit(1)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)
    
    print("✅ Connection closed")

if __name__ == "__main__":
    asyncio.run(test_wss_connection())
```

## Step 5: Testing Client (Rust)

File: `tests/wss_client_test.rs`

```rust
#[cfg(test)]
mod wss_tests {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use tokio::task;

    #[tokio::test]
    #[ignore]  // Run manually: cargo test -- --ignored
    async fn test_wss_connection() {
        // This test assumes the WSS server is running on localhost:8443
        // with cert.pem and key.pem in the current directory
        
        let uri = "wss://127.0.0.1:8443";
        
        println!("🧪 Testing WSS connection to {}", uri);
        
        // Note: This will fail without proper TLS configuration
        // In a real test, you would use proper certificate verification
        match connect_async(uri).await {
            Ok((ws, _)) => {
                println!("✅ Connected to WSS server");
                
                // Send a test message
                let (mut write, mut read) = ws.split();
                
                let test_msg = Message::Text("test".to_string());
                if let Err(e) = write.send(test_msg).await {
                    println!("❌ Failed to send: {}", e);
                }
                
                // Read response
                if let Ok(Some(msg)) = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    read.next(),
                ).await {
                    println!("📨 Received: {:?}", msg);
                }
            }
            Err(e) => {
                println!("❌ Connection failed: {}", e);
            }
        }
    }
}
```

## Step 6: Cargo.toml Configuration

Ensure your `Cargo.toml` includes:

```toml
[package]
name = "commy"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "wss_server"
path = "src/bin/wss_server.rs"

[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.23"
tokio-rustls = "0.24"
rustls = "0.21"
rustls-pemfile = "1.0"
futures = "0.3"
rmp-serde = "1.1"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio-tungstenite = "0.23"
```

## Running the Server

### Step 1: Generate Certificates
```bash
cd /path/to/commy
openssl genrsa -out key.pem 2048
openssl req -new -x509 -key key.pem -out cert.pem -days 365 \
    -subj "/CN=localhost"
```

### Step 2: Run the Server
```bash
cargo run --bin wss_server --release

# Output:
# 🚀 Commy WebSocket Secure Server
# ==================================
#
# 📦 Creating Commy server...
#    ✅ Commy server created
#
# ⚙️  Configuring WSS server...
#    - Bind address: 0.0.0.0:8443
#    - Certificate: cert.pem
#    - Private key: key.pem
#    - Max connections: 1000
#    - Buffer size: 65536 bytes
#
# 🔒 Initializing TLS...
#    ✅ TLS initialized successfully
#
# 🌐 Starting WebSocket Secure server...
#    Listening on wss://0.0.0.0:8443
#    Ready to accept remote clients
```

### Step 3: Test Connection (in another terminal)

**Option A: Using wscat**
```bash
npm install -g wscat
wscat -c wss://localhost:8443 --ca cert.pem
```

**Option B: Using Python client**
```bash
pip install websockets
python test_client.py
```

**Option C: Using Rust tests**
```bash
cargo test -- --ignored test_wss_connection -- --nocapture
```

## Expected Output

### Server Output
```
🚀 Commy WebSocket Secure Server
==================================

📦 Creating Commy server...
   ✅ Commy server created

⚙️  Configuring WSS server...
   - Bind address: 0.0.0.0:8443
   - Certificate: cert.pem
   - Private key: key.pem
   - Max connections: 1000
   - Buffer size: 65536 bytes

🔒 Initializing TLS...
   ✅ TLS initialized successfully

🌐 Starting WebSocket Secure server...
   Listening on wss://0.0.0.0:8443
   Ready to accept remote clients

📡 Connection Protocol:
   1. TLS 1.2+ handshake (RFC 5246)
   2. WebSocket upgrade (RFC 6455)
   3. Binary frame messaging (MessagePack)
   4. Automatic ping/pong keepalive

TLS handshake successful: 127.0.0.1:54321
WebSocket connection established from 127.0.0.1:54321 (session_id: sess_abc123...)
```

### Client Output
```
🧪 Testing WebSocket Connection
================================

📡 Connecting to wss://localhost:8443...
✅ Connected to WSS server
```

## Troubleshooting

### Certificate Issues
```
❌ TLS initialization failed: Certificate file not found: cert.pem

Solution:
  1. cd to the project directory
  2. Run: openssl genrsa -out key.pem 2048
  3. Run: openssl req -new -x509 -key key.pem -out cert.pem -days 365
```

### Port Already in Use
```
❌ Error: Address already in use (os error 48)

Solution:
  1. Kill the previous process: lsof -ti:8443 | xargs kill -9
  2. Or change the port in WssServerConfig
```

### Connection Refused
```
❌ Connection refused. Is the server running?

Solution:
  1. Make sure cargo run is still executing
  2. Check that the server printed "Ready to accept remote clients"
  3. Verify the port matches (default 8443)
```

### Self-Signed Certificate Warning
```
❌ CERTIFICATE_VERIFY_FAILED

Solution:
  This is expected for self-signed certificates
  - With wscat: use --ca cert.pem flag
  - With Python: set ssl.verify_mode = ssl.CERT_NONE
  - In production: use proper CA-signed certificates
```

## Performance Testing

### Measure Connection Speed
```bash
time wscat -c wss://localhost:8443 --ca cert.pem --execute "ping"
```

### Load Testing
```bash
# Install Apache Bench (ab) if not present
# Create multiple connections
for i in {1..100}; do
    wscat -c wss://localhost:8443 --ca cert.pem &
done
wait
```

## Security Notes

### Development
- Self-signed certificates are fine for development
- Use `CERT_NONE` for testing only
- Keep key.pem secure

### Production
- Use certificates from trusted CA
- Enable proper certificate validation
- Use firewall to restrict access
- Monitor certificate expiration
- Use strong cipher suites
- Enable rate limiting
- Use authentication on all operations

## Code Structure

```
commy/
├── src/
│   ├── bin/
│   │   └── wss_server.rs          # Main server executable
│   ├── server/
│   │   ├── mod.rs                 # WssServer struct
│   │   ├── tls.rs                 # TLS configuration
│   │   └── ws_handler.rs          # WebSocket handler
│   └── lib.rs
├── tests/
│   └── wss_client_test.rs         # Client tests
├── cert.pem                        # Self-signed certificate
├── key.pem                         # Private key
├── Cargo.toml                      # Dependencies
└── README.md
```

## Next Steps

1. **Client Libraries**: Implement clients in JavaScript, Python, Go, etc.
2. **Authentication**: Integrate with your auth system
3. **Message Handlers**: Implement application-specific message handling
4. **Monitoring**: Add metrics and logging
5. **Load Testing**: Stress test with many concurrent connections
6. **Deployment**: Deploy to cloud/production environment

## References

- RFC 6455: WebSocket Protocol - https://tools.ietf.org/html/rfc6455
- RFC 5246: TLS 1.2 - https://tools.ietf.org/html/rfc5246
- Tokio-Tungstenite: https://docs.rs/tokio-tungstenite/
- Tokio-Rustls: https://docs.rs/tokio-rustls/
