# Commy Quick Reference Guide

**Quick lookup for common tasks.** For detailed explanations, see other guides.

---

## TL;DR - What is Commy?

**Commy is a shared memory server that lets multiple programs talk to each other in real-time.**

```
Program A ──┐
Program B ──├──> [Commy] ──> Shared Memory ──> All programs see changes instantly
Program C ──┘
```

---

## Getting Started (5 minutes)

### 1. Start Server

```powershell
cd C:\commy

$env:COMMY_TLS_CERT_PATH = "dev-cert.pem"
$env:COMMY_TLS_KEY_PATH = "dev-key.pem"
$env:COMMY_LISTEN_ADDR = "127.0.0.1"
$env:COMMY_LISTEN_PORT = "8443"

.\target\release\commy.exe
```

### 2. Connect a Client

```javascript
const ws = new WebSocket("wss://127.0.0.1:8443", {
    rejectUnauthorized: false  // Dev only
});

ws.send(JSON.stringify({
    type: "Authenticate",
    payload: {
        tenant_name: "my_app",
        method: "api_key",
        credentials: "my-secret-key"
    }
}));
```

### 3. Send Data

```javascript
ws.send(JSON.stringify({
    type: "SetVariables",
    payload: {
        tenant_name: "my_app",
        service_name: "data",
        variables: {
            status: "processing",
            progress: "45%"
        }
    }
}));
```

### 4. Read Data

```javascript
ws.send(JSON.stringify({
    type: "GetVariables",
    payload: {
        tenant_name: "my_app",
        service_name: "data"
    }
}));
```

---

## Concepts at a Glance

### Hierarchy

```
Server
├─ Tenant: "finance"
│  ├─ Service: "prices"
│  │  ├─ Variable: AAPL = 150.45
│  │  ├─ Variable: GOOGL = 143.20
│  │  └─ Watchers: [client1, client2]
│  └─ Service: "trades"
│     └─ Variable: LastTrade = "Buy 100"
└─ Tenant: "sales"
   └─ Service: "leads"
      └─ Variable: Count = 5
```

### Key Terms

| Term | Meaning |
|------|---------|
| **Server** | Central Commy instance (port 8443) |
| **Tenant** | Organization/division (isolated) |
| **Service** | Collection of related variables |
| **Variable** | Individual piece of data |
| **Client** | Program connecting to Commy |
| **Authenticated** | Client has credentials verified |
| **Permission** | What client can do (read/write) |

---

## Message Types

### Authentication

```javascript
// Login
{
  "type": "Authenticate",
  "payload": {
    "tenant_name": "my_app",
    "method": "api_key",  // or "jwt"
    "credentials": "secret-key-123"
  }
}

// Response
{
  "type": "AuthResponse",
  "success": true,
  "token": "auth-token-xyz",
  "permissions": ["ServiceRead", "ServiceWrite"]
}
```

### Variables

```javascript
// Get variables
{
  "type": "GetVariables",
  "payload": {
    "tenant_name": "my_app",
    "service_name": "data"
  }
}

// Set variables
{
  "type": "SetVariables",
  "payload": {
    "tenant_name": "my_app",
    "service_name": "data",
    "variables": {
      "status": "ready",
      "count": "42"
    }
  }
}

// Variable changed notification
{
  "type": "VariableChanged",
  "payload": {
    "service_name": "data",
    "variables": {
      "status": "processing"
    }
  }
}
```

### Subscriptions

```javascript
// Watch for changes
{
  "type": "Subscribe",
  "payload": {
    "tenant_name": "my_app",
    "service_name": "data",
    "variables": ["status", "count"]
  }
}

// Stop watching
{
  "type": "Unsubscribe",
  "payload": {
    "tenant_name": "my_app",
    "service_name": "data",
    "variables": ["status"]
  }
}
```

### Misc

```javascript
// Keep connection alive
{"type": "Heartbeat"}

// Disconnect
{"type": "Logout"}

// Error response (any failure)
{
  "type": "Error",
  "message": "Permission denied",
  "code": "FORBIDDEN"
}
```

---

## Code Snippets

### JavaScript Client

```javascript
const WebSocket = require('ws');

const client = {
    ws: null,
    authenticated: false,
    
    async connect(url) {
        this.ws = new WebSocket(url, { rejectUnauthorized: false });
        return new Promise(resolve => {
            this.ws.on('open', resolve);
        });
    },
    
    async authenticate(tenant, key) {
        this.ws.send(JSON.stringify({
            type: "Authenticate",
            payload: {
                tenant_name: tenant,
                method: "api_key",
                credentials: key
            }
        }));
    },
    
    async getVariable(tenant, service, variable) {
        this.ws.send(JSON.stringify({
            type: "GetVariables",
            payload: {
                tenant_name: tenant,
                service_name: service,
                variables: [variable]
            }
        }));
    },
    
    async setVariable(tenant, service, name, value) {
        this.ws.send(JSON.stringify({
            type: "SetVariables",
            payload: {
                tenant_name: tenant,
                service_name: service,
                variables: { [name]: value }
            }
        }));
    },
    
    subscribe(tenant, service, variables) {
        this.ws.send(JSON.stringify({
            type: "Subscribe",
            payload: {
                tenant_name: tenant,
                service_name: service,
                variables: variables
            }
        }));
    },
    
    on(event, callback) {
        this.ws.on('message', (data) => {
            const msg = JSON.parse(data);
            if (msg.type === event) {
                callback(msg.payload);
            }
        });
    }
};

// Usage
(async () => {
    await client.connect('wss://127.0.0.1:8443');
    await client.authenticate('myapp', 'key123');
    
    client.on('VariableChanged', (data) => {
        console.log('Variable changed:', data);
    });
    
    client.subscribe('myapp', 'data', ['status']);
})();
```

---

## Environment Variables

### Required

```
COMMY_TLS_CERT_PATH=./dev-cert.pem     # TLS certificate
COMMY_TLS_KEY_PATH=./dev-key.pem       # TLS private key
```

### Optional

```
COMMY_LISTEN_ADDR=0.0.0.0              # Default
COMMY_LISTEN_PORT=8443                 # Default
COMMY_SERVER_ID=node-1                 # Default
COMMY_CLUSTER_ENABLED=false             # Default
ENVIRONMENT=development                 # For memory auth backend
```

---

## Architecture Overview

### Client Connection Flow

```
1. Client ──[WSS Connect]──> Server
2. Client ──[Authenticate]──> Server validates credentials
3. Server ──[Grant Token]──> Client
4. Client ──[GetVariables]──> Server reads from memory
5. Server ──[VariableData]──> Client
6. Client ──[Subscribe]──> Server watches for changes
7. Other Client ──[SetVariables]──> Server updates memory
8. Server ──[Notify]──> Both clients notified
```

### Memory Layout

```
Shared Memory File (tenant_service_abc123.mem)
┌─────────────────────────────────────────────┐
│ Header (metadata)                           │ 16 bytes
├─────────────────────────────────────────────┤
│ AAPL price          = 150.45               │ 8 bytes
├─────────────────────────────────────────────┤
│ GOOGL price         = 143.20               │ 8 bytes
├─────────────────────────────────────────────┤
│ LastTrade message   = "Buy 100 AAPL"       │ 24 bytes
├─────────────────────────────────────────────┤
│ Free space                                  │ ... remaining
└─────────────────────────────────────────────┘

Multiple processes read/write this file simultaneously!
```

---

## Common Patterns

### Pattern 1: Real-Time Dashboard

```javascript
// Program A: Data Source
async function updateData() {
    setInterval(() => {
        client.setVariable('app', 'status', 'cpu_usage', getCPU());
        client.setVariable('app', 'status', 'memory_usage', getMemory());
    }, 1000);
}

// Program B: Display
async function displayData() {
    client.subscribe('app', 'status', ['cpu_usage', 'memory_usage']);
    client.on('VariableChanged', (vars) => {
        console.log(`CPU: ${vars.cpu_usage}%`);
        console.log(`RAM: ${vars.memory_usage}%`);
    });
}
```

### Pattern 2: Configuration Sync

```javascript
// Program A: Configuration Server
async function pushConfig() {
    client.setVariable('app', 'config', 'log_level', 'debug');
    client.setVariable('app', 'config', 'max_connections', '1000');
}

// Program B: Service Reading Config
async function syncConfig() {
    const config = await client.getVariable('app', 'config');
    applyConfig(config);
    
    // Watch for changes
    client.subscribe('app', 'config', ['*']);
    client.on('VariableChanged', applyConfig);
}
```

### Pattern 3: Multi-Client Coordination

```javascript
// All clients watch the same service
client.subscribe('app', 'coordinator', ['current_task']);

// Client A: Update task
client.setVariable('app', 'coordinator', 'current_task', 'processing');

// Clients B, C, D: Immediately notified
client.on('VariableChanged', (vars) => {
    if (vars.current_task === 'processing') {
        // Other client started task, wait
    }
});
```

---

## Performance Characteristics

| Operation | Local | Remote | Notes |
|-----------|-------|--------|-------|
| Read variable | 0.001ms | 5-20ms | Local is zero-copy |
| Write variable | 0.001ms | 5-20ms | Remote has network latency |
| Get notification | N/A | 1-10ms | Broadcast to subscribers |
| Allocate | 0.035ms | N/A | Server-side only |

### Throughput

- **Local:** 1,000,000+ ops/sec per process
- **Remote:** 1,000-10,000 ops/sec per client
- **Broadcast:** 1,000+ clients notified simultaneously

---

## Security

### Three Levels

```
1. TLS (HTTPS-like encryption)
   ├─ WSS protocol (WebSocket Secure)
   └─ Certificate validation

2. Authentication (Who are you?)
   ├─ API key
   ├─ JWT token
   └─ Custom methods

3. Authorization (What can you do?)
   ├─ ServiceRead (read variables)
   ├─ ServiceWrite (modify variables)
   ├─ ServiceAdmin (delete/manage)
   └─ TenantLevel (access tenant)
```

### Multi-Tenant Isolation

```
Tenant A                          Tenant B
├─Service 1                       ├─Service 1
│ ├─Var 1  ✓ Client A can see   │ ├─Var 1  ✗ Client A cannot see
│ └─Var 2                        │ └─Var 2
├─Service 2                       ├─Service 2
│ └─Var 1  ✓                     │ └─Var 1  ✗
└─Auth: Client A                  └─Auth: Client B (different)
```

---

## Troubleshooting Checklist

| Problem | Solutions |
|---------|-----------|
| Connection refused | Server running? Port 8443 open? |
| Auth failed | Correct tenant name? API key valid? |
| Permission denied | Do you have ServiceRead/Write? |
| Variable not found | Did you SetVariable first? |
| No notifications | Did you Subscribe? Is service sending updates? |
| Slow performance | Local or remote client? Network latency? |
| Memory full | Allocate more space? Delete old variables? |
| TLS error | Valid certificates? Correct paths in env vars? |

---

## Common Tasks

### Task 1: Create a Service

```javascript
// Just start setting variables - service auto-creates
client.setVariable('mytenant', 'myservice', 'myvar', 'value');
```

### Task 2: Read All Variables

```javascript
client.ws.send(JSON.stringify({
    type: "GetVariables",
    payload: {
        tenant_name: "mytenant",
        service_name: "myservice",
        // No specific variables = get all
    }
}));
```

### Task 3: Delete a Variable

```javascript
// Note: Not directly supported, but you can:
// 1. Set to empty string
client.setVariable('mytenant', 'myservice', 'oldvar', '');

// 2. Or use admin API (requires ServiceAdmin permission)
```

### Task 4: Watch Multiple Variables

```javascript
client.subscribe('mytenant', 'myservice', 
    ['price', 'volume', 'timestamp']);
    
client.on('VariableChanged', (vars) => {
    // Called whenever ANY of these change
    console.log(vars);
});
```

### Task 5: Batch Update

```javascript
client.ws.send(JSON.stringify({
    type: "SetVariables",
    payload: {
        tenant_name: "mytenant",
        service_name: "myservice",
        variables: {
            // All updated atomically
            price: "150.45",
            volume: "1000000",
            timestamp: "2026-02-15T07:45:00Z"
        }
    }
}));
```

---

## Production Checklist

Before deploying:

- [ ] Use real TLS certificates (not self-signed)
- [ ] Set up database backend (PostgreSQL/MySQL)
- [ ] Configure auth method (JWT/API keys)
- [ ] Set up monitoring/logging
- [ ] Test with realistic load
- [ ] Plan backup strategy
- [ ] Document your services
- [ ] Set up client SDKs
- [ ] Create runbooks for ops
- [ ] Set up clustering (optional)

---

## Resources

| Document | For |
|----------|-----|
| BEGINNERS_GUIDE.md | What is Commy overview |
| RUST_BASICS.md | Learning Rust basics |
| PRACTICAL_TUTORIAL.md | Building real applications |
| ARCHITECTURE.md | Deep technical details |
| TEST_RESULTS.md | Test coverage & performance |

---

## One-Liner Help

```bash
# Start server
$env:COMMY_TLS_CERT_PATH="cert.pem"; .\commy.exe

# Test with websocat
wscat -c wss://127.0.0.1:8443 --no-check

# Run tests
cargo test

# Build release
cargo build --release
```

---

**Need more help?** Check the detailed guides or look at PRACTICAL_TUTORIAL.md for examples!
