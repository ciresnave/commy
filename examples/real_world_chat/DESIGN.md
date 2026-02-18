# Chat System Design Guide

**Why we built it this way and how Commy makes it work.**

## Design Philosophy

This chat system is a blueprint for any real-time broadcasting application. Every design decision prioritizes:

1. **Instant Delivery** - <10ms latency, not seconds
2. **Zero Polling** - Event-driven, no wasted requests
3. **Scalability** - Works the same with 5 users or 5000
4. **Simplicity** - No complex caching layers needed
5. **Reliability** - Messages never lost (stored in shared memory)

## 🏗️ Architecture Decisions

### Decision 1: Commy as the Message Store

**What we chose:** Store messages directly in Commy shared memory

**Why NOT a traditional database:**

```
Traditional Approach (PostgreSQL):
┌─────────────────────────────────────────────┐
User types "Hello"
     ↓
Client sends to Server
     ↓
Server validates and saves to PostgreSQL
     ↓
PostgreSQL writes to disk (5-10ms)
     ↓
Server sends "OK" to client
     ↓
Server queries for new messages every 2 seconds
     ↓
Client waits... 0-2000ms for new message
     ↓
New message appears
└─────────────────────────────────────────────┘
TOTAL LATENCY: 5-2010ms ❌
```

```
Commy Approach (Shared Memory):
┌──────────────────────────────────┐
User types "Hello"
     ↓
Client writes to Commy memory
     ↓
All subscribed clients instantly notified (<1ms)
     ↓
New message appears in all clients
└──────────────────────────────────┘
TOTAL LATENCY: <10ms ✅
```

**The key difference:**
- Database: async write + async poll read = slow
- Commy: zero-copy write + instant broadcast = fast

### Decision 2: Tenant-Per-Room

**What we chose:** Each room is a separate Commy tenant

```
Architecture:
Server (Commy)
├─ Tenant: room_lobby
│  ├─ Service: messages
│  ├─ Service: presence
│  └─ Service: typing
├─ Tenant: room_gaming
│  ├─ Service: messages
│  ├─ Service: presence
│  └─ Service: typing
└─ Tenant: room_support
   ├─ Service: messages
   ├─ Service: presence
   └─ Service: typing
```

**Why per-tenant (not per-service):**

```
Option A: Single tenant, multiple services (BAD)
──────────────────────────────────────────────
Tenant: chat_system
└─ Service: room_lobby_messages
└─ Service: room_gaming_messages
└─ Service: room_support_messages

Problem:
- All rooms share permission context
- One malicious user could see ALL rooms (permission leak)
- Hard to scale (one permission check for all)
- Auditing is difficult (who accessed what room?)
```

```
Option B: Per-tenant per-room (GOOD) ✅
──────────────────────────────────────────────
Tenant: room_lobby
└─ Service: messages
└─ Service: presence

Tenant: room_gaming
└─ Service: messages
└─ Service: presence

Benefits:
- Complete isolation (Alice can be in lobby, can't see gaming)
- Per-room authentication (different API keys per room)
- Scaling is natural (add more rooms = add more tenants)
- Auditing is clear (per-tenant logs)
```

### Decision 3: Three Services Per Room

**What we chose:**

```
Service 1: messages (the chat messages)
├─ Variable: message_count (how many total)
└─ Variable: message_list (array of all messages)

Service 2: presence (who's online)
├─ Variable: active_users (list with join times)
└─ Variable: user_<id>_status (online/idle/away)

Service 3: typing (who's composing)
└─ Variable: typing_users (names of users currently typing)
```

**Why three services (not one):**

```
Option A: One service for everything (BAD)
──────────────────────────────────────────
Service: room_state
├─ Variable: messages = [big array]
├─ Variable: typing_users = ["Alice"]
└─ Variable: presence = {active_users}

Problem:
- Every typing indicator update = rewrite entire message array to disk
- Presence change = rewrite messages (15MB) + presence (small)
- Change detection fires on all variables (inefficient)
```

```
Option B: Three focused services (GOOD) ✅
──────────────────────────────────────────
Service: messages    ← Only changes when new message
Service: presence    ← Only changes when join/leave
Service: typing      ← Only changes when type start/stop

Benefits:
- Typing indicator doesn't touch message service
- Joining room doesn't require message rewrite
- Change detection is granular (only notify if relevant)
- Each service can have independent retention policy
```

### Decision 4: Array-Based Message Storage

**What we chose:** Store messages as a growing array in shared memory

```rust
// In Commy shared memory:
message_list = [
    Message { id: 1, user: "Alice", text: "Hello", ts: 100 },
    Message { id: 2, user: "Bob", text: "Hi!", ts: 105 },
    Message { id: 3, user: "Alice", text: "How are you?", ts: 110 },
    ...
]
```

**Why array (not key-value):**

```
Option A: Key-value storage (BAD for chat)
──────────────────────────────────────────
HashMap<msg_id, Message>
├─ Problem: Membership changes are hard to detect
├─ Problem: Ordering is implicit (need separate index)
└─ Problem: Change notifications don't tell you "which messages are new"

Option B: Array-based (GOOD) ✅  
──────────────────────────────────────────
Vec<Message>
├─ Benefit: Natural ordering (chronological)
├─ Benefit: New messages = append (constant time)
├─ Benefit: Change notification = "array grew" (easy)
├─ Benefit: New client reads vec[last_50..] (simple slice)
└─ Benefit: Compatible with CRDT algorithms if needed
```

**Performance consideration:**

```
Array size: 100,000 messages (typical room with 1 year history)
Message size: ~200 bytes
Total memory: ~20MB

Cost of appending one message:
- Write to shared memory: ~0.001ms
- Notify all subscribers: <1ms
- Total: <1.1ms

Why this works: Commy writes only the NEW message,
not the entire array. Memory-mapped file handles this efficiently.
```

### Decision 5: Client-Side Subscription Management

**What we chose:** Clients subscribe to services they care about

```rust
// Client joins room_lobby
client.subscribe("room_lobby", "messages", ["message_list"]).await?;
client.subscribe("room_lobby", "presence", ["active_users"]).await?;
client.subscribe("room_lobby", "typing", ["typing_users"]).await?;

// Client is notified of changes to these variables
// No polling needed
```

**Why client drives subscriptions (not server push):**

```
Option A: Server pushes updates (BAD)
──────────────────────────────────────
Server maintains a list of all clients
When message arrives, server sends to each client
Problems:
- Server carries all connection state (not scalable)
- If server restarts, clients don't know which data they had
- Hard to implement backpressure (what if client is slow?)

Option B: Client subscribes to Commy (GOOD) ✅
──────────────────────────────────────────────
Client tells Commy: "Notify me when message_list changes"
Client receives notifications from Commy, not server
Benefits:
- Clients don't depend on server (subscribe directly to Commy)
- Server is stateless (just coordinates initial join)
- Backpressure handled by Commy (queable)
- Client disconnects = automatic cleanup (Commy notices)
```

### Decision 6: Presence as Structured Data

**What we chose:** Store user presence as a structured list

```rust
#[derive(Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: String,
    pub username: String,
    pub joined_at: i64,
    pub last_active: i64,
    pub status: UserStatus,  // Online, Idle, Away
}

// Stored in Commy:
active_users = vec![
    UserPresence { user_id: "u1", username: "Alice", ... },
    UserPresence { user_id: "u2", username: "Bob", ... },
]
```

**Why structure (not just string list):**

```
Option A: Simple string list (BAD for features)
───────────────────────────────────────────────
typing_users = ["Alice", "Bob"]

Problem:
- No metadata (when did they join?)
- No idle detection (how long inactive?)
- Hard to sort "who's been here longest"
```

```
Option B: Structured presence (GOOD) ✅
────────────────────────────────────────
active_users = [
    { user: "Alice", joined: 1000, last_active: 1050, status: "online" },
    { user: "Bob", joined: 1100, last_active: 1140, status: "idle" },
]

Benefits:
- Can show "Alice joined 5 minutes ago"
- Can detect idle (last_active < now - 5min)
- Can sort by join time, activity, etc.
- Can implement "last seen" feature
```

### Decision 7: Typing as Transient Data

**What we chose:** Typing indicator as a separate, lightweight service

```rust
// Service: typing
// Variable: typing_users = ["Alice", "Charlie"]
// 
// Expires after 5 seconds (client must re-broadcast if still typing)
```

**Why separate from messages:**

```
Option A: Typing as message type (BAD)
───────────────────────────────────────
messages = [
    Message { type: "text", user: "Alice", text: "Hello" },
    Message { type: "typing", user: "Bob" },
    Message { type: "text", user: "Bob", text: "Hi!" },
]

Problem:
- Typing events clutter message history
- Hard to filter "real messages" vs "meta events"
- "Typing" event stays in history forever

Option B: Typing as separate service (GOOD) ✅
───────────────────────────────────────────────
Service: messages → [text messages only]
Service: typing → [who's currently typing]

Benefits:
- Message retention policy doesn't affect typing
- Typing data can be ephemeral (expire after 5s)
- UI can render differently ("typing indicator" vs "message")
- No "noise" in message history
```

## Performance Optimization Techniques

### Technique 1: Subscription Filtering

**Problem:** Room with 1M messages. New client joins, gets all 1M?

**Solution: Smart subscription**

```rust
// Don't do this (loads entire history):
client.subscribe("room_lobby", "messages", ["message_list"]).await?;

// Do this (subscribe to new messages only):
let history = client.read_variable(
    "room_lobby", 
    "messages", 
    "message_list"
).await?;

let last_50 = &history[history.len().saturating_sub(50)..];
display_messages(&last_50);

// Then subscribe to NEW messages only
client.subscribe("room_lobby", "messages", ["message_list"]).await?;
// Only changes AFTER this point are notified
```

### Technique 2: Presence Heartbeat

**Problem:** If client crashes, presence stays "online" forever

**Solution: Heartbeat with timeout**

```rust
// Client sends periodic heartbeat
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        client.heartbeat().await?;
        update_last_active(user_id).await?;
    }
});

// Server detects idle (no heartbeat for 30s)
if user.last_active < now - 30s {
    remove_from_active_users(user).await?;
}
```

### Technique 3: Batch Message Processing

**Problem:** 100 users send message at same time = 100 Commy writes

**Solution: Batch into single write**

```rust
// Collect messages for 100ms
let mut pending_messages = Vec::new();

for _ in 0..100ms {
    if let Some(msg) = receive_message().await {
        pending_messages.push(msg);
    }
}

// Write all at once
if !pending_messages.is_empty() {
    let current_list = client.read_variable(...).await?;
    let mut new_list = current_list;
    new_list.extend(pending_messages);
    
    client.write_variable(..., new_list).await?;
    // Single notification for all messages!
}
```

### Technique 4: Capacity Planning

**Problem:** Message array grows unbounded

**Solution: Retention policy**

```rust
const MAX_MESSAGE_HISTORY: usize = 100_000;

async fn add_message(msg: Message) {
    let mut messages = get_messages().await?;
    
    messages.push(msg);
    
    // Keep only recent messages
    if messages.len() > MAX_MESSAGE_HISTORY {
        messages = messages[messages.len() - MAX_MESSAGE_HISTORY..].to_vec();
        // Oldest 100K deleted, latest 100K kept
    }
    
    client.write_variable(..., messages).await?;
}
```

## Multi-Server Deployment

### Single Server (Development)

```
┌──────────────────────────────────────────┐
│ Machine 1: Commy Server + Chat Server    │
│ - Port 8443: Commy WSS                   │
│ - Port 9000: Chat Server WSS             │
│ - Stores: shared memory files            │
└──────────────────────────────────────────┘
     ↑    ↑    ↑    ↑
  Client  Client  Client  Client
```

### Multi-Server (Production)

```
┌─────────────────────────────────────────────────────────────┐
│ Commy Cluster (3 nodes)                                     │
│ - Node 1: Primary (stores messages)                         │
│ - Node 2: Replica (backup)                                  │
│ - Node 3: Replica (backup)                                  │
└──────────────┬────────────────────────────────────────┬─────┘
               │                                        │
        ┌──────▼──────────┐                    ┌────────▼──────┐
        │ Chat Server 1   │                    │ Chat Server 2 │
        │ (Region: US)    │                    │ (Region: EU)  │
        └──────┬──────────┘                    └────────┬──────┘
               │           Clustering enabled!          │
               │ (any room can connect to any server)   │
               │                                        │
    ┌──────────┴──────────┐            ┌───────────────┴──────┐
    │                     │            │                      │
  Clients            Clients         Clients              Clients
  (US)               (EU)
```

**Commy's multi-server benefits:**

1. **Room locality** - A room can "live" on any server
2. **No client need for failover** - If server 1 dies, clients reconnect to server 2
3. **Shared data** - All servers see same message history (via replication)
4. **Chat servers are stateless** - Can add/remove chat servers without impact

## Security Considerations

### 1. Per-Room Authentication

```rust
// Each room has independent API key
client.authenticate("room_lobby", auth::api_key("lobby_key_456"))?;
client.authenticate("room_gaming", auth::api_key("gaming_key_789"))?;

// Alice can't join room_gaming even if she has room_lobby access
```

### 2. Permission Structure

```
API Key: "lobby_key_456"
├─ ServiceRead: true   (can read messages)
├─ ServiceWrite: true  (can add messages)
└─ ServiceAdmin: false (can't delete room)

API Key: "admin_key_123"
├─ ServiceRead: true
├─ ServiceWrite: true
└─ ServiceAdmin: true  (can manage rooms)
```

### 3. Message Validation

```rust
async fn validate_message(msg: &ChatMessage) -> Result<()> {
    // Size check
    if msg.text.len() > 10000 {
        return Err("Message too long");
    }
    
    // Content check (optional moderation)
    if contains_flagged_content(&msg.text) {
        return Err("Message contains prohibited content");
    }
    
    // Rate limiting
    let recent = count_messages(msg.user, Duration::from_secs(60));
    if recent > 100 {
        return Err("Rate limit exceeded");
    }
    
    Ok(())
}
```

## Comparison: Commy vs. Alternatives

### Alternative 1: HTTP Polling

```
Architecture:
 Client → HTTP request → Server → Database query
 Client ← Response (OK/No Messages) ← Server

Latency: 0-2000ms (average 1000ms if polling every 2s)
Scalability: Server must handle N requests per second per client
Database load: High (every poll = query)
Reliability: Messages can be lost between polls

Commy beats because:
✓ Sub-10ms latency (not seconds)
✓ 0 polling (not 1000s of requests/min)
✓ No database load (shared memory)
✓ Guaranteed delivery (event-driven)
```

### Alternative 2: WebSocket + In-Memory Server

```
Architecture:
 Client ← Event stream ← Server (holds all state in memory)
                          └─ Each client connection = memory

Latency: ~10ms (good!)
Scalability: Limited (server memory is bottleneck)
Reliability: Messages lost if server restarts
Multi-server: Hard (need complex message routing)

Commy beats because:
✓ Same latency (sub 10ms)
✓ Scales better (messages in persistent shared memory)
✓ Reliable (survives server restart)
✓ Multi-server easy (Commy handles replication)
```

### Alternative 3: Redis Pub/Sub

```
Architecture:
 Client → WebSocket → Server ← Redis Pub/Sub
                        ↓
                    Publish to channel

Latency: ~50ms (pretty good)
Scalability: Good (Redis is scalable)
Reliability: Messages can be lost (not persisted)
Multi-server: Good (Redis is central)

Commy beats in:
✓ Latency (single digit ms vs 50ms)
✓ Reliability (messages persistent, not ephemeral)
✓ Multi-tenant (Redis channels are flat)
✓ Zero-copy (direct memory mapping)
```

### Alternative 4: Apache Kafka

```
Architecture:
 Client → Server → Kafka Topic → All subscribed clients

Latency: ~100-500ms (slower)
Scalability: Excellent (industrial scale)
Reliability: Perfect (messages persisted, replicated)
Multi-server: Excellent (distributed consensus)

Commy is better for:
✓ Chat (lower latency needed)
✓ Small scale (Kafka is overkill for <10k users)
✓ Simple setup (no cluster overhead)
```

## Summary: Why This Design Works

| Aspect | Why | How Commy Enables |
|--------|-----|-------------------|
| Low latency | < 10ms delivery | Event-driven notifications, no polling |
| Scales easily | Support 1000s users per room | Shared memory doesn't bottleneck on server |
| Simple code | No complex caching logic | Commy is the cache (shared memory) |
| Reliable | Messages never lost | Persistent memory-mapped files |
| Multi-tenant | Rooms are isolated | Native Commy tenant feature |
| Extensible | Easy to add features | Services are independent |

This design pattern applies to many systems:
- Stock ticker (prices instead of messages)
- IoT sensors (sensor readings instead of chat)
- Collaborative editing (document instead of messages)
- Gaming (player state instead of messages)
- Alerts/notifications (notifications instead of messages)

The pattern is: **Shared data structure + Event notifications + Multi-tenant isolation = Real-time system**

---

**Next:** Check out the Ticker example to see this pattern applied to financial data!
