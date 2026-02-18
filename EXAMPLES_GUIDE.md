# Commy Examples Guide

**Complete catalog of working examples demonstrating Commy's capabilities.**

---

## 📍 Quick Navigation

| Example | Language | Demonstrates | Location |
|---------|----------|---------------|----------|
| Basic Client | Rust SDK | CRUD operations, lifecycle | `ClientSDKs/rust-sdk/examples/basic_client.rs` |
| Hybrid Client | Rust SDK | Local + remote access patterns | `ClientSDKs/rust-sdk/examples/hybrid_client.rs` |
| Permissions | Rust SDK | Permission-aware CRUD | `ClientSDKs/rust-sdk/examples/permissions_example.rs` |
| C FFI Basic | C | Using Commy from C programs | `ClientSDKs/c-ffi/examples/basic_client.c` |
| Stress Test | Rust Core | Multi-process allocation stress | `examples/stress_intensive.rs` |
| Minimal Allocator | Rust Core | Memory allocation fundamentals | `examples/minimal_alloc_test.rs` |
| Multi-Process | Rust Core | Parallel process coordination | `examples/multiprocess_stress.rs` |
| Weather System | JavaScript | Real-time data distribution | `PRACTICAL_TUTORIAL.md` (guide only) |

---

## 🦀 Rust SDK Examples

### 1. Basic Client (`ClientSDKs/rust-sdk/examples/basic_client.rs`)

**What it shows:**
- Client creation
- WebSocket connection
- Authentication to tenant
- Service creation (explicit CRUD)
- Service reading (read-only)
- Service deletion
- Heartbeat mechanism
- Graceful disconnection

**Demo Output:**
```
Client ID: client-uuid-123
Connecting to server...
Connected!
Authenticating to tenant...
Authenticated! Permissions: [ServiceRead, ServiceWrite, ServiceAdmin]

Creating service (explicit operation)...
✓ Service created with ID: service-id-456

Reading service info (read-only)...
✓ Service obtained! ID: service-id-456

Sending heartbeat...
✓ Heartbeat sent!

Deleting service (explicit operation)...
✓ Service deleted!

Disconnecting...
Disconnected!
```

**Key Learning:**
- Explicit CRUD pattern (no implicit side effects)
- Permission separation (create ≠ read ≠ delete)
- Proper connection lifecycle

**Run it:**
```bash
cd ClientSDKs/rust-sdk
cargo run --example basic_client
```

---

### 2. Hybrid Client (`ClientSDKs/rust-sdk/examples/hybrid_client.rs`)

**What it shows:**
- Virtual service files abstraction
- Variable registration and metadata
- File watching for changes
- SIMD-based change detection
- Zero-copy local access
- Automatic remote fallback
- Unified API for local/remote

**Demo Flow:**
```
Commy Hybrid Client Example
===========================

Initializing Commy client...
✓ Client initialized!

Client ID: client-uuid-789

Step 1: Getting virtual service file...
✓ Got virtual service file for: config

Step 2: Registering variables...
✓ Variable 'counter' registered (offset: 0, size: 8)
✓ Variable 'status' registered (offset: 8, size: 32)

Step 3: Writing variables...
✓ Variable 'counter' updated to 42
✓ Variable 'status' updated to processing

Step 4: File watching (detecting changes)...
Watching for changes...
[Change detected via SIMD comparison]
✓ Variable changed detected in counter
```

**Key Learning:**
- Virtual files work transparently for local and remote
- Change detection using SIMD acceleration
- No code changes needed to switch between local/remote

**Run it:**
```bash
cd ClientSDKs/rust-sdk
cargo run --example hybrid_client
```

---

### 3. Permissions Example (`ClientSDKs/rust-sdk/examples/permissions_example.rs`)

**What it shows:**
- Admin client with full permissions
- Read-only client behavior
- Write-only client behavior
- Permission enforcement
- Multi-client coordination
- Error handling for denied operations

**Demo Scenarios:**

```
SCENARIO 1: Admin Client (has create, read, delete permissions)
──────────────────────────────────────────────────────────────
✓ Admin authenticated
  - Can create services
  - Can read services
  - Can delete services

✓ Admin created service: service-id-abc


SCENARIO 2: Read-Only Client (has only read permission)
──────────────────────────────────────────────────────────────
✓ Reader authenticated
  - Can read services only

✓ Reader obtained service: service-id-abc
✗ Reader cannot create (permission denied)
✗ Reader cannot delete (permission denied)


SCENARIO 3: Write-Only Client (has write permission)
──────────────────────────────────────────────────────────────
✓ Writer authenticated
  - Can write variables
  - Can read their own writes

✓ Writer updated variables
✓ Writer read back their values
```

**Key Learning:**
- Fine-grained permission control
- Different clients can have different CRUD permissions
- Permissions per-tenant (not global)

**Run it:**
```bash
cd ClientSDKs/rust-sdk
cargo run --example permissions_example
```

---

## 🔗 C FFI Examples

### 4. C Basic Client (`ClientSDKs/c-ffi/examples/basic_client.c`)

**What it shows:**
- Creating clients in C
- FFI function calls
- Connection management
- Authentication from C
- Service operations
- Variable read/write
- Memory management (malloc/free)
- Error handling

**Demo Output:**
```
Commy C FFI Example
===================

Creating client...
Client ID: client-uuid-c11
Server URL: wss://localhost:9000

Connecting to server...
Connected!

Connection state: 3
Is connected: yes

Authenticating with API key...
Authenticated!

Authenticated to my_tenant: yes
Authenticated to other_tenant: no

Sending heartbeat...
Heartbeat sent!
Idle seconds: 2

Getting service...
Service ID: service-id-def
Service name: config
Service tenant: my_tenant

Reading variable...
Variable data retrieved
```

**Key Learning:**
- FFI enables C/C++ programs to use Commy
- Type-safe bindings from Rust to C
- All async operations work from C

**Compile & Run:**
```bash
cd ClientSDKs/c-ffi
cargo build --example basic_client
./target/debug/examples/basic_client
```

---

## 🚀 Rust Core Examples

### 5. Stress Intensity Test (`examples/stress_intensive.rs`)

**What it shows:**
- Parent-child process spawning
- Concurrent memory allocation
- Multi-process file access
- Performance under load
- Memory-mapped file sharing
- Process synchronization

**Test Parameters:**
- 8 child processes
- 200 allocations per process
- 50MB test file
- Real concurrent stress

**Demo Output:**
```
Creating 50MB test file...
Spawning 8 processes with 200 allocations each...
Starting stress test...
[Child 0] Running 200 allocations...
[Child 1] Running 200 allocations...
[Child 2] Running 200 allocations...
...
All children completed in 1.234s
Total allocations: 1600
Success rate: 100%
```

**Key Learning:**
- How Commy handles multi-process access
- Memory safety across processes
- FreeListAllocator efficiency at scale

**Run it:**
```bash
cd c:\Users\cires\OneDrive\Documents\projects\commy
cargo run --example stress_intensive --release
```

---

### 6. Minimal Allocator Test (`examples/minimal_alloc_test.rs`)

**What it shows:**
- FreeListAllocator initialization
- Single allocation lifecycle
- Deallocation patterns
- Layout calculations
- Performance metrics
- Memory overhead

**Operations Demonstrated:**
```
1. Create 1MB test file
2. Open and memory-map file
3. Initialize FreeListAllocator
4. Allocate 4 bytes (i32)
5. Read allocated memory
6. Write to allocated memory
7. Deallocate
8. Check free space
```

**Demo Output:**
```
Creating file...
File created in 125.3µs

File opened in 45.2µs

Mmap created in 32.1µs

Allocator initialized in 8.9µs

Single allocation in 35.3µs, result: OK

Read value: 0

Wrote value: 42

Read back: 42

Deallocation in 15.2µs

Free space after: 1048304 bytes
```

**Key Learning:**
- Allocator performance baseline
- Memory efficiency
- Zero-copy access patterns

**Run it:**
```bash
cd c:\Users\cires\OneDrive\Documents\projects\commy
cargo run --example minimal_alloc_test
```

---

### 7. Multi-Process Stress (`examples/multiprocess_stress.rs`)

**What it shows:**
- Multiple simultaneous processes
- Shared memory coordination
- Change detection under load
- Concurrent writes from many sources
- Watcher notifications
- Memory file persistence

**Performance Metrics:**
- Throughput: operations/second
- Latency: time to propagate changes
- Memory usage: per-process overhead
- Allocator efficiency: fragmentation patterns

**Run it:**
```bash
cd c:\Users\cires\OneDrive\Documents\projects\commy
cargo run --example multiprocess_stress --release
```

---

## 📊 JavaScript Examples (Weather System)

### 8. Weather Monitoring System (`PRACTICAL_TUTORIAL.md`)

This is a complete, real-world application demonstrating Commy capabilities.

**Three Integrated Programs:**

#### A. Weather Sensor (Data Producer)
```javascript
// Simulates temperature sensor reading
// Publishes updates every 3 seconds
// Real-time data distribution

setInterval(async () => {
    const temp = baseTemp + (Math.random() - 0.5) * 2;
    
    await client.setVariable('weather', 'sensors', 'temperature', temp);
    console.log(`📡 Temperature: ${temp}°C`);
}, 3000);
```

**Demonstrates:**
- Real-time data publication
- Periodic updates
- WebSocket communication
- SetVariable operations

#### B. Dashboard (Data Consumer)
```javascript
// Displays live temperature
// Refreshes every 1 second
// Subscribes to changes

setInterval(async () => {
    const temp = await client.getVariable('weather', 'sensors', 'temperature');
    console.log(`📊 Current Temperature: ${temp}°C`);
}, 1000);

client.subscribe('weather', 'sensors', ['temperature']);
client.on('VariableChanged', (vars) => {
    console.log('🔔 Temperature updated!');
});
```

**Demonstrates:**
- Data consumption
- Event subscriptions
- Change notifications
- Real-time updates

#### C. Alert System (Conditional Actions)
```javascript
// Monitors for thresholds
// Sends alerts when exceeded
// Maintains alert state

client.on('VariableChanged', (vars) => {
    if (vars.temperature > 30) {
        console.log('🚨 ALERT: High temperature!');
        client.setVariable('weather', 'alerts', 'high_temp_active', 'true');
    }
});
```

**Demonstrates:**
- Threshold-based actions
- Multi-service coordination
- State management
- Alert patterns

**Complete System:**
```
Sensor Publishes              Dashboard Displays           Alert System Monitors
    ↓                             ↓                            ↓
   30.2°C                       30.2°C                    [Checks threshold]
     ↓                             ↓                            ↓
  [Commy]                       Alerts! High temp!
     ↑                             ↑                            ↑
  All changes broadcast to all clients simultaneously
```

**Run the weather example:**

1. Start Commy server (see QUICK_REFERENCE.md)
2. Terminal 1: Run weather sensor
3. Terminal 2: Run dashboard
4. Terminal 3: Run alert system
5. Watch real-time data flow!

---

## 📈 Capabilities Demonstrated

### By Concept

#### Real-Time Data Sharing
- Weather sensor → Dashboard
- Immediate notifications
- Multiple subscribers

#### Multi-Client Coordination
- Sensor producing data
- Dashboard consuming
- Alerts reacting
- All synchronized

#### Permission Management
- Admin with full access
- Read-only clients
- Write-only clients
- Per-tenant isolation

#### Memory Efficiency
- Local zero-copy access
- Shared memory files
- Minimal allocations
- SIMD change detection

#### Process Isolation
- Multiple processes
- Shared file access
- Independent memory spaces
- No process crashes affecting others

#### Language Interoperability
- Rust clients
- C clients
- JavaScript clients
- JavaScript SDK (from PRACTICAL_TUTORIAL)

#### Performance
- Allocation: 35.3 microseconds
- Throughput: 6,922 ops/sec
- Stress: 1600+ concurrent allocations
- Multi-process: 8 simultaneous writers

---

## 🏗️ Architecture Patterns Shown

### Pattern 1: Producer-Consumer
**Shown in:** Weather Sensor + Dashboard

```
Sensor (Producer)
    ↓
  [Commy] 
    ↑
Dashboard (Consumer)
```

### Pattern 2: Broadcast
**Shown in:** All clients receive notifications

```
Program A writes
    ↓
  [Commy broadcasts]
    ↓
Programs B, C, D notified simultaneously
```

### Pattern 3: Permission-Based Access
**Shown in:** Permissions Example

```
Admin    → [Read, Write, Create, Delete]
Reader   → [Read only]
Writer   → [Write only]
```

### Pattern 4: Hierarchical Organization
**Shown in:** Weather example with Services

```
Tenant: weather
├─ Service: sensors (data)
├─ Service: alerts (state)
└─ Service: config (settings)
```

---

## 🔍 Examples by Feature

### If you want to see...

**Memory Allocation:** 
- `examples/minimal_alloc_test.rs` (simple)
- `examples/stress_intensive.rs` (advanced)

**Client Connectivity:**
- `ClientSDKs/rust-sdk/examples/basic_client.rs`
- `ClientSDKs/c-ffi/examples/basic_client.c`

**Permission System:**
- `ClientSDKs/rust-sdk/examples/permissions_example.rs`

**Multi-Process Access:**
- `examples/stress_intensive.rs`
- `examples/multiprocess_stress.rs`

**Real-Time Data:**
- `PRACTICAL_TUTORIAL.md` (weather system)

**Hybrid Local/Remote:**
- `ClientSDKs/rust-sdk/examples/hybrid_client.rs`

**Zero-Copy Optimization:**
- `ClientSDKs/rust-sdk/examples/hybrid_client.rs`

**Change Detection:**
- `ClientSDKs/rust-sdk/examples/hybrid_client.rs`
- `PRACTICAL_TUTORIAL.md` (alerts)

---

## 🚀 Running Examples Locally

### Prerequisites

```bash
# Ensure you're in the Commy root directory
cd c:\Users\cires\OneDrive\Documents\projects\commy

# Ensure Rust nightly is installed
rustup update nightly
rustup default nightly
```

### Rust Examples

```bash
# Build all examples
cargo build --examples

# Run a specific example
cargo run --example stress_intensive
cargo run --example minimal_alloc_test
cargo run --example multiprocess_stress

# Run with release optimizations (faster)
cargo run --example stress_intensive --release
```

### SDK Examples

```bash
# Rust SDK examples
cd ClientSDKs/rust-sdk
cargo run --example basic_client
cargo run --example hybrid_client
cargo run --example permissions_example

# C FFI example
cd ClientSDKs/c-ffi
cargo build --example basic_client
./target/debug/examples/basic_client  # Windows: basic_client.exe
```

### JavaScript Example (Weather)

See [PRACTICAL_TUTORIAL.md](PRACTICAL_TUTORIAL.md) for complete step-by-step instructions.

```bash
# You'll need Node.js installed
# Then follow the weather sensor/dashboard/alerts setup
```

---

## 📝 Example Categories

### Beginner (Start here)
1. ✅ Basic Client (Rust SDK)
2. ✅ Weather System (JavaScript)

### Intermediate
3. ✅ Permissions Example (Rust SDK)
4. ✅ C FFI Example

### Advanced
5. ✅ Hybrid Client (Rust SDK)
6. ✅ Stress Intensity (Rust Core)
7. ✅ Multi-Process Stress (Rust Core)

### Deep Dive
8. ✅ Minimal Allocator (Rust Core)

---

## 🎓 Learning Path

1. **Understand the Concept:**
   - Read BEGINNERS_GUIDE.md

2. **See Simple Client Code:**
   - Read `ClientSDKs/rust-sdk/examples/basic_client.rs`
   - Run it: `cargo run --example basic_client`

3. **Build Something Real:**
   - Follow PRACTICAL_TUTORIAL.md (weather system)
   - Build your own JavaScript client

4. **Learn Advanced Patterns:**
   - Read `examples/hybrid_client.rs`
   - Run stress tests to see performance

5. **Deep Technical Understanding:**
   - Read source code in `src/`
   - Study allocator examples

---

## 💡 Pro Tips

### Tip 1: Examples are self-documented
Every example has comments explaining what it does. Read them!

### Tip 2: Run examples with --release for best performance
```bash
cargo run --example stress_intensive --release
# Much faster than debug!
```

### Tip 3: Modify examples to learn
Try changing values, adding variables, testing edge cases.

### Tip 4: Examples show best practices
The code style, error handling, and patterns are recommended for your code.

### Tip 5: Examples test real scenarios
These aren't toy code - they show realistic use cases.

---

## ❓ FAQ

**Q: Can I use these examples as a template for my project?**

**A:** Absolutely! The code is real, tested, and follows best practices. Copy and modify as needed.

**Q: Do examples work on Windows/Mac/Linux?**

**A:** Yes! Examples are cross-platform (Cargo handles that).

**Q: What if an example fails?**

**A:** Check:
1. Is Commy server running?
2. Are you in the right directory?
3. Is Rust nightly installed?
4. Check PRACTICAL_TUTORIAL.md troubleshooting section

**Q: Can I run multiple examples simultaneously?**

**A:** Yes! Each is independent. Try running stress tests side-by-side.

**Q: Should examples have unit tests?**

**A:** No, examples are self-contained. Tests are in `src/`

**Q: Where do I find SDK examples?**

**A:** `ClientSDKs/rust-sdk/examples/` and `ClientSDKs/c-ffi/examples/`

**Q: How do example permissions work?**

**A:** See `permissions_example.rs` - shows admin, read-only, and write-only clients

**Q: Do examples cover clustering?**

**A:** Yes, clustering setup in `examples/cluster-3-node-local.yaml` and `cluster-5-node-prod.yaml`

---

## 🔗 Related Documentation

| Document | Shows | For |
|----------|-------|-----|
| PRACTICAL_TUTORIAL.md | Working weather system | Learning by doing |
| QUICK_REFERENCE.md | Message formats | Reference |
| BEGINNERS_GUIDE.md | Concepts | Understanding Commy |
| RUST_BASICS.md | Language features | Learning Rust |
| ARCHITECTURE.md | Design details | Deep understanding |

---

## ✅ Examples Checklist

Use this to track your learning:

- [ ] Read BEGINNERS_GUIDE.md
- [ ] Run basic_client.rs example
- [ ] Read Rust permissions example
- [ ] Try C FFI example
- [ ] Read weather system tutorial
- [ ] Run weather system (sensor + dashboard + alerts)
- [ ] Run stress tests
- [ ] Modify an example for your use case
- [ ] Build your own Commy application
- [ ] Read ARCHITECTURE.md

You're done when all boxes are checked! 🎉

---

**Next Steps:**

1. Pick an example that interests you
2. Read the code and comments
3. Run it and see it work
4. Modify it and experiment
5. Build something similar for your use case

Happy learning! 🚀
