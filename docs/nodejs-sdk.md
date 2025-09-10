# Node.js SDK Documentation

The Commy Node.js SDK provides comprehensive async/await support for integrating Commy's distributed communication mesh into JavaScript and TypeScript applications.

## Installation

```bash
npm install @commy/nodejs-sdk
```

## Quick Start

```javascript
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');

async function main() {
    const mesh = new AsyncCommyMesh('my-service', 8080);

    try {
        await mesh.start();
        console.log('Mesh started successfully');

        // Your application logic here

    } finally {
        await mesh.stop();
    }
}

main().catch(console.error);
```

## TypeScript Support

```typescript
import { AsyncCommyMesh, ServiceInfo, MeshConfig } from '@commy/nodejs-sdk';

const config: MeshConfig = {
    heartbeatInterval: 5000,
    discoveryTimeout: 30000,
    maxConnections: 100,
};

const mesh = new AsyncCommyMesh('my-service', 8080, config);
```

## Core APIs

### `AsyncCommyMesh`

The main mesh interface for Node.js applications.

#### Constructor

```javascript
new AsyncCommyMesh(serviceName, port, config?)
```

**Parameters:**

- `serviceName` (string): Name of the service
- `port` (number): Port to bind to
- `config` (object, optional): Configuration object

#### Methods

##### `start(): Promise<void>`

Starts the mesh and begins service discovery.

```javascript
await mesh.start();
```

##### `stop(): Promise<void>`

Stops the mesh and cleans up resources.

```javascript
await mesh.stop();
```

##### `registerService(serviceInfo): Promise<void>`

Registers a service with the mesh.

```javascript
const serviceInfo = {
    name: 'user-api',
    type: 'api',
    address: '127.0.0.1:8080',
    metadata: { version: '1.0' }
};

await mesh.registerService(serviceInfo);
```

##### `discoverServices(serviceType): Promise<ServiceInfo[]>`

Discovers services of a specific type.

```javascript
const services = await mesh.discoverServices('api');
services.forEach(service => {
    console.log(`Found: ${service.name} at ${service.address}`);
});
```

##### `on(event, listener): void`

Registers event listeners for mesh events.

```javascript
mesh.on('serviceRegistered', (service) => {
    console.log('Service registered:', service.name);
});

mesh.on('serviceDiscovered', (service) => {
    console.log('Service discovered:', service.name);
});

mesh.on('error', (error) => {
    console.error('Mesh error:', error);
});
```

## Configuration

### Mesh Configuration

```javascript
const config = {
    heartbeatInterval: 5000,    // milliseconds
    discoveryTimeout: 30000,    // milliseconds
    maxConnections: 100,
    enableEncryption: true,
    logLevel: 'info'
};

const mesh = new AsyncCommyMesh('my-service', 8080, config);
```

### Environment Variables

```bash
# Set environment variables
export COMMY_LOG_LEVEL=debug
export COMMY_HEARTBEAT_INTERVAL=3000
export COMMY_MAX_CONNECTIONS=200
```

## Integration Examples

### Express.js Integration

```javascript
const express = require('express');
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');

const app = express();
const mesh = new AsyncCommyMesh('express-api', 8080);

app.use(express.json());

// Middleware for mesh integration
app.use(async (req, res, next) => {
    req.mesh = mesh;
    next();
});

app.get('/health', (req, res) => {
    res.json({
        status: 'healthy',
        service: 'express-api',
        meshRunning: mesh.isRunning()
    });
});

app.get('/services/:type', async (req, res) => {
    try {
        const services = await mesh.discoverServices(req.params.type);
        res.json({ services });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

app.post('/services', async (req, res) => {
    try {
        await mesh.registerService(req.body);
        res.json({ success: true });
    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

async function start() {
    try {
        await mesh.start();

        // Register the Express API service
        await mesh.registerService({
            name: 'express-api',
            type: 'api',
            address: '127.0.0.1:3000',
            metadata: {
                framework: 'express',
                version: '1.0',
                endpoints: ['/health', '/services']
            }
        });

        app.listen(3000, () => {
            console.log('Express API listening on port 3000');
        });

    } catch (error) {
        console.error('Failed to start:', error);
        process.exit(1);
    }
}

async function shutdown() {
    console.log('Shutting down...');
    await mesh.stop();
    process.exit(0);
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

start();
```

### Microservice Pattern

```javascript
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');
const { EventEmitter } = require('events');

class MicroService extends EventEmitter {
    constructor(name, port, serviceType = 'microservice') {
        super();
        this.name = name;
        this.port = port;
        this.serviceType = serviceType;
        this.mesh = new AsyncCommyMesh(name, port);
        this.isRunning = false;
    }

    async start() {
        try {
            await this.mesh.start();

            // Register service
            await this.mesh.registerService({
                name: this.name,
                type: this.serviceType,
                address: `127.0.0.1:${this.port}`,
                metadata: this.getServiceMetadata()
            });

            this.isRunning = true;
            this.emit('started');

            console.log(`${this.name} started on port ${this.port}`);

        } catch (error) {
            this.emit('error', error);
            throw error;
        }
    }

    async stop() {
        if (this.isRunning) {
            await this.mesh.stop();
            this.isRunning = false;
            this.emit('stopped');
            console.log(`${this.name} stopped`);
        }
    }

    async discoverService(serviceType) {
        return await this.mesh.discoverServices(serviceType);
    }

    getServiceMetadata() {
        return {
            version: '1.0',
            startTime: new Date().toISOString(),
            capabilities: this.getCapabilities()
        };
    }

    getCapabilities() {
        return ['health-check', 'discovery'];
    }
}

// User Service
class UserService extends MicroService {
    constructor() {
        super('user-service', 8080, 'user-api');
        this.users = new Map();
    }

    getCapabilities() {
        return [...super.getCapabilities(), 'user-management'];
    }

    async createUser(userData) {
        const userId = Date.now().toString();
        this.users.set(userId, { id: userId, ...userData });
        this.emit('userCreated', { userId, userData });
        return userId;
    }

    async getUser(userId) {
        return this.users.get(userId);
    }

    async getAllUsers() {
        return Array.from(this.users.values());
    }
}

// Order Service
class OrderService extends MicroService {
    constructor() {
        super('order-service', 8081, 'order-api');
        this.orders = new Map();
    }

    getCapabilities() {
        return [...super.getCapabilities(), 'order-management'];
    }

    async createOrder(orderData) {
        // Discover user service to validate user
        const userServices = await this.discoverService('user-api');
        if (userServices.length === 0) {
            throw new Error('User service not available');
        }

        const orderId = Date.now().toString();
        this.orders.set(orderId, { id: orderId, ...orderData });
        this.emit('orderCreated', { orderId, orderData });
        return orderId;
    }

    async getOrder(orderId) {
        return this.orders.get(orderId);
    }
}

// Usage
async function main() {
    const userService = new UserService();
    const orderService = new OrderService();

    // Set up event handlers
    userService.on('started', () => console.log('User service ready'));
    orderService.on('started', () => console.log('Order service ready'));

    userService.on('userCreated', (event) => {
        console.log('User created:', event.userId);
    });

    orderService.on('orderCreated', (event) => {
        console.log('Order created:', event.orderId);
    });

    try {
        // Start services
        await userService.start();
        await orderService.start();

        // Wait for service discovery
        setTimeout(async () => {
            try {
                // Test inter-service communication
                const userId = await userService.createUser({
                    name: 'John Doe',
                    email: 'john@example.com'
                });

                const orderId = await orderService.createOrder({
                    userId: userId,
                    items: ['laptop', 'mouse'],
                    total: 1299.99
                });

                console.log('Demo completed successfully');

            } catch (error) {
                console.error('Demo error:', error);
            }
        }, 2000);

        // Graceful shutdown
        setTimeout(async () => {
            await userService.stop();
            await orderService.stop();
        }, 10000);

    } catch (error) {
        console.error('Failed to start services:', error);
    }
}

main().catch(console.error);
```

### Event-Driven Communication

```javascript
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');
const { EventEmitter } = require('events');

class EventDrivenService extends EventEmitter {
    constructor(name, port) {
        super();
        this.name = name;
        this.mesh = new AsyncCommyMesh(name, port);
        this.eventHandlers = new Map();
    }

    async start() {
        await this.mesh.start();

        // Register as event service
        await this.mesh.registerService({
            name: this.name,
            type: 'event-service',
            address: `127.0.0.1:${this.mesh.port}`,
            metadata: {
                events: Array.from(this.eventHandlers.keys()),
                capabilities: ['event-publishing', 'event-subscription']
            }
        });

        console.log(`Event service ${this.name} started`);
    }

    async stop() {
        await this.mesh.stop();
    }

    // Register event handler
    onEvent(eventType, handler) {
        if (!this.eventHandlers.has(eventType)) {
            this.eventHandlers.set(eventType, []);
        }
        this.eventHandlers.get(eventType).push(handler);
    }

    // Publish event to other services
    async publishEvent(eventType, eventData) {
        const eventServices = await this.mesh.discoverServices('event-service');

        for (const service of eventServices) {
            if (service.name !== this.name) {
                // Send event to other service
                // In a real implementation, this would use the actual transport mechanism
                console.log(`Publishing ${eventType} to ${service.name}:`, eventData);
            }
        }

        // Emit locally
        this.emit(eventType, eventData);
    }

    // Handle incoming event
    handleEvent(eventType, eventData) {
        const handlers = this.eventHandlers.get(eventType) || [];
        handlers.forEach(handler => {
            try {
                handler(eventData);
            } catch (error) {
                console.error(`Error handling ${eventType}:`, error);
            }
        });
    }
}

// Example usage
async function eventExample() {
    const publisher = new EventDrivenService('publisher', 8080);
    const subscriber = new EventDrivenService('subscriber', 8081);

    // Set up event handlers
    subscriber.onEvent('user.created', (data) => {
        console.log('User created event received:', data);
    });

    subscriber.onEvent('order.placed', (data) => {
        console.log('Order placed event received:', data);
    });

    try {
        await publisher.start();
        await subscriber.start();

        // Wait for discovery
        setTimeout(async () => {
            // Publish events
            await publisher.publishEvent('user.created', {
                userId: '123',
                name: 'Alice',
                timestamp: new Date().toISOString()
            });

            await publisher.publishEvent('order.placed', {
                orderId: '456',
                userId: '123',
                amount: 99.99,
                timestamp: new Date().toISOString()
            });
        }, 2000);

        // Cleanup
        setTimeout(async () => {
            await publisher.stop();
            await subscriber.stop();
        }, 5000);

    } catch (error) {
        console.error('Event example error:', error);
    }
}

eventExample();
```

## Error Handling

### Error Types

```javascript
const { CommyError, MeshError, NetworkError } = require('@commy/nodejs-sdk');

try {
    await mesh.start();
} catch (error) {
    if (error instanceof MeshError) {
        console.error('Mesh error:', error.message);
    } else if (error instanceof NetworkError) {
        console.error('Network error:', error.message);
    } else if (error instanceof CommyError) {
        console.error('General Commy error:', error.message);
    } else {
        console.error('Unexpected error:', error);
    }
}
```

### Retry Logic

```javascript
async function withRetry(operation, maxRetries = 3, delay = 1000) {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            return await operation();
        } catch (error) {
            if (attempt === maxRetries) {
                throw error;
            }
            console.log(`Attempt ${attempt} failed, retrying in ${delay}ms...`);
            await new Promise(resolve => setTimeout(resolve, delay));
            delay *= 2; // Exponential backoff
        }
    }
}

// Usage
const mesh = new AsyncCommyMesh('my-service', 8080);
await withRetry(() => mesh.start());
```

## Testing

### Unit Testing with Jest

```javascript
const { AsyncCommyMesh } = require('@commy/nodejs-sdk');

describe('AsyncCommyMesh', () => {
    let mesh;

    beforeEach(() => {
        mesh = new AsyncCommyMesh('test-service', 8080);
    });

    afterEach(async () => {
        if (mesh.isRunning()) {
            await mesh.stop();
        }
    });

    test('should start and stop successfully', async () => {
        await mesh.start();
        expect(mesh.isRunning()).toBe(true);

        await mesh.stop();
        expect(mesh.isRunning()).toBe(false);
    });

    test('should register and discover services', async () => {
        await mesh.start();

        const serviceInfo = {
            name: 'test-api',
            type: 'api',
            address: '127.0.0.1:8080',
            metadata: { version: '1.0' }
        };

        await mesh.registerService(serviceInfo);

        const services = await mesh.discoverServices('api');
        expect(services.length).toBeGreaterThan(0);

        const foundService = services.find(s => s.name === 'test-api');
        expect(foundService).toBeDefined();
        expect(foundService.type).toBe('api');
    });
});
```

### Integration Testing

```javascript
describe('Mesh Integration', () => {
    let meshes = [];

    afterEach(async () => {
        // Cleanup all meshes
        await Promise.all(meshes.map(mesh => mesh.stop()));
        meshes = [];
    });

    test('should enable inter-service communication', async () => {
        // Create multiple mesh instances
        const mesh1 = new AsyncCommyMesh('service-1', 8080);
        const mesh2 = new AsyncCommyMesh('service-2', 8081);
        meshes.push(mesh1, mesh2);

        await Promise.all([mesh1.start(), mesh2.start()]);

        // Register services
        await mesh1.registerService({
            name: 'producer',
            type: 'producer',
            address: '127.0.0.1:8080'
        });

        await mesh2.registerService({
            name: 'consumer',
            type: 'consumer',
            address: '127.0.0.1:8081'
        });

        // Wait for discovery propagation
        await new Promise(resolve => setTimeout(resolve, 1000));

        // Test discovery from each mesh
        const producersFromMesh2 = await mesh2.discoverServices('producer');
        expect(producersFromMesh2.length).toBeGreaterThan(0);

        const consumersFromMesh1 = await mesh1.discoverServices('consumer');
        expect(consumersFromMesh1.length).toBeGreaterThan(0);
    });
});
```

## Performance Optimization

### Connection Pooling

```javascript
const optimizedConfig = {
    maxConnections: 200,
    connectionPoolSize: 50,
    keepAliveTimeout: 60000,
    enableCompression: true,
    batchingEnabled: true,
    batchSize: 100,
    flushInterval: 1000
};

const mesh = new AsyncCommyMesh('optimized-service', 8080, optimizedConfig);
```

### Memory Management

```javascript
// Set up periodic cleanup
setInterval(() => {
    if (global.gc) {
        global.gc();
    }
}, 30000);

// Monitor memory usage
mesh.on('memoryWarning', (usage) => {
    console.warn('High memory usage detected:', usage);
});
```

This completes the Node.js SDK documentation. The SDK provides full async/await support and integrates seamlessly with popular Node.js frameworks.
