# ZoldyQ Python Client

Python client library for ZoldyQ message queue using the native ZQP protocol.

## Installation

```bash
pip install zoldyq
```

## Quick Start

### Synchronous API

```python
from zoldyq import ZoldyQ

# Using context manager
with ZoldyQ('localhost', 6380) as client:
    # Push a message
    msg_id = client.push('tasks', {'job': 'process-image', 'id': 123})
    print(f"Pushed message: {msg_id}")
    
    # Pop a message (blocking with timeout)
    msg = client.pop('tasks', timeout=30)
    if msg:
        print(f"Got message: {msg.payload}")
        client.ack(msg.id)
    
    # Check queue length
    length = client.length('tasks')
    print(f"Queue length: {length}")
```

### Asynchronous API

```python
import asyncio
from zoldyq import ZoldyQAsync

async def main():
    async with ZoldyQAsync('localhost', 6380) as client:
        # Push messages
        await client.push('tasks', {'job': 'task-1'})
        await client.push('tasks', {'job': 'task-2'})
        
        # Pop a message
        msg = await client.pop('tasks', timeout=5)
        if msg:
            print(f"Processing: {msg.payload}")
            await client.ack(msg.id)

asyncio.run(main())
```

### Subscription (Server Push)

```python
import asyncio
from zoldyq import ZoldyQAsync

async def consumer():
    async with ZoldyQAsync('localhost', 6380) as client:
        async for msg in client.subscribe('tasks'):
            print(f"Received: {msg.payload}")
            await client.ack(msg.id)

asyncio.run(consumer())
```

## API Reference

### ZoldyQ (Synchronous)

- `connect()` - Connect to the server
- `close()` - Close the connection
- `ping(message=None)` - Health check
- `push(queue, payload)` - Push a message, returns message ID
- `pop(queue, timeout=0)` - Pop a message, returns Message or None
- `ack(message_id)` - Acknowledge a message
- `nack(message_id)` - Reject a message (will be requeued)
- `length(queue)` - Get queue length
- `delete(queue)` - Delete a queue

### ZoldyQAsync (Asynchronous)

Same methods as ZoldyQ, but async. Plus:

- `subscribe(queue)` - Async iterator that yields messages as they arrive
- `unsubscribe(queue)` - Stop receiving messages from a queue

### Message

- `id` - Unique message identifier
- `queue` - Queue name
- `payload` - Message payload (any JSON-serializable data)

## Authentication

```python
# Sync
client = ZoldyQ('localhost', 6380, password='secret')
client.connect()

# Async
client = ZoldyQAsync('localhost', 6380, password='secret')
await client.connect()
```

## License

MIT
