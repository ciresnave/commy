# Rust Basics for Commy Users

**Don't know Rust?** This guide explains just enough Rust to understand Commy code.

---

## Part 1: Rust Basics (What You Need to Know)

### What is Rust?

Rust is a programming language that's:
- **Fast** - As fast as C/C++
- **Safe** - Prevents crashes and bugs
- **Modern** - Great for systems programming

**Commy is written in Rust**, so this guide explains the Rust you'll see.

---

## Part 2: Key Rust Concepts

### Variables (Storing Data)

**In Rust:**
```rust
// Create a variable
let name = "Alice";
let age = 30;
let price = 99.99;

// Variables are immutable by default (can't change)
let x = 5;
x = 10;  // ❌ ERROR: can't change x

// Make it mutable if you want to change it
let mut counter = 0;
counter = 1;  // ✅ OK
```

**In Plain English:**
- `let` = Create a variable
- `mut` = Allow changing (mutable)
- Default: Variables don't change (immutable) - Safer!

### Functions (Doing Tasks)

**In Rust:**
```rust
// Define a function
fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

// Call it
let result = add_numbers(5, 3);  // result = 8
```

**Breaking it down:**
- `fn` = Function keyword
- `(a: i32, b: i32)` = Parameters (what we pass in)
- `-> i32` = Return type (what we get back)
- `a + b` = The result (no semicolon means "return this")

### Strings (Text)

**In Rust:**
```rust
// String literal (fixed text)
let greeting = "Hello";

// String that can grow
let mut message = String::new();
message.push_str("Hello ");
message.push_str("World");
// message = "Hello World"
```

**Common string operations:**
```rust
let text = "Alice";
text.len();           // Length: 5
text.to_uppercase();  // "ALICE"
text.contains("lice"); // true
```

### Collections (Groups of Data)

#### Vector (List of items)
```rust
// Create an empty list
let mut numbers: Vec<i32> = Vec::new();

// Add items
numbers.push(10);
numbers.push(20);
numbers.push(30);

// Access items
let first = numbers[0];  // 10

// Loop through items
for num in &numbers {
    println!("{}", num);  // Prints: 10, 20, 30
}
```

#### HashMap (Dictionary / Key-Value)
```rust
use std::collections::HashMap;

let mut prices = HashMap::new();
prices.insert("AAPL", 150.45);
prices.insert("GOOGL", 143.20);

let apple_price = prices.get("AAPL");  // 150.45
```

### Error Handling (What if something goes wrong?)

**In Rust:**
```rust
// Option: Something might exist or not
fn find_user(id: i32) -> Option<String> {
    if id == 1 {
        Some("Alice".to_string())
    } else {
        None
    }
}

// Using it:
match find_user(1) {
    Some(name) => println!("Found: {}", name),
    None => println!("Not found"),
}
```

**Or simpler:**
```rust
if let Some(name) = find_user(1) {
    println!("Found: {}", name);
}
```

---

## Part 3: Understanding Commy Code

### Example: Authenticating a Client

**Commy's Rust code:**
```rust
async fn handle_authenticate(client: &mut Client, payload: Payload) -> Result<(), Error> {
    // Extract credentials from payload
    let tenant_name = payload.get("tenant_name")?;
    let credentials = payload.get("credentials")?;
    
    // Call authentication framework
    let auth_result = auth_framework.authenticate(credentials).await?;
    
    // Create session for this client
    client.session = Some(Session {
        user_id: auth_result.user_id,
        permissions: auth_result.permissions,
        token: auth_result.token,
    });
    
    // Return success
    Ok(())
}
```

**Breaking it down:**

```rust
async fn handle_authenticate(...)
         └─ "async" means this might wait for network/disk

Result<(), Error>
└─ Returns either success [Ok()] or error [Err(...)]

let tenant_name = payload.get("tenant_name")?;
                                           └─ "?" means: 
                                              If error, return it immediately
                                              If ok, use the value
```

### Reading a Variable

**Commy Code:**
```rust
async fn get_variables(service: &Service) -> Result<HashMap<String, Vec<u8>>, Error> {
    // Get all variables from the service
    let variables = HashMap::new();
    
    for (name, metadata) in &service.variables {
        // Read the variable value from shared memory
        let value = service.allocator.offset_to_slice(
            metadata.offset,
            metadata.size
        )?;
        
        variables.insert(name.clone(), value.to_vec());
    }
    
    Ok(variables)
}
```

**What it does:**
1. Iterate through each variable in the service
2. Get its location in memory (offset)
3. Read the data from that location
4. Return it to the client

### Sending a Message to a Client

**Commy Code:**
```rust
async fn send_message(ws: &mut WebSocket, msg: Message) -> Result<()> {
    // Serialize the message to JSON
    let json = serde_json::to_string(&msg)?;
    
    // Send over WebSocket
    ws.send(Message::Text(json)).await?
}
```

**In plain terms:**
1. Convert message to JSON format
2. Send it through WebSocket connection
3. `?` = If error happens, return it

---

## Part 4: Key Data Structures

### The Server Struct

**How Commy is organized:**
```rust
pub struct Server {
    // Map of all tenants
    pub tenants: HashMap<String, Tenant>,
}

pub struct Tenant {
    // Map of all services in this tenant
    pub services: HashMap<String, Service>,
    pub auth_context: TenantAuthContext,
}

pub struct Service {
    pub allocator: FreeListAllocator,     // Memory manager
    pub variables: HashMap<String, VariableMetadata>,
    pub watchers: Vec<Watcher>,           // Clients watching for changes
}
```

**Visual structure:**
```
Server
├─ Tenant: "finance"
│  ├─ Service: "prices"
│  │  ├─ Variable: AAPL (offset: 0, size: 8)
│  │  ├─ Variable: GOOGL (offset: 8, size: 8)
│  │  └─ Watchers: [client1, client2]
│  └─ Service: "trades"
│     └─ Variable: LastTrade
└─ Tenant: "sales"
   └─ Service: "leads"
      └─ Variable: LeadCount
```

### The Client Struct

```rust
pub struct ClientSession {
    pub client_id: String,
    pub tenant_id: String,
    pub permissions: PermissionSet,
    pub token: AuthToken,
    pub subscriptions: Vec<subscription>,
}

pub struct PermissionSet {
    pub can_read: bool,
    pub can_write: bool,
    pub is_admin: bool,
}
```

---

## Part 5: Common Rust Patterns You'll See

### The `?` Operator (Error Handling)

```rust
// This:
let value = do_something()?;

// Is shorthand for:
let value = match do_something() {
    Ok(v) => v,
    Err(e) => return Err(e),
};
```

**When to use it:** When you want to return the error if something fails.

### Pattern Matching

```rust
match client.permission {
    Permission::Read => { /* can read */ }
    Permission::Write => { /* can write */ }
    Permission::Admin => { /* can do anything */ }
}
```

**When to use it:** When you need different logic for different values.

### Borrowing and Ownership

```rust
// Ownership: Takes control
fn process(data: String) { /*...*/  }
let s = String::from("hello");
process(s);
// s is gone now!

// Borrowing: Just temporarily use
fn process(data: &String) { /*...*/  }
let s = String::from("hello");
process(&s);
// s still exists!

// Mutable borrowing: Temporarily change
fn change(data: &mut String) { 
    data.push_str(" world");
}
let mut s = String::from("hello");
change(&mut s);
// s is now "hello world"
```

**Key Rule:** Only one thing can change data at a time (prevents bugs!).

### Async/Await (Waiting for things)

```rust
// This function might wait for network/disk
async fn fetch_price(symbol: &str) -> f64 {
    // Wait for network request
    let response = http_client.get(url).await;
    
    // Parse response
    response.price
}

// To call it, use await:
let price = fetch_price("AAPL").await;

// Or do multiple things in parallel:
let (aapl, googl) = tokio::join!(
    fetch_price("AAPL"),
    fetch_price("GOOGL")
);
```

**When to use it:** Operations that might take time (network, disk, database).

---

## Part 6: Understanding Commy's Main Loop

### Server Startup

```rust
#[tokio::main]  // Start async runtime
async fn main() -> Result<()> {
    // Load configuration from environment
    let cert_path = env::var("COMMY_TLS_CERT_PATH")?;
    let key_path = env::var("COMMY_TLS_KEY_PATH")?;
    
    // Create server
    let server = Arc::new(RwLock::new(Server::new()));
    
    // Create WebSocket listener
    let mut wss_server = WssServer::new(config, server);
    
    // Initialize TLS
    wss_server.initialize_tls()?;
    
    // Start accepting connections
    wss_server.run().await?;
    
    Ok(())
}
```

**What each line does:**
1. `#[tokio::main]` - Set up async system
2. Load TLS certificates
3. Create empty Server instance
4. Create WebSocket listener
5. Set up encryption
6. Start accepting client connections

### Connection Handler

```rust
async fn handle_connection(ws: WebSocket, server: Arc<RwLock<Server>>) {
    // Get mutable reference to server
    let mut server = server.write().await;
    
    // Client loop: keep reading messages
    loop {
        // Wait for message from client
        if let Some(msg) = ws.next().await {
            // Route message to correct handler
            match msg.msg_type {
                "Heartbeat" => handle_heartbeat(&mut server, msg).await,
                "Authenticate" => handle_authenticate(&mut server, msg).await,
                "GetVariables" => handle_get_variables(&server, msg).await,
                "SetVariables" => handle_set_variables(&mut server, msg).await,
                _ => send_error(&ws, "Unknown message type"),
            }
        }
    }
}
```

**The loop:**
1. Wait for client message
2. Determine message type
3. Call appropriate handler
4. Send response back
5. Repeat

---

## Part 7: Memory Management (How Data is Stored)

### The Allocator

```rust
pub struct FreeListAllocator {
    // Points to start of memory file
    ptr: *const u8,
    
    // How big is the allocation
    size: usize,
    
    // Which portions are free/used
    free_list: Vec<FreeBlock>,
}

impl FreeListAllocator {
    pub fn allocate(&mut self, size: usize) -> Result<*mut u8> {
        // Find free block large enough
        for block in &mut self.free_list {
            if block.size >= size {
                // Found space!
                let ptr = block.start;
                block.size -= size;
                block.start += size;
                return Ok(ptr);
            }
        }
        // No space available
        Err("Allocation failed")
    }
}
```

**How it works:**
```
Memory Layout:
┌──────────────────────────────────────┐
│ Header  │ AAPL  │ GOOGL │ FREE  │    │
├─────────┼───────┼───────┼───────┴────┤
│  16 B   │  8 B  │  8 B  │  48 B      │
└──────────────────────────────────────┘

Allocating MSFT (8 bytes):
┌──────────────────────────────────────┐
│ Header  │ AAPL  │ GOOGL │ MSFT  │ FREE
├─────────┼───────┼───────┼───────┼────┤
│  16 B   │  8 B  │  8 B  │  8 B  │40 B│
└──────────────────────────────────────┘
```

---

## Part 8: Concurrency (Multiple Things at Once)

### Atomic Operations

```rust
// Shares data safely between threads
let counter = Arc::new(Mutex::new(0));

// Task 1: Increment
let c = counter.clone();
let t1 = tokio::spawn(async move {
    let mut val = c.lock().await;
    *val += 1;
});

// Task 2: Increment
let c = counter.clone();
let t2 = tokio::spawn(async move {
    let mut val = c.lock().await;
    *val += 1;
});

// Wait for both
tokio::join!(t1, t2);
```

**Explained:**
- `Arc` = Atomic Reference Count (share ownership)
- `Mutex` = Only one task can access at a time (prevents conflicts)
- `clone()` = Make another reference to same data
- `lock().await` = Wait for turn to access

---

## Part 9: Testing in Rust

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocate() {
        let mut alloc = FreeListAllocator::new(1024);
        
        // Allocate memory
        let ptr = alloc.allocate(100).unwrap();
        
        // Verify it worked
        assert!(ptr.is_valid());
        
        // Deallocate
        alloc.deallocate(ptr, 100);
    }
}
```

**Run tests:**
```bash
cargo test
```

### Assertion Helpers

```rust
assert_eq!(value, 42);           // Value must equal 42
assert!(value > 0);              // Value must be true
assert_ne!(a, b);                // Must not be equal
```

---

## Part 10: Common Rust Gotchas

### Move vs Copy

```rust
// String: Moved (no longer accessible)
let s1 = String::from("hello");
let s2 = s1;  // s1 is moved, can't use it
println!("{}", s1);  // ❌ ERROR

// Integer: Copied (still accessible)
let n1 = 42;
let n2 = n1;  // n1 is copied, still works
println!("{}", n1);  // ✅ OK
```

**Rule of thumb:** Expensive things get moved, cheap things get copied.

### Lifetimes

```rust
// Rust needs to know: How long is this valid?

fn get_name(person: &Person) -> &String {
    &person.name
    //^ This reference is valid as long as person is valid
}

// Explicit lifetime:
fn get_name<'a>(person: &'a Person) -> &'a String {
    &person.name
}
```

**When you see `'a`:** It's a "lifetime label" - just ignore for now.

---

## Part 11: Reading Error Messages

### Example Error

```
error[E0382]: borrow of moved value: `s`
   --> src/main.rs:3:20
    |
2   |     let s2 = s1;
    |              -- value moved here
3   |     println!("{}", s1);
    |                    ^^ value borrowed after move

error: aborting due to 1 previous error
```

**Translation:**
- "borrow of moved value" = You moved the variable
- Line 2: Where you moved it
- Line 3: Where you tried to use it
- Solution: Use `&s1` to borrow instead

---

## Part 12: Useful Commands

### Running Commy

```bash
# Build debug version
cargo build

# Build release version (optimized)
cargo build --release

# Run tests
cargo test

# Run with output
cargo test -- --nocapture

# Check code without building
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

### Viewing Documentation

```bash
# Generate and open docs
cargo doc --open

# Look at specific module
cargo doc --open commy::server
```

---

## Part 13: Rust vs Other Languages

### If you know JavaScript:

```javascript
// JS
const user = { name: "Alice", age: 30 };
user.name = "Bob";     // OK, can change

// Rust
let user = User { name: "Alice".into(), age: 30 };
user.name = "Bob";     // ❌ ERROR

let mut user = User { name: "Alice".into(), age: 30 };
user.name = "Bob";     // ✅ OK
```

Key difference: Rust immutable by default.

### If you know Python:

```python
# Python: Duck typing
def process(data):
    return data.length   # Works if length exists

# Rust: Type checking
fn process<T: HasLength>(data: T) -> usize {
    data.length()        // Only works if HasLength is implemented
}
```

Key difference: Rust checks at compile time, Python at runtime.

### If you know Java:

```java
// Java: Garbage collection
List<String> items = new ArrayList<>();
items.add("hello");    // Auto-managed memory

// Rust: Ownership
let mut items = Vec::new();
items.push("hello");   // Manual but automatic rules
```

Key difference: Rust no garbage collection (faster).

---

## Part 14: Glossary of Rust Terms

| Term | Meaning |
|------|---------|
| `fn` | Function definition |
| `let` | Create variable |
| `mut` | Mutable (can change) |
| `&` | Borrow (temporary access) |
| `&mut` | Mutable borrow |
| `async` | Function that waits for things |
| `await` | Wait for async operation |
| `Result` | Either Ok(value) or Err(error) |
| `Option` | Either Some(value) or None |
| `String` | Text that can grow |
| `&str` | Text reference (fixed) |
| `Vec` | Dynamic list/array |
| `HashMap` | Key-value dictionary |
| `Arc` | Atomic Reference Count |
| `Mutex` | Mutual exclusion (one at a time) |
| `RwLock` | Read-Write lock |

---

## Conclusion

You don't need to be a Rust expert to understand Commy! Key concepts:

1. ✅ Variables and types
2. ✅ Functions
3. ✅ Collections (Vec, HashMap)
4. ✅ Error handling (Result, Option)
5. ✅ Async/await for operations that wait
6. ✅ Borrowing (&) vs moving
7. ✅ The `?` operator for error propagation

**Remember:** Rust is designed to be safe. If it doesn't compile, it's usually saving you from a bug!

---

**Next: Read BEGINNERS_GUIDE.md for Commy concepts** ✅
