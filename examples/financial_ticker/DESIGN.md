# Financial Ticker Design Guide

**Why we built it this way and how Commy enables ultra-low latency.**

## Design Philosophy

The financial ticker is optimized for three critical requirements:

1. **Ultra-low latency** - <5ms from price change to trader's screen
2. **High throughput** - 1000+ price updates per second
3. **Consistency** - All traders see same prices at same time

Every design decision is made to maximize these three properties.

## 🏗️ Architecture Decisions

### Decision 1: Commy as Real-Time Market Data Store

**What we chose:** Store all prices directly in Commy shared memory

**Why NOT traditional approaches:**

```
Approach A: Database polling (Traditional Finance)
──────────────────────────────────────────────────
Data Source → Database Write (5-10ms) → Storage disk
Trader A     ← Polls database (query + network = 20ms)
Trader B     ← Polls database (old data if just written!)
Trader C     ← Polls database (missing updates)

Cost: Database can't keep up with 1000+ prices/sec
Latency: 25-100ms for each trader
Consistency: Each trader sees different prices!

Approach B: Commy shared memory ✅
─────────────────────────────────────────────────
Data Source → Writes to Commy memory (0.5ms)
Trader A     ← Notified instantly (< 1ms) ✓
Trader B     ← Notified instantly (< 1ms) ✓
Trader C     ← Notified instantly (< 1ms) ✓

Cost: Zero database load
Latency: <5ms total
Consistency: All see same price simultaneously!
```

**Key insight:** Commy makes traders "memory-mapped" subscribers. When market data updates, traders' memory instantly reflects it.

### Decision 2: Per-Stock Variables (Not Single Aggregated Object)

**What we chose:** Store each stock price as separate variable

```
Service: stocks
├─ Variable: aapl = {price: 150.45, bid: 150.44, ask: 150.46, ...}
├─ Variable: googl = {price: 143.20, bid: 143.19, ask: 143.21, ...}
├─ Variable: msft = {price: 380.90, bid: 380.89, ask: 380.91, ...}
└─ [100+ more]
```

**Why separate variables (not one big object):**

```
Option A: Single aggregated object (BAD)
─────────────────────────────────────────
Service: stocks
└─ Variable: all_prices = {
    "AAPL": 150.45,
    "GOOGL": 143.20,
    "MSFT": 380.90,
    ... [all 1000 stocks]
}

Problem:
- AAPL price changes → rewrite entire 10MB market snapshot
- Trader watching 10 stocks still gets notified of all 1000
- Change detection overhead (need to parse entire object)

Option B: Per-stock variable ✅
───────────────────────────────
Service: stocks
├─ Variable: aapl = 150.45
├─ Variable: googl = 143.20
├─ Variable: msft = 380.90
└─ [separate variables]

Benefits:
- AAPL changes → only AAPL variable updated
- Subscribers get only their stock updates
- Fine-grained change detection
- Scales perfectly (1000 stocks = 1000 independent updates)
```

**Performance implication:**
- Option A: 1.5 billion bytes/sec of writes (unbearable)
- Option B: 50KB/sec of updates (trivial)

### Decision 3: Hierarchical Data Services

**What we chose:** Organize data by asset class

```
Tenant: financial_market
├─ Service: stocks         ← Individual stock prices
├─ Service: indices        ← Market indices (S&P500, NASDAQ)
├─ Service: options        ← Options data (future)
├─ Service: futures        ← Futures data (future)
├─ Service: alerts         ← Alert statuses
├─ Service: market_state   ← Open/closed/halted
└─ Service: news           ← News items (future)
```

**Why this structure:**

```
Option A: Single service (BAD)
───────────────────────────────
Service: market_data
├─ Variable: AAPL_price
├─ Variable: AAPL_iv
├─ Variable: AAPL_bid
├─ Variable: AAPL_ask
├─ Variable: GOOGL_price
├─ Variable: GOOGL_iv
├─ Variable: MSFT_price
├─ Variable: S&P500_value
├─ Variable: DOW_value
├─ Variable: AAPL_alert_status
└─ [thousands more]

Problem:
- Single service gets HUGE
- Subscription scope is too broad
- Hard to manage permissions (all traders see everything)

Option B: Segmented by asset type ✅
─────────────────────────────────────
Service: stocks
├─ AAPL, GOOGL, MSFT...
Service: indices
├─ S&P500, NASDAQ, DOW...
Service: alerts
├─ All alert statuses

Benefits:
- Traders subscribe to stock data but not index data if unwanted
- Permissions can be per-service (derivatives traders see options)
- Future addition of options/futures is natural
- Scalability per service (independent management)
```

### Decision 4: Timestamp Synchronization

**What we chose:** Every price update includes server timestamp

```rust
#[derive(Serialize, Deserialize)]
pub struct StockPrice {
    pub symbol: String,
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume: u64,
    pub timestamp: i64,  // ← Nanoseconds since epoch
    pub last_update: i64,
}
```

**Why timestamps matter:**

```
Scenario: AAPL updates from 150.45 to 150.47

Without timestamps:
────────────────────
Trader A gets: price = 150.47
Trader B gets: price = 150.47
Trader C gets: old cached price = 150.45

Who's right? Nobody knows! ❌

With timestamps:
────────────────
Trader A gets: price = 150.47, ts = 15000000000 → "new!"
Trader B gets: price = 150.47, ts = 15000000000 → "same"
Trader C gets: price = 150.45, ts = 14999999500 → "old! discard"

Everyone can make correct decisions ✅
```

**Timestamp accuracy requirement:** Nanosecond precision (1 billionth of a second)
- Prevents price/volume mismatches
- Enables ordering of concurrent updates
- Allows replay/reconstruction

### Decision 5: Market State as Separate Service

**What we chose:** Market open/closed/halted status in separate service

```
Service: market_state
├─ Variable: status = "open"         (open/premarket/closed/halted)
├─ Variable: last_update = timestamp
└─ Variable: halt_reason = None      (trading halt explanation)
```

**Why separate:**

```
Option A: Part of each price (BAD)
──────────────────────────────────
Variable: AAPL = {
    price: 150.45,
    market_status: "open",    ← Duplicated in every variable!
    ...
}

Problem: 1000 stocks × redundant market_status = massive waste

Option B: Separate service ✅
───────────────────────────
Service: market_state
└─ Variable: status = "open"

Service: stocks
└─ Variable: AAPL = 150.45   (market status not needed here)

Benefit: Market status changes once per day, not per price tick
```

### Decision 6: Alert System as Derived Data

**What we chose:** Alert state stored separately from prices

```
Service: alerts
├─ Variable: AAPL_above_155 = false      (monitored threshold)
├─ Variable: GOOGL_volume_spike = false  (anomaly detection)
└─ Variable: circuit_breaker_active = false
```

**Why separate from prices:**

The core principle: **Separation of concerns**

```
Prices change 50/second → 50 writes/sec
Alerts change 1/minute  → 1 write/sec

If combined:
- Every price write updates alert service
- 50x overhead for alert system
- Change detection fire spikes on price data
- Hard to query "current alerts" without scanning prices

If separate:
- Price updates are independent
- Alert updates are independent
- Traders can monitor just alerts if desired
- Alert system can focus on state changes
```

### Decision 7: Market Data Source as Independent Producer

**What we chose:** Single authoritative data source

```
┌────────────────────────────────────┐
│ Market Data Source                 │
│ - Updates prices 50x/second        │
│ - Connects to Commy                │
│ - Publishes updates atomically     │
│ - Maintains error handling         │
└────────────────────────────────────┘
         ↓ Single Write Channel
┌────────────────────────────────────┐
│ Commy Shared Memory                │
│ - Stores price = 150.45            │
│ - Notifies all subscribers         │
└────────────────────────────────────┘
         ↓ Multi-Read Channel
    ┌────┴────┬──────┐
    ↓         ↓      ↓
Trader1    Trader2  AlertSystem
```

**Why single source (not distributed):**

```
Option A: Distributed writers (Multiple servers write prices)
────────────────────────────────────────────────────────────
Server A writes AAPL = 150.45
Server B writes AAPL = 150.46  ← Conflict!
Server C writes AAPL = 150.44  ← Conflict!

Which is correct? Race condition!
Traders see inconsistent state!

Option B: Single authoritative source ✅
────────────────────────────────────────
Market Data Source writes AAPL = 150.47
All others read (never write)

Benefits:
- Single source of truth
- No conflicts, no races
- Strong consistency
- Audit trail (one writer = easy to verify)
```

**Note:** Source itself can be redundant (failover), but only one writes at a time.

## Performance Optimization Techniques

### Technique 1: Selective Subscription

**Problem:** Trader only cares about 10 stocks, but gets notified of all 1000

**Solution:**

```rust
// Don't subscribe to all stocks
client.subscribe("financial_market", "stocks", &["*"]).await?;  // ❌ Too much

// Subscribe to specific stocks only
client.subscribe(
    "financial_market",
    "stocks",
    &["aapl", "googl", "msft", "tsla", "nvda"]
).await?;  // ✅ Efficient
```

**Performance impact:**
- No wildcard: 5 notifications/sec if 5 stocks change
- Wildcard: 1000 notifications/sec (all stocks)
- Difference: 200x message reduction!

### Technique 2: Update Batching Window

**Problem:** Price changes constantly, Commy updates constantly

**Solution: Small batching window**

```rust
// Instead of writing every single tick:
client.write_variable("stocks", "aapl", price1).await?;  // 0ms
client.write_variable("stocks", "aapl", price2).await?;  // 50ms
client.write_variable("stocks", "aapl", price3).await?;  // 100ms
// ... 50 times/second = 50 Commy writes/sec

// Use small batching window:
let mut prices_buffer = vec![];

for _ in 0..10ms {  // Collect 10ms of updates
    if let Some(price) = receive_price_from_feed().await {
        prices_buffer.push(price);
    }
}

// Write once per 10ms batch
for (symbol, latest_price) in aggregate_latest(&prices_buffer) {
    client.write_variable(
        "financial_market",
        "stocks",
        &symbol,
        latest_price
    ).await?;
}
```

**Trade-off:**
- Latency impact: +10ms (acceptable for trading at 5ms baseline)
- Throughput improvement: 5x (fewer Commy writes)
- Consistency: Higher (batched updates are atomic)

### Technique 3: Compression for Large Market Snapshots

**Problem:** Initial snapshot has 1000 stocks, traders need all of them

**Solution:**

```rust
// First load: Read entire snapshot (one time)
let snapshot = client.read_variable(
    "financial_market",
    "stocks",
    "snapshot"
).await?;

let prices = decompress(snapshot)?;

// Then subscribe to updates only
client.subscribe(
    "financial_market",
    "stocks",
    &specific_symbols
).await?;
```

**Benefit:** One big read (1MB) + many small updates (100 bytes each)

### Technique 4: Update Rate Limiting Per Trader

**Problem:** Slow network trader gets overwhelmed with 1000 updates/sec

**Solution:**

```rust
// Trader specifies max update rate
client.set_subscription_throttle(
    Duration::from_millis(100)  // Max 10 updates/second
).await?;

// Commy coalesces: if 5 updates in 100ms, sends 1 summary
// Latest price always included, intermediate prices optional
```

## Correctness & Consistency Guarantees

### Guarantee 1: Causal Consistency

```rust
// Update sequence:
T0: AAPL = 150.00
T1: AAPL = 150.05  ← This ALWAYS happens after T0
T2: AAPL = 150.10  ← This ALWAYS happens after T1

Guarantee: Traders never see T2 without seeing T1.
```

**Why Commy provides this:**
- Single source writes in order
- Commy preserves order in shared memory
- Subscribers get updates in order

### Guarantee 2: Atomicity Within Symbol

```rust
// Update to AAPL:
Before: {price: 150.00, bid: 150.01, ask: 150.02, vol: 1000000, ts: T0}
After:  {price: 150.05, bid: 150.04, ask: 150.06, vol: 1100000, ts: T1}

Guarantee: Traders see EITHER old values OR new values
          NEVER mixed (old price with new bid, etc.)
```

**Implementation:**
```rust
// Write entire struct as atomic unit
client.write_variable(
    "financial_market",
    "stocks",
    "aapl",
    StockPrice {
        price: 150.05,
        bid: 150.04,
        ask: 150.06,
        volume: 1100000,
        timestamp: now(),
    }
).await?;

// Commy writes entire struct in one operation
// Subscribers get entire new struct or entire old struct
// Never partial/inconsistent state
```

### Guarantee 3: Exactly-Once Delivery

**Problem:** Network error could duplicate price update

```
Network failure scenario:
Trader A receives: AAPL = 150.05
Network drops and reconnects
Trader A receives: AAPL = 150.05 again (duplicate!)
```

**Solution with timestamps:**

```rust
// Trader-side deduplication
last_processed_timestamp = 0;

while let Some(update) = receive_update() {
    if update.timestamp <= last_processed_timestamp {
        // Skip (we've seen this)
        continue;
    }
    
    process_update(update);
    last_processed_timestamp = update.timestamp;
}

// Duplicate eliminated via timestamp check!
```

## Scalability Analysis

### Throughput Scaling

```
Market Data Source performance:
────────────────────────────────
Price updates/second: 1,000
Bytes per update: 200
Total throughput: 200KB/sec

Commy can handle:
├─ Per-process: 1,000,000+ ops/sec (plenty!)
└─ Network: 10Gbps+ WSS (unlimited for 200KB/sec)

Conclusion: ✅ No bottleneck
```

### Concurrent Trader Scaling

```
100 traders, 10 stocks each, 1000 updates/sec total:

Option A: Database polling (1 query per trader per 2s)
──────────────────────────────────────────────────────
Queries per second: 100 traders / 2s = 50 queries/sec
Database load: Manageable but increasing

Option B: Commy with selective subscription
──────────────────────────────────────────────────────
Updates per trader: 100 updates/sec (10 stocks × 10 updates/sec)
Commy load: 100 traders × 100 updates = 10,000 updates/sec
But all from same memory! No bottleneck!

Conclusion: ✅ Scales to 1000+ traders with no performance degradation
```

### Memory Scaling

```
Memory per stock price: ~500 bytes (50 fields/metadata)
Number of securities: 10,000 (stocks + options + futures)
Total memory: 5MB

This fits entirely in L3 cache on modern CPUs!
Result: Ultra-fast access, no disk I/O needed
```

## Comparison: Commy vs. Market Data Alternatives

### Alternative 1: REST API Polling

**Architecture:**
```
Trader → HTTP GET /api/price/AAPL → Database query → Response
```

**Metrics:**
- Latency: 50-100ms per request
- Throughput: Rate-limited (e.g., 100 req/sec)
- Cost: High (per-request API charges)
- Consistency: Eventual (polling lag)

**Why Commy wins:**
- ✅ 50x lower latency
- ✅ Unlimited throughput
- ✅ No per-request costs
- ✅ Immediate consistency

### Alternative 2: FIX Protocol (Financial Information Exchange)

**Architecture:**
```
Market Data Source → FIX Gateway → TCP/Binary protocol → Trader terminals
```

**Metrics:**
- Latency: <5ms (comparable)
- Throughput: Industrial scale (1M+ messages/sec)
- Cost: High (licensing fees)
- Complexity: Very high (binary protocol, FIX standard)

**Where Commy is better:**
- ✓ Similar latency
- ✓ Open source (no licensing)
- ✓ Simpler implementation (JSON/REST)
- ✓ Easier to build custom clients
- ✗ Less standardized (but Commy is emerging standard)

### Alternative 3: Bloomberg / Reuters Terminals

**Metrics:**
- Latency: <1ms (best-in-class)
- Throughput: Unlimited
- Cost: $2000+/month per trader
- Coverage: Absolute (every security known to Wall Street)

**Commy comparison:**
- ⚖️ Similar latency
- ⚖️ Similar throughput  
- ✅ Free/open source
- ✗ You build your own data feeds

**Use case:** Commy for company-internal market data, Bloomberg for external pricing.

## Production Deployment Considerations

### High Availability

```
Commy Cluster (3 nodes)
├─ Primary: Accepts all writes, replicates
├─ Replica 1: Standby
└─ Replica 2: Standby

If Primary fails:
└─ Replica 1 promoted to Primary (automatic failover)

Downtime: <100ms
Data loss: Zero (replicated)
```

### Monitoring & Alerting

```
Metrics to track:
├─ Update latency (should be <10ms P99)
├─ Updates per second
├─ Subscriber count
├─ Memory usage
├─ Replication lag
└─ Alert trigger rate
```

### Data Retention

```
Real-time prices: Keep in memory indefinitely
  (memory footprint is small: 10K securities × 500B = 5MB)

Historical prices: Checkpoint to disk weekly
  (Archive old snapshots for compliance)

Alerts: Keep recent 7 days
  (Then archive to long-term storage)
```

## Summary: Why This Design Works

| Aspect | Why | How |
|--------|-----|-----|
| Ultra-low latency | Sub-5ms critical for traders | Event-driven from shared memory |
| High throughput | 1000+ updates/sec needed | Commy handles without database |
| Consistency | All traders see same prices | Single source writes, Commy broadcasts |
| Scalability | 1000+ concurrent traders | Per-stock variables enable independent updates |
| Simplicity | No complex state management | Commy manages persistence |
| Reliability | No message loss | Shared memory survives process restart |

This design pattern applies to:
- **Cryptocurrency exchanges** (similar real-time data needs)
- **Sports live scoring** (statistics instead of prices)
- **IoT sensor networks** (measurements instead of prices)
- **Weather data distribution** (forecasts instead of prices)
- **Any high-frequency data system** (same architecture, different domain)

---

**Next:** Check out the Chat example for a different real-time use case!
