# Getting Started with Commy - Practical Tutorial

This guide teaches you how to **actually use Commy** with working examples.

---

## What We'll Build

A simple real-time **weather monitoring system**:
- ☀️ **Weather Sensor** - Reads temperature and updates Commy
- 📊 **Dashboard** - Displays current weather
- ⚠️ **Alert System** - Warns if temperature is too high
- 📱 **Mobile Client** - Views weather from phone

All communicating through Commy!

---

## Step 1: Start the Commy Server

### 1.1: Generate Certificates (One-time setup)

```powershell
# Create cert and key files
# (We did this in testing)
# Files: dev-cert-temp.pem, dev-key-temp.pem
```

### 1.2: Start the Server

```powershell
cd C:\Users\cires\OneDrive\Documents\projects\commy

# Set environment variables
$env:COMMY_TLS_CERT_PATH = "dev-cert-temp.pem"
$env:COMMY_TLS_KEY_PATH = "dev-key-temp.pem"
$env:COMMY_LISTEN_ADDR = "127.0.0.1"
$env:COMMY_LISTEN_PORT = "8443"
$env:COMMY_SERVER_ID = "weather-server"
$env:COMMY_CLUSTER_ENABLED = "false"

# Start server
.\target\debug\commy.exe
```

**Expected output:**
```
[Commy] ════════════════════════════════════════════
[Commy] Initializing Commy Server
[Commy] ════════════════════════════════════════════
[Commy] Server ID: weather-server
[Commy] Client listen address: 127.0.0.1:8443
[Commy] TLS enabled: true
[Commy] Clustering enabled: false
[Commy] ✓ TLS initialized successfully (WSS mode)
[Commy] ✓ Server initialized, starting WSS listener...
[Commy] Remote clients connect to: wss://127.0.0.1:8443
```

Server is now running! ✅

---

## Step 2: Program 1 - Weather Sensor

This program reads temperature and sends it to Commy.

### Create `weather_sensor.js`

```javascript
// weather_sensor.js - Reads temperature and updates Commy

const WebSocket = require('ws');

// Configuration
const COMMY_URL = 'wss://127.0.0.1:8443';
const TENANT = 'weather';
const SERVICE = 'current_conditions';
const API_KEY = 'sensor-key-123';

// Connect to Commy
const ws = new WebSocket(COMMY_URL, {
    rejectUnauthorized: false  // OK for dev, use real certs in prod
});

// When connected
ws.on('open', async () => {
    console.log('✓ Connected to Commy');
    
    // Step 1: Authenticate
    const authRequest = {
        type: 'Authenticate',
        payload: {
            tenant_name: TENANT,
            method: 'api_key',
            credentials: API_KEY
        }
    };
    
    ws.send(JSON.stringify(authRequest));
    console.log('→ Sent authentication request');
});

// Handle responses
ws.on('message', (data) => {
    const msg = JSON.parse(data);
    
    if (msg.type === 'AuthResponse') {
        if (msg.success) {
            console.log('✓ Authenticated successfully');
            startSendingTemperatures();
        } else {
            console.log('✗ Authentication failed');
        }
    }
});

// Simulate reading temperatures
function startSendingTemperatures() {
    let temp = 72; // Start at 72°F
    
    setInterval(() => {
        // Simulate temperature change
        temp += (Math.random() - 0.5) * 2; // ±1°F
        temp = Math.round(temp * 10) / 10; // Round to 1 decimal
        
        // Send temperature to Commy
        const setVarRequest = {
            type: 'SetVariables',
            payload: {
                tenant_name: TENANT,
                service_name: SERVICE,
                variables: {
                    temperature: Math.round(temp).toString(),
                    humidity: Math.round(Math.random() * 100).toString(),
                    timestamp: new Date().toISOString()
                }
            }
        };
        
        ws.send(JSON.stringify(setVarRequest));
        console.log(`→ Updated: Temp=${temp}°F`);
    }, 3000); // Update every 3 seconds
}

// Error handling
ws.on('error', (error) => {
    console.error('✗ Error:', error.message);
});

ws.on('close', () => {
    console.log('✗ Disconnected from Commy');
});
```

### Run the Sensor

```bash
# Install dependency
npm install ws

# Run sensor
node weather_sensor.js
```

**Expected output:**
```
✓ Connected to Commy
→ Sent authentication request
✓ Authenticated successfully
→ Updated: Temp=72°F
→ Updated: Temp=72.8°F
→ Updated: Temp=73.1°F
...
```

✅ Your sensor is now updating Commy every 3 seconds!

---

## Step 3: Program 2 - Dashboard Display

This program reads temperature and displays it.

### Create `weather_dashboard.js`

```javascript
// weather_dashboard.js - Shows live weather

const WebSocket = require('ws');

const COMMY_URL = 'wss://127.0.0.1:8443';
const TENANT = 'weather';
const SERVICE = 'current_conditions';
const API_KEY = 'dashboard-key-456';

const ws = new WebSocket(COMMY_URL, {
    rejectUnauthorized: false
});

let authenticated = false;

ws.on('open', () => {
    console.log('✓ Connected to Commy');
    
    // Authenticate
    const authRequest = {
        type: 'Authenticate',
        payload: {
            tenant_name: TENANT,
            method: 'api_key',
            credentials: API_KEY
        }
    };
    
    ws.send(JSON.stringify(authRequest));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    
    if (msg.type === 'AuthResponse' && msg.success) {
        console.log('✓ Authenticated');
        authenticated = true;
        startReadingWeather();
    }
    
    // Handle variable change notifications
    if (msg.type === 'VariableChanged') {
        displayWeather(msg.payload.variables);
    }
});

function startReadingWeather() {
    console.log('\n📊 WEATHER DASHBOARD\n');
    
    // Request initial data
    const getVarRequest = {
        type: 'GetVariables',
        payload: {
            tenant_name: TENANT,
            service_name: SERVICE
        }
    };
    
    ws.send(JSON.stringify(getVarRequest));
    
    // Update display every second
    setInterval(() => {
        ws.send(JSON.stringify(getVarRequest));
    }, 1000);
}

function displayWeather(variables) {
    console.clear();
    console.log('╔═══════════════════════════════╗');
    console.log('║      WEATHER DASHBOARD        ║');
    console.log('╚═══════════════════════════════╝\n');
    
    console.log(`🌡️  Temperature: ${variables.temperature || '--'}°F`);
    console.log(`💧 Humidity:    ${variables.humidity || '--'}%`);
    console.log(`⏰ Updated:     ${variables.timestamp || '--'}\n`);
    
    // Show status
    const temp = parseFloat(variables.temperature);
    if (temp > 80) {
        console.log('⚠️  HOT - Temperature above 80°F');
    } else if (temp < 60) {
        console.log('❄️  COLD - Temperature below 60°F');
    } else {
        console.log('✅ COMFORTABLE - Temperature normal');
    }
}

ws.on('error', (error) => {
    console.error('✗ Error:', error.message);
});
```

### Run the Dashboard

In a **second terminal**:

```bash
node weather_dashboard.js
```

**Expected output:**
```
✓ Connected to Commy
✓ Authenticated

╔═══════════════════════════════╗
║      WEATHER DASHBOARD        ║
╚═══════════════════════════════╝

🌡️  Temperature: 72.8°F
💧 Humidity:    45%
⏰ Updated:     2026-02-15T07:45:23.123Z

✅ COMFORTABLE - Temperature normal
```

✅ Dashboard is now displaying live temperature!

---

## Step 4: Program 3 - Alert System

Sends alerts when temperature is too high.

### Create `weather_alerts.js`

```javascript
// weather_alerts.js - Sends alerts for extreme temperatures

const WebSocket = require('ws');
const COMMY_URL = 'wss://127.0.0.1:8443';
const TENANT = 'weather';
const SERVICE = 'current_conditions';
const API_KEY = 'alert-key-789';

const TEMP_THRESHOLD_HIGH = 85;  // Alert if above
const TEMP_THRESHOLD_LOW = 55;   // Alert if below

const ws = new WebSocket(COMMY_URL, {
    rejectUnauthorized: false
});

let lastAlertTime = 0;
const ALERT_COOLDOWN = 5000; // 5 second cooldown between alerts

ws.on('open', () => {
    console.log('✓ Alert system connected');
    
    ws.send(JSON.stringify({
        type: 'Authenticate',
        payload: {
            tenant_name: TENANT,
            method: 'api_key',
            credentials: API_KEY
        }
    }));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    
    if (msg.type === 'AuthResponse' && msg.success) {
        console.log('✓ Alert system authenticated\n');
        subscribeToWeather();
    }
    
    // Check for temperature changes
    if (msg.type === 'VariableChanged' && msg.payload.variables) {
        checkTemperature(msg.payload.variables);
    }
});

function subscribeToWeather() {
    ws.send(JSON.stringify({
        type: 'Subscribe',
        payload: {
            tenant_name: TENANT,
            service_name: SERVICE,
            variables: ['temperature']
        }
    }));
}

function checkTemperature(variables) {
    const temp = parseFloat(variables.temperature);
    const now = Date.now();
    
    // Avoid spamming alerts
    if (now - lastAlertTime < ALERT_COOLDOWN) {
        return;
    }
    
    if (temp > TEMP_THRESHOLD_HIGH) {
        sendAlert('🔥', `HOT ALERT: ${temp}°F (threshold: ${TEMP_THRESHOLD_HIGH}°F)`);
        lastAlertTime = now;
    } else if (temp < TEMP_THRESHOLD_LOW) {
        sendAlert('❄️ ', `COLD ALERT: ${temp}°F (threshold: ${TEMP_THRESHOLD_LOW}°F)`);
        lastAlertTime = now;
    }
}

function sendAlert(emoji, message) {
    console.log(`${emoji} ALERT: ${message}`);
    
    // In production, you could:
    // - Send email
    // - Send SMS
    // - Post to Slack
    // - Trigger automation
}

ws.on('error', (error) => {
    console.error('✗ Error:', error.message);
});
```

### Run the Alert System

In a **third terminal**:

```bash
node weather_alerts.js
```

**Expected output:**
```
✓ Alert system connected
✓ Alert system authenticated

🔥 ALERT: Hot temperature: 85.3°F (threshold: 85°F)
❄️  ALERT: Cold temperature: 54.8°F (threshold: 55°F)
```

---

## How It All Works Together

```
┌──────────────────────────────────────────────────┐
│                   COMMY SERVER                    │
│  (wss://127.0.0.1:8443)                          │
└────────────────┬──────────────────┬──────────────┘
                 │                  │
    ┌────────────┴──────┐  ┌───────┴──────┐
    │                   │  │              │
    ▼                   ▼  ▼              ▼
┌─────────────┐  ┌────────────────┐  ┌─────────┐
│   SENSOR    │  │   DASHBOARD    │  │ ALERTS  │
│ (Writes)    │  │  (Reads every  │  │(Watches)│
│             │  │    second)     │  │         │
│Temperature: │─────────────────────────────│
│  72°F       │         SERVICE:            │
│  73°F       │   current_conditions        │
│  74°F       │────────────────────────────►│
│  ...        │                             │
└─────────────┘  └────────────────────────┘  └─────────┘

Timeline:
T+0s:  Sensor sends 72°F
T+1s:  Dashboard reads 72°F -> displays
T+2s:  Sensor sends 73°F
T+3s:  Dashboard reads 73°F -> displays
       Alerts checks -> 73°F is OK, no alert
T+5s:  Sensor sends 85°F
       Dashboard reads 85°F -> displays
       Alerts checks -> 85°F TOO HIGH! -> ALERT!
```

---

## Step 5: Testing Your System

### Test 1: Basic Functionality

1. ✅ Start sensor - Should show "Updated: Temp=..."
2. ✅ Start dashboard - Should show live temperature
3. ✅ Start alerts - Should show system ready

### Test 2: Data Flow

1. Modify sensor code to send `temp = 90`
2. Watch dashboard update to show 90°F
3. Watch alert system trigger HIGH TEMP alert ✅

### Test 3: Multiple Clients

Open **4 dashboards** - all should show same temperature:
```bash
# Terminal 1
node weather_dashboard.js

# Terminal 2
node weather_dashboard.js

# Terminal 3  
node weather_dashboard.js

# Terminal 4
node weather_dashboard.js
```

All 4 dashboards update together! ✅

---

## Real-World Applications

### Application 1: Stock Trading

Replace "temperature" with "stock price":
```
Sensor   → Price Feed (external data)
Dashboard → Trader terminals
Alerts   → Risk management (stop losses)
```

### Application 2: IoT Smart Home

```
Sensors:  Temperature, humidity, light
Dashboard: Central control panel
Alerts:   Notifications on phone
```

### Application 3: Factory Automation

```
Sensors:  Machine status, production metrics
Dashboard: Factory floor monitor
Alerts:   Maintenance warnings
```

### Application 4: Cloud Infrastructure

```
Sensors:  CPU, memory, disk usage
Dashboard: Operations dashboard
Alerts:   PagerDuty/Slack notifications
```

---

## Troubleshooting

### Problem: "Connection refused"

**Cause:** Server not running  
**Solution:** Start server first in first terminal

```powershell
$env:COMMY_TLS_CERT_PATH = "dev-cert-temp.pem"
$env:COMMY_TLS_KEY_PATH = "dev-key-temp.pem"
.\target\debug\commy.exe
```

### Problem: "Authentication failed"

**Cause:** Wrong tenant name or API key  
**Solution:** Check TENANT and API_KEY match config

```javascript
const TENANT = 'weather';  // Must match everywhere
const API_KEY = 'sensor-key-123';  // Use API key method
```

### Problem: "Certificate error"

**Cause:** Invalid dev certificate  
**Solution:** Already handled with `rejectUnauthorized: false`

### Problem: Dashboard not updating

**Cause:** Sensor not sending data  
**Solution:** Check sensor is still running and authenticated

```bash
# Monitor sensor output
# Should see "→ Updated: Temp=XX°F" every 3 seconds
```

---

## Next Steps

### 1. Add More Features
- [ ] Store historical data
- [ ] Add graph visualization  
- [ ] Send email alerts
- [ ] Mobile app support

### 2. Move to Production
- [ ] Get real TLS certificates
- [ ] Set up PostgreSQL backend
- [ ] Deploy to cloud server
- [ ] Add monitoring

### 3. Scale Up
- [ ] Monitor 1000+ sensors
- [ ] Add data aggregation
- [ ] Implement caching
- [ ] Set up clustering

### 4. Build SDKs
- [ ] Python SDK
- [ ] C# SDK
- [ ] Go SDK

---

## Challenge: Build Your Own!

Try building one of these:

**Easy:**
- IoT temperature sensor
- System stats monitor
- Price tracker

**Medium:**
- Multi-location monitoring
- Alert rule engine
- Data logger

**Hard:**
- Real-time collaboration (like Google Docs)
- Distributed cache
- Event streaming system

---

## Conclusion

You've learned:
- ✅ How to start Commy server
- ✅ How to write sensor program
- ✅ How to read shared data
- ✅ How to set up alerts
- ✅ How multiple programs communicate

**Commy lets you:**
- 🚀 Build real-time systems
- 🔄 Coordinate multiple services
- 📊 Share live data efficiently
- ⚡ Respond instantly to changes

---

**You're ready to use Commy!** 🎉

Need help? Check:
- BEGINNERS_GUIDE.md - Commy concepts
- ARCHITECTURE.md - Technical details
- RUST_BASICS.md - Understanding code
