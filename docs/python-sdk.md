# Python SDK Documentation

The Commy Python SDK provides async/await support for integrating Commy's distributed communication mesh into Python applications.

## Installation

```bash
pip install commy-python
```

## Quick Start

```python
import asyncio
from commy_async import AsyncCommyMesh, create_mesh_cluster, shutdown_mesh_cluster

async def main():
    # Create a mesh cluster
    node_configs = [
        ("node-1", 8080),
        ("node-2", 8081),
    ]

    meshes = await create_mesh_cluster(node_configs)
    print(f"Created mesh cluster with {len(meshes)} nodes")

    # Use the mesh for your application
    # ... application logic ...

    # Clean shutdown
    await shutdown_mesh_cluster(meshes)

if __name__ == "__main__":
    asyncio.run(main())
```

## Core APIs

### `AsyncCommyMesh`

The main mesh interface for Python applications.

```python
from commy_async import AsyncCommyMesh

mesh = AsyncCommyMesh("my-service", 8080)
await mesh.start()
```

#### Constructor

```python
AsyncCommyMesh(service_name: str, port: int, config: Optional[Dict] = None)
```

**Parameters:**

- `service_name`: Name of the service
- `port`: Port to bind to
- `config`: Optional configuration dictionary

#### Methods

##### `start() -> None`

Starts the mesh and begins service discovery.

```python
await mesh.start()
```

##### `stop() -> None`

Stops the mesh and cleans up resources.

```python
await mesh.stop()
```

##### `register_service(service_info: Dict) -> None`

Registers a service with the mesh.

```python
service_info = {
    "name": "user-api",
    "type": "api",
    "address": "127.0.0.1:8080",
    "metadata": {}
}
await mesh.register_service(service_info)
```

##### `discover_services(service_type: str) -> List[Dict]`

Discovers services of a specific type.

```python
services = await mesh.discover_services("api")
for service in services:
    print(f"Found service: {service['name']} at {service['address']}")
```

### Utility Functions

#### `create_mesh_cluster(node_configs: List[Tuple[str, int]]) -> List[AsyncCommyMesh]`

Creates a mesh cluster with multiple nodes.

```python
node_configs = [
    ("producer", 8080),
    ("consumer", 8081),
    ("processor", 8082),
]

meshes = await create_mesh_cluster(node_configs)
```

#### `shutdown_mesh_cluster(meshes: List[AsyncCommyMesh]) -> None`

Shuts down all nodes in a mesh cluster.

```python
await shutdown_mesh_cluster(meshes)
```

## Configuration

### Mesh Configuration

```python
config = {
    "heartbeat_interval": 5.0,  # seconds
    "discovery_timeout": 30.0,  # seconds
    "max_connections": 100,
    "enable_encryption": True,
}

mesh = AsyncCommyMesh("my-service", 8080, config)
```

### Logging Configuration

```python
import logging

# Configure logging for Commy
logging.getLogger("commy").setLevel(logging.INFO)
```

## Integration Examples

### FastAPI Integration

```python
from fastapi import FastAPI
from commy_async import AsyncCommyMesh
import asyncio

app = FastAPI()
mesh = None

@app.on_event("startup")
async def startup_event():
    global mesh
    mesh = AsyncCommyMesh("fastapi-service", 8080)
    await mesh.start()

    # Register API service
    service_info = {
        "name": "user-api",
        "type": "api",
        "address": "127.0.0.1:8000",
        "metadata": {"version": "1.0", "endpoints": ["/users", "/health"]}
    }
    await mesh.register_service(service_info)

@app.on_event("shutdown")
async def shutdown_event():
    global mesh
    if mesh:
        await mesh.stop()

@app.get("/health")
async def health():
    return {"status": "healthy", "service": "user-api"}

@app.get("/discover")
async def discover_services():
    services = await mesh.discover_services("api")
    return {"services": services}
```

### Producer-Consumer Pattern

```python
import asyncio
from commy_async import AsyncCommyMesh
import json

class Producer:
    def __init__(self, name: str, port: int):
        self.mesh = AsyncCommyMesh(name, port)

    async def start(self):
        await self.mesh.start()

    async def produce_message(self, topic: str, message: dict):
        # Discover consumer services
        consumers = await self.mesh.discover_services("consumer")

        for consumer in consumers:
            # Send message to consumer
            # Implementation depends on transport mechanism
            print(f"Sending to {consumer['name']}: {message}")

    async def stop(self):
        await self.mesh.stop()

class Consumer:
    def __init__(self, name: str, port: int):
        self.mesh = AsyncCommyMesh(name, port)

    async def start(self):
        await self.mesh.start()

        # Register as consumer service
        service_info = {
            "name": self.mesh.service_name,
            "type": "consumer",
            "address": f"127.0.0.1:{port}",
            "metadata": {"topics": ["orders", "events"]}
        }
        await self.mesh.register_service(service_info)

    async def consume_message(self, message: dict):
        print(f"Processing message: {message}")
        # Process the message

    async def stop(self):
        await self.mesh.stop()

# Usage
async def main():
    producer = Producer("producer-1", 8080)
    consumer = Consumer("consumer-1", 8081)

    try:
        await producer.start()
        await consumer.start()

        # Wait for service discovery
        await asyncio.sleep(2)

        # Produce messages
        await producer.produce_message("orders", {"id": 1, "item": "laptop"})
        await producer.produce_message("orders", {"id": 2, "item": "mouse"})

        # Keep running
        await asyncio.sleep(10)

    finally:
        await producer.stop()
        await consumer.stop()

if __name__ == "__main__":
    asyncio.run(main())
```

### Microservice Discovery

```python
import asyncio
from commy_async import AsyncCommyMesh

class ServiceRegistry:
    def __init__(self):
        self.mesh = AsyncCommyMesh("service-registry", 8090)
        self.registered_services = {}

    async def start(self):
        await self.mesh.start()

    async def register_microservice(self, service_info: dict):
        """Register a microservice in the registry"""
        service_name = service_info["name"]
        self.registered_services[service_name] = service_info
        await self.mesh.register_service(service_info)
        print(f"Registered service: {service_name}")

    async def discover_microservice(self, service_name: str) -> dict:
        """Discover a specific microservice"""
        if service_name in self.registered_services:
            return self.registered_services[service_name]

        # Try mesh discovery
        services = await self.mesh.discover_services("microservice")
        for service in services:
            if service["name"] == service_name:
                return service

        return None

    async def list_all_services(self) -> list:
        """List all available services"""
        return await self.mesh.discover_services("microservice")

    async def stop(self):
        await self.mesh.stop()

# Example microservice
class UserService:
    def __init__(self):
        self.mesh = AsyncCommyMesh("user-service", 8080)

    async def start(self):
        await self.mesh.start()

        # Register with service registry
        service_info = {
            "name": "user-service",
            "type": "microservice",
            "address": "127.0.0.1:8080",
            "metadata": {
                "version": "1.0",
                "endpoints": ["/users", "/users/{id}"],
                "health_check": "/health"
            }
        }
        await self.mesh.register_service(service_info)

    async def stop(self):
        await self.mesh.stop()

# Usage
async def main():
    registry = ServiceRegistry()
    user_service = UserService()

    try:
        await registry.start()
        await user_service.start()

        # Wait for registration
        await asyncio.sleep(2)

        # Discover the user service
        discovered = await registry.discover_microservice("user-service")
        print(f"Discovered: {discovered}")

        # List all services
        all_services = await registry.list_all_services()
        print(f"All services: {all_services}")

    finally:
        await user_service.stop()
        await registry.stop()

if __name__ == "__main__":
    asyncio.run(main())
```

## Error Handling

### Exception Types

```python
from commy_async import CommyError, MeshError, NetworkError

try:
    await mesh.start()
except MeshError as e:
    print(f"Mesh error: {e}")
except NetworkError as e:
    print(f"Network error: {e}")
except CommyError as e:
    print(f"General Commy error: {e}")
```

### Retry Patterns

```python
import asyncio
from commy_async import AsyncCommyMesh, NetworkError

async def retry_operation(operation, max_retries=3, delay=1.0):
    for attempt in range(max_retries):
        try:
            return await operation()
        except NetworkError as e:
            if attempt == max_retries - 1:
                raise e
            print(f"Attempt {attempt + 1} failed, retrying in {delay}s...")
            await asyncio.sleep(delay)
            delay *= 2  # Exponential backoff

# Usage
mesh = AsyncCommyMesh("my-service", 8080)
await retry_operation(mesh.start)
```

## Testing

### Unit Testing

```python
import pytest
from commy_async import AsyncCommyMesh

@pytest.mark.asyncio
async def test_mesh_lifecycle():
    mesh = AsyncCommyMesh("test-service", 8080)

    # Test startup
    await mesh.start()
    assert mesh.is_running()

    # Test service registration
    service_info = {
        "name": "test-api",
        "type": "api",
        "address": "127.0.0.1:8080"
    }
    await mesh.register_service(service_info)

    # Test service discovery
    services = await mesh.discover_services("api")
    assert len(services) >= 1

    # Test shutdown
    await mesh.stop()
    assert not mesh.is_running()

@pytest.mark.asyncio
async def test_mesh_cluster():
    node_configs = [("node-1", 8080), ("node-2", 8081)]
    meshes = await create_mesh_cluster(node_configs)

    assert len(meshes) == 2

    # Test inter-node communication
    # ... test implementation ...

    await shutdown_mesh_cluster(meshes)
```

### Integration Testing

```python
import asyncio
import pytest
from commy_async import create_mesh_cluster, shutdown_mesh_cluster

@pytest.mark.asyncio
async def test_service_discovery():
    # Create a multi-node cluster
    node_configs = [
        ("producer", 8080),
        ("consumer", 8081),
        ("processor", 8082),
    ]

    meshes = await create_mesh_cluster(node_configs)

    try:
        # Register services
        services_to_register = [
            {"name": "producer", "type": "producer", "address": "127.0.0.1:8080"},
            {"name": "consumer", "type": "consumer", "address": "127.0.0.1:8081"},
            {"name": "processor", "type": "processor", "address": "127.0.0.1:8082"},
        ]

        for i, service_info in enumerate(services_to_register):
            await meshes[i].register_service(service_info)

        # Wait for propagation
        await asyncio.sleep(2)

        # Test discovery from each node
        for mesh in meshes:
            discovered = await mesh.discover_services("producer")
            assert len(discovered) >= 1

    finally:
        await shutdown_mesh_cluster(meshes)
```

## Performance Optimization

### Connection Pooling

```python
from commy_async import AsyncCommyMesh

class OptimizedMesh:
    def __init__(self, service_name: str, port: int):
        self.mesh = AsyncCommyMesh(service_name, port, {
            "max_connections": 200,
            "connection_pool_size": 50,
            "keepalive_timeout": 60,
        })

    async def start(self):
        await self.mesh.start()

    async def stop(self):
        await self.mesh.stop()
```

### Async Context Manager

```python
from commy_async import AsyncCommyMesh

class MeshManager:
    def __init__(self, service_name: str, port: int):
        self.service_name = service_name
        self.port = port
        self.mesh = None

    async def __aenter__(self):
        self.mesh = AsyncCommyMesh(self.service_name, self.port)
        await self.mesh.start()
        return self.mesh

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.mesh:
            await self.mesh.stop()

# Usage
async def main():
    async with MeshManager("my-service", 8080) as mesh:
        # Use mesh here
        await mesh.register_service(service_info)
        # Automatic cleanup on exit
```

This completes the Python SDK documentation. The SDK provides full async/await support and integrates well with modern Python frameworks like FastAPI.
