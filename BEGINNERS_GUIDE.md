# Commy: A Beginner's Guide

**Welcome!** This guide will teach you what Commy is and how to use it, even if you're new to Rust.

---

## Part 1: What is Commy?

### The Simple Answer

Commy is a **shared memory coordination system** for Windows. Think of it like a bulletin board that multiple programs can read from and write to simultaneously, securely.

```
Program A ──┐
            ├──> [Commy Server] ──> Shared Memory File
Program B ──┤
Program C ──┘
```

Multiple programs can talk to the same Commy server and safely share data.

---

## Part 2: Why Would You Use Commy?

### Real-World Example: A Stock Trading System

Imagine you have three programs:
1. **Price Fetcher** - Gets latest stock prices from the internet
2. **Trading Bot** - Makes buy/sell decisions based on prices
3. **Dashboard** - Shows traders the current status

Without Commy, they'd need to:
- Write files to disk (slow)
- Use sockets over network (complex)
- Copy data between programs (error-prone)

With Commy:
✅ All three programs connect to one Commy server
✅ Price Fetcher updates stock prices in shared memory
✅ Trading Bot reads prices instantly (zero-copy)
✅ Dashboard sees changes in real-time

---

## Part 3: Key Concepts

### 3.1 - Server, Tenant, Service, Variables

Commy has a **4-level hierarchy**:

```
┌─ Commy Server (Port 8443)
│  │
│  ├─ Tenant: "Finance Department"
│  │  │
│  │  ├─ Service: "Stock Prices"
│  │  │  ├─ Variable: AAPL = 150.45
│  │  │  ├─ Variable: GOOGL = 143.20
│  │  │  └─ Variable: MSFT = 380.15
│  │  │
│  │  └─ Service: "Trade History"
│  │     ├─ Variable: Last Trade = "Buy 100 AAPL"
│  │     └─ Variable: Balance = $50,000
│  │
│  └─ Tenant: "Sales Department"
│     └─ Service: "Customer Leads"
│        ├─ Variable: New Leads = 5
│        └─ Variable: Qualified = 2
```

**What each level means:**

| Level | Purpose | Example |
|-------|---------|---------|
| **Server** | Entry point, handles all connections | Port 8443 (WSS) |
| **Tenant** | Organization/department (isolated) | "Finance", "Sales" |
| **Service** | Collection of related data | "Stock Prices", "Trade History" |
| **Variable** | Individual piece of data | AAPL price, Account balance |

### 3.2 - Clients

A **Client** is any program that connects to Commy. There are two types:

**Remote Client** (on different machine):
```
My Laptop ──[Internet/WSS]──> Commy Server ──> Shared Memory
```
- Connects over WebSocket Secure (WSS)
- Receives copies of data
- Sends commands to update data

**Local Client** (same machine):
```
My Program ──[Direct Memory Mapping]──> Commy Server ──> Shared Memory File
```
- Maps the memory file directly
- Ultra-fast zero-copy access
- Only works on same machine

### 3.3 - Authentication & Permissions

Before a client can do anything, it must authenticate:

```
Client: "Hi, I'm John from Finance"
        ↓
Commy: "Prove it - show me your credentials"
        ↓
Client: "Here's my API key: xxx..."
        ↓
Commy: "✓ Verified. You can READ stock prices and WRITE trades"
```

**Permissions** are what you're allowed to do:
- `ServiceRead` - Can read variables
- `ServiceWrite` - Can update variables
- `ServiceAdmin` - Can delete/manage services

---

## Part 4: How Commy Works Behind the Scenes

### 4.1 - Message Flow

```
Client Program                          Commy Server
     │                                      │
     ├─ "Hello, authenticate me"           │
     │                                      │
     │◄─────── Server validates ──────────┤
     │                                      │
     ├─ "Get the AAPL price"              │
     │                                      │
     │◄─── Fetch from shared memory ──────┤
     │       (AAPL = 150.45)               │
     │                                      │
     ├─ "Update AAPL to 150.67"           │
     │                                      │
     │◄─── Write to shared memory ───────┤
     │     Update other clients ───────────┤
```

### 4.2 - Shared Memory Files

Behind the scenes, Commy creates **memory-mapped files** for each service:

```
Disk Storage:
├── tenant_finance/
│   ├── service_stock_prices_abc123.mem  (1 MB)
│   └── service_trade_history_def456.mem (500 KB)
├── tenant_sales/
│   └── service_leads_ghi789.mem (256 KB)
```

These files are:
- **Persistent** - Data survives program restarts
- **Multi-process** - Multiple programs can access simultaneously
- **Memory-mapped** - Direct memory access (fast)
- **Secure** - Only authenticated clients can access

---

## Part 5: Using Commy - Step by Step

### Step 1: Start the Server

On your Windows machine:

```powershell
# Set environment variables
$env:COMMY_TLS_CERT_PATH = "cert.pem"
$env:COMMY_TLS_KEY_PATH = "key.pem"
$env:COMMY_LISTEN_ADDR = "127.0.0.1"
$env:COMMY_LISTEN_PORT = "8443"

# Start the server
.\commy.exe
```

The server is now listening on `wss://127.0.0.1:8443`

### Step 2: Connect as a Client

**Client A - Price Fetcher**:
```javascript
// Connect to Commy
const client = new CommyClient("wss://127.0.0.1:8443");

// Authenticate
await client.authenticate({
  tenant: "finance",
  credentials: "my-api-key"
});

// Create a service for prices
const priceService = await client.getService("stock_prices");

// Set price variable
await priceService.setVariable("AAPL", 150.45);

// Broadcast to all listeners
await priceService.notifyChange("AAPL");
```

**Client B - Trading Bot**:
```javascript
const client = new CommyClient("wss://127.0.0.1:8443");
await client.authenticate({ tenant: "finance", ... });

const priceService = await client.getService("stock_prices");

// Subscribe to price changes
priceService.on("change", (variable, newValue) => {
  console.log(`${variable} changed to ${newValue}`);
  
  if (variable === "AAPL" && newValue > 150) {
    console.log("SELL signal: AAPL too high!");
  }
});

// Read a price
const price = await priceService.getVariable("AAPL");
console.log(`Current AAPL price: ${price}`);
```

**Client C - Dashboard**:
```javascript
const client = new CommyClient("wss://127.0.0.1:8443");
await client.authenticate({ tenant: "finance", ... });

setInterval(async () => {
  const priceService = await client.getService("stock_prices");
  const aapl = await priceService.getVariable("AAPL");
  const googl = await priceService.getVariable("GOOGL");
  
  updateUI({
    aapl_price: aapl,
    googl_price: googl
  });
}, 1000);
```

---

## Part 6: Real-World Examples

### Example 1: IoT Sensor Network

**Setup:**
- 100 temperature sensors around a building
- A monitoring service
- An alert system

**With Commy:**
```
Sensor 1 ──┐
Sensor 2 ──┤
Sensor 3 ──├──> Commy Service: "Building Temps"
...        │     Variables: Temp_Floor1, Temp_Floor2, ...
Sensor 100┘
           │
           └──> Monitoring: Reads all temps every second
           │
           └──> Alerts: Sends email if any temp > 100°F
```

Benefits:
- Sensors don't know about each other
- Monitoring and alerts are independent
- All in one place (shared memory)

### Example 2: Game Server Coordination

**Setup:**
- Game server
- Chat server
- Leaderboard tracker

**With Commy:**
```
Game Server ──┐
              ├──> Commy Service: "Game State"
Chat Server ──┤     Variables:
              │     - Active Players
              ├──> Commy Service: "Chat"
Leaderboard ──┤     Variables:
              │     - Recent Messages
              └──> Commy Service: "Scores"
                    Variables:
                    - Player Rankings
```

All three services stay in sync automatically.

### Example 3: Financial Trading

**Setup:**
- Price feed (external data)
- Trading algorithms
- Risk manager
- Reporting system

**With Commy:**
```
Price Feed ──┐
             ├──> Commy Service: "Market Data"
Trading Bot ─┤     Variables:
             │     - Bid/Ask prices
Risk Manager ├──> Commy Service: "Positions"
             │     Variables:
Reporting ───┤     - Open trades
             │     - P&L
             └──> Commy Service: "Alerts"
                   Variables:
                   - Risk warnings
```

---

## Part 7: Common Questions

### Q: How is this different from a database?

**Database:**
- Designed for complex queries
- Data stored permanently on disk
- Slower (round-trip over network)
- Best for: Historical data, archives

**Commy:**
- Designed for real-time sharing
- Data in memory (faster)
- Permission-based access
- Best for: Live data, inter-process communication

### Q: How is this different from Redis?

**Redis:**
- Key-value store (like a dictionary)
- Great for caching
- Network-based

**Commy:**
- Shared memory system
- Whole collections (arrays, strings, maps)
- Local or remote access
- Memory-mapped files

### Q: Is my data secure?

**Yes**, because:
1. ✅ TLS encryption (HTTPS-like security)
2. ✅ Authentication required before access
3. ✅ Permission system (read/write controls)
4. ✅ Client isolation (can't see other tenants' data)

### Q: What if my program crashes?

Commy keeps the shared memory file intact:
- ✅ Data is not lost
- ✅ Other programs keep running
- ✅ Your program can restart and reconnect

---

## Part 8: Architecture Deep Dive

### How Data Flows from One Client to Another

```
Client A (Writer)          Shared Memory           Client B (Reader)
      │                          │                        │
      ├─ "Set AAPL=150"         │                        │
      │                          │                        │
      │──────────────────────────┼────────────────────────┤
      │                          │                        │
      │                  [Update memory]                  │
      │                          │                        │
      │                    [Notify change]                │
      │                          │                        │
      │                          │◄───────────────────────┤
      │                          │      (notification)    │
      │                          │                        │
      │                       [Client B reads]            │
      │                          │                        │
      │                   [Gets 150 immediately]          │
```

### Local vs Remote Clients

**Local Client (Same Machine):**
```
Your Program
    │
    └─> Memory Map File
        (Direct, instant access)
        ↓
    Shared Memory
```
Speed: **Microseconds**

**Remote Client (Different Machine):**
```
Your Program
    │
    └─> WSS Connection (WebSocket over TLS)
        └─> Network ──> Commy Server
            └─> Memory Map File
                └─> Shared Memory
```
Speed: **Milliseconds** (network latency)

---

## Part 9: Getting Started Tutorial

### Objective: Create a simple temperature monitoring system

**Step 1: Create a temperature service**

```javascript
const client = new CommyClient("wss://localhost:8443");
await client.authenticate({
  tenant: "building",
  method: "api_key",
  credentials: "my-key-12345"
});

const tempService = await client.getService("temperatures");

// Create variables for each room
await tempService.createVariable("room_101", 72.5);  // °F
await tempService.createVariable("room_102", 71.2);
await tempService.createVariable("room_103", 73.8);
```

**Step 2: Update temperatures (Sensor Program)**

```javascript
setInterval(async () => {
  const current = await sensor.readTemperature();
  await tempService.setVariable("room_101", current);
}, 5000);  // Every 5 seconds
```

**Step 3: Monitor temperatures (Alert Program)**

```javascript
tempService.on("change", async (room, temp) => {
  if (temp > 75) {
    alert(`⚠️ ${room} is HOT: ${temp}°F`);
  }
});
```

**Step 4: Display temperatures (Dashboard Program)**

```javascript
setInterval(async () => {
  const temp101 = await tempService.getVariable("room_101");
  const temp102 = await tempService.getVariable("room_102");
  const temp103 = await tempService.getVariable("room_103");
  
  updateDisplay({
    room_101: temp101,
    room_102: temp102,
    room_103: temp103
  });
}, 1000);  // Every second
```

**Result:**
- Sensor updates temperatures
- Alerts fire for high temps
- Dashboard shows live data
- All in sync automatically

---

## Part 10: Commy Ecosystem

### Clients (SDKs)

```
Commy Server
    │
    ├─ Python SDK   (commy-py)
    ├─ JavaScript SDK (commy-js)
    ├─ Rust SDK      (commy-rs)
    ├─ Java SDK      (commy-java)
    └─ ... more coming
```

Each SDK provides:
- Connection management
- Authentication
- Permission handling
- Message routing
- Subscribe/publish system

### Clustering

For large deployments, you can run **multiple Commy servers**:

```
Commy Server 1  ──┐
                  ├─> Cluster Coordination
Commy Server 2  ──┤
                  ├─> Data Replication
Commy Server 3  ──┘
```

Benefits:
- High availability
- Load balancing
- Geographic distribution
- Automatic failover

---

## Part 11: Security Model

### Three Levels of Security

**Level 1: Connection Security (TLS)**
```
Client ──[Encrypted Connection]──> Server
        (like HTTPS)
```

**Level 2: Authentication**
```
Client: "I'm user@company.com, here's my JWT token"
Server: "Let me verify... ✓ You're authenticated"
```

**Level 3: Authorization (Permissions)**
```
Client: "Can I read 'salary_data'?"
Server: "No, you don't have ServiceRead permission"

Client: "Can I read 'project_status'?"
Server: "Yes, you have ServiceRead permission ✓"
```

### Multi-Tenant Isolation

Each tenant is completely isolated:

```
Tenant: Finance         Tenant: Marketing
├─ Services            ├─ Services
├─ Variables           ├─ Variables
├─ Users/Permissions   ├─ Users/Permissions
(Finance DB)           (Marketing DB)

They never see each other's data!
```

---

## Part 12: Performance Characteristics

### Speed Comparison

| Operation | Time | How |
|-----------|------|-----|
| Read local variable | 0.001 ms | Direct memory access |
| Write local variable | 0.001 ms | Direct memory access |
| Read remote variable | 5-20 ms | Network + server |
| Get notification | 1-10 ms | WSS broadcast |

**Example:** Trading system with 1000 updates/second:
- Local clients: **Zero slowdown**
- Remote clients: **Millisecond delays** (acceptable)

---

## Part 13: Troubleshooting

### Problem: "Connection refused"
```
Client: "Can you hear me?"
Server: [Not listening]

Solution: Start the server first!
```

### Problem: "Authentication failed"
```
Client: "Here's my token: xxx"
Server: "That's not valid"

Solution: Check your API key or JWT token is correct
```

### Problem: "Permission denied"
```
Client: "Can I read temperature data?"
Server: "No, you only have Write permission"

Solution: Ask your admin to grant Read permission
```

### Problem: "Service not found"
```
Client: "Where's the 'prices' service?"
Server: "I don't have it"

Solution: Create the service first with createService()
```

---

## Part 14: Comparison with Alternatives

### Commy vs Message Queues (RabbitMQ, Kafka)

| Feature | Commy | Message Queue |
|---------|-------|---------------|
| Real-time sync | ✅ | ❌ |
| Persistent | ✅ | ✅ |
| Multi-recipient | ✅ | ✅ |
| Guaranteed delivery | ❌ | ✅ |
| Best for | Live data | Event streaming |

### Commy vs APIs (REST)

| Feature | Commy | REST API |
|---------|-------|----------|
| Speed | ✅ Fast | ❌ Slow |
| Real-time push | ✅ | ❌ |
| Network efficient | ✅ | ❌ |
| Query complex | ❌ | ✅ |

### Commy vs Files

| Feature | Commy | Files |
|---------|-------|-------|
| Multi-access | ✅ Safe | ❌ Risky |
| Real-time | ✅ | ❌ Slow |
| Atomicity | ✅ | ❌ |
| Size limits | ❌ | ✅ Unlimited |

---

## Part 15: Cheat Sheet

### Starting Commy
```powershell
$env:COMMY_TLS_CERT_PATH = "cert.pem"
$env:COMMY_TLS_KEY_PATH = "key.pem"
.\commy.exe
```

### Client Connection
```javascript
const client = new CommyClient("wss://localhost:8443");
await client.authenticate({ tenant: "myapp", credentials: "key" });
```

### Working with Services
```javascript
const service = await client.getService("my_service");
await service.setVariable("myvar", value);
const val = await service.getVariable("myvar");
```

### Subscribing to Changes
```javascript
service.on("change", (varName, newValue) => {
  console.log(`${varName} = ${newValue}`);
});
```

### Permissions
```
ServiceRead  = Can read variables
ServiceWrite = Can modify variables
ServiceAdmin = Can delete/manage
```

---

## Conclusion

**Commy** is a shared memory coordination system that lets multiple programs **communicate in real-time** through:

1. ✅ A central server
2. ✅ Secure authentication
3. ✅ Permission-based access
4. ✅ Ultra-fast shared memory
5. ✅ Real-time change notifications

It's perfect for:
- **Real-time systems** (trading, monitoring)
- **Multi-process coordination** (services working together)
- **Live data sharing** (prices, sensor data, status)
- **High-frequency updates** (thousands per second)

**Remember:** Commy isn't better than databases or APIs—it's *different*. Use it when you need real-time coordination between local processes!

---

## Next Steps

1. **Read** ARCHITECTURE.md for technical details
2. **Run** the test suite: `cargo test`
3. **Deploy** your first Commy service
4. **Build** your first client application

Good luck! 🚀
