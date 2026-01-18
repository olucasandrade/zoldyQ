# ZQP (ZoldyQ Protocol) Specification

**Version**: 1.0  
**Status**: Stable  
**Default Port**: 6380

ZQP is a binary protocol for ZoldyQ message queue communication. It uses MessagePack for serialization with length-prefixed framing, designed for high performance and simplicity.

## Wire Format

Every message (request, response, and server push) follows this frame structure:

```
┌────────────────┬──────────────────────────────────────────┐
│ Length (4B)    │ MessagePack Payload (variable)           │
│ Little-Endian  │                                          │
└────────────────┴──────────────────────────────────────────┘
```

| Field | Size | Description |
|-------|------|-------------|
| Length | 4 bytes | Payload size in bytes (little-endian uint32) |
| Payload | Variable | MessagePack-encoded data |

**Maximum frame size**: 16 MB (16,777,216 bytes)

## Request Format

All requests are MessagePack maps with the following structure:

```
{
  "cmd": string,        // Required: command name
  "queue": string,      // Optional: queue name
  "payload": any,       // Optional: message payload (JSON-compatible)
  "timeout": uint64,    // Optional: timeout in seconds
  "id": string,         // Optional: message ID
  "password": string    // Optional: authentication password
}
```

### Commands

| Command | Required Fields | Optional Fields | Description |
|---------|-----------------|-----------------|-------------|
| `auth` | `password` | - | Authenticate connection |
| `ping` | - | `payload` | Health check |
| `push` | `queue`, `payload` | - | Enqueue a message |
| `pop` | `queue` | `timeout` | Dequeue a message |
| `subscribe` | `queue` | - | Subscribe to queue (server push) |
| `unsubscribe` | `queue` | - | Unsubscribe from queue |
| `ack` | `id` | - | Acknowledge message |
| `nack` | `id` | - | Reject message (requeue) |
| `len` | `queue` | - | Get queue length |
| `del` | `queue` | - | Delete queue |

## Response Format

All responses are MessagePack maps:

```
{
  "ok": bool,           // Required: success status
  "error": string,      // Present if ok=false
  "id": string,         // Message ID (push/pop)
  "payload": any,       // Message payload (pop)
  "queue": string,      // Queue name (pop)
  "length": uint64,     // Queue length (len/push/del)
  "pong": string        // Ping response
}
```

### Success Response

```
{"ok": true, ...additional fields...}
```

### Error Response

```
{"ok": false, "error": "Error message"}
```

## Server Push Format

When subscribed to a queue, the server pushes messages without waiting for requests:

```
{
  "type": "message",    // Push type
  "id": string,         // Message ID
  "queue": string,      // Queue name
  "payload": any        // Message payload
}
```

## Command Reference

### AUTH

Authenticate the connection. Required if server has password configured.

**Request:**
```
{"cmd": "auth", "password": "secret"}
```

**Response:**
```
{"ok": true}
```

**Errors:**
- `WRONGPASS invalid password` - Incorrect password
- `NOAUTH Authentication required` - Auth required but not provided

---

### PING

Health check. Optionally echoes back a message.

**Request:**
```
{"cmd": "ping"}
{"cmd": "ping", "payload": "hello"}
```

**Response:**
```
{"ok": true, "pong": "PONG"}
{"ok": true, "pong": "hello"}
```

---

### PUSH

Enqueue a message to a queue. Creates the queue if it doesn't exist.

**Request:**
```
{"cmd": "push", "queue": "tasks", "payload": {"job": "process", "id": 123}}
```

**Response:**
```
{"ok": true, "id": "550e8400-e29b-41d4-a716-446655440000", "length": 1}
```

**Errors:**
- `Missing 'queue' field`
- `Missing 'payload' field`
- `Queue is full`
- `Maximum number of queues reached`

---

### POP

Dequeue a message from a queue.

**Request:**
```
{"cmd": "pop", "queue": "tasks"}
{"cmd": "pop", "queue": "tasks", "timeout": 30}
```

**Response (message found):**
```
{
  "ok": true,
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "queue": "tasks",
  "payload": {"job": "process", "id": 123}
}
```

**Response (no message, timeout=0 or expired):**
```
{"ok": true}
```

**Notes:**
- `timeout=0` (default): Non-blocking, returns immediately
- `timeout>0`: Blocks until message available or timeout expires
- If ACK mode is enabled, message is tracked for acknowledgment

---

### SUBSCRIBE

Subscribe to receive messages from a queue via server push.

**Request:**
```
{"cmd": "subscribe", "queue": "tasks"}
```

**Response:**
```
{"ok": true}
```

After subscription, server pushes messages as they arrive:
```
{"type": "message", "id": "...", "queue": "tasks", "payload": {...}}
```

**Notes:**
- Multiple queues can be subscribed simultaneously
- Messages must still be ACKed if ACK mode is enabled
- Subscription persists until unsubscribe or disconnect

---

### UNSUBSCRIBE

Stop receiving messages from a queue.

**Request:**
```
{"cmd": "unsubscribe", "queue": "tasks"}
```

**Response:**
```
{"ok": true}
```

---

### ACK

Acknowledge successful processing of a message. Only available when ACK mode is enabled.

**Request:**
```
{"cmd": "ack", "id": "550e8400-e29b-41d4-a716-446655440000"}
```

**Response:**
```
{"ok": true}
```

**Errors:**
- `ACK mode not enabled`
- `Missing 'id' field`
- `Message {id} not found in flight`

---

### NACK

Reject a message and return it to the queue. Only available when ACK mode is enabled.

**Request:**
```
{"cmd": "nack", "id": "550e8400-e29b-41d4-a716-446655440000"}
```

**Response:**
```
{"ok": true}
```

**Notes:**
- Message is returned to the original queue
- Delivery count is incremented
- After max retries, message moves to Dead Letter Queue (`{queue}:dlq`)

**Errors:**
- `ACK mode not enabled`
- `Missing 'id' field`
- `Message {id} not found in flight`

---

### LEN

Get the current length of a queue.

**Request:**
```
{"cmd": "len", "queue": "tasks"}
```

**Response:**
```
{"ok": true, "length": 42}
```

**Notes:**
- Returns 0 for non-existent queues

---

### DEL

Delete a queue and all its messages.

**Request:**
```
{"cmd": "del", "queue": "tasks"}
```

**Response:**
```
{"ok": true, "length": 1}
```

**Notes:**
- `length=1` if queue existed and was deleted
- `length=0` if queue didn't exist

## Connection Lifecycle

### Basic Flow

```
Client                              Server
   │                                   │
   │──────── [Connect TCP] ───────────>│
   │                                   │
   │──────── AUTH (if required) ──────>│
   │<─────── {"ok": true} ─────────────│
   │                                   │
   │──────── PUSH ────────────────────>│
   │<─────── {"ok": true, "id": ...} ──│
   │                                   │
   │──────── POP ─────────────────────>│
   │<─────── {"ok": true, ...} ────────│
   │                                   │
   │──────── [Close TCP] ─────────────>│
   │                                   │
```

### Subscription Flow

```
Client                              Server
   │                                   │
   │──────── SUBSCRIBE ───────────────>│
   │<─────── {"ok": true} ─────────────│
   │                                   │
   │<─────── {"type": "message", ...} ─│  (server push)
   │──────── ACK ─────────────────────>│
   │<─────── {"ok": true} ─────────────│
   │                                   │
   │<─────── {"type": "message", ...} ─│  (server push)
   │──────── NACK ────────────────────>│
   │<─────── {"ok": true} ─────────────│
   │                                   │
   │──────── UNSUBSCRIBE ─────────────>│
   │<─────── {"ok": true} ─────────────│
   │                                   │
```

## Error Handling

### Error Response Structure

All errors follow the format:
```
{"ok": false, "error": "Error description"}
```

### Common Errors

| Error | Cause |
|-------|-------|
| `NOAUTH Authentication required` | Server requires auth, client not authenticated |
| `WRONGPASS invalid password` | Invalid password provided |
| `Missing 'queue' field` | Command requires queue but none provided |
| `Missing 'payload' field` | PUSH command without payload |
| `Missing 'id' field` | ACK/NACK without message ID |
| `Queue is full` | Queue capacity exceeded |
| `Maximum number of queues reached` | Server queue limit reached |
| `ACK mode not enabled` | ACK/NACK when server ACK mode disabled |
| `Message {id} not found in flight` | ACK/NACK for unknown message |
| `Invalid request: ...` | Malformed MessagePack data |

## Implementation Notes

### Byte Order

All multi-byte integers use **little-endian** byte order.

### MessagePack

- Use MessagePack's map type for all structures
- String keys are case-sensitive
- Payload can be any valid MessagePack value (maps, arrays, strings, numbers, etc.)
- Binary data should use MessagePack's bin type

### Connection

- TCP-based, no TLS in current version
- One connection per client recommended
- Connection is stateful (auth state, subscriptions)
- No built-in keepalive; implement at application level if needed

### Threading

- Protocol is request-response based (except subscriptions)
- Clients should not pipeline requests (send one, wait for response)
- Subscriptions can receive pushes at any time after subscribe

## Example: Python Implementation

```python
import socket
import struct
import msgpack

def send_request(sock, request):
    payload = msgpack.packb(request)
    header = struct.pack('<I', len(payload))
    sock.sendall(header + payload)

def recv_response(sock):
    header = sock.recv(4)
    length = struct.unpack('<I', header)[0]
    payload = sock.recv(length)
    return msgpack.unpackb(payload, raw=False)

# Connect and push
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.connect(('localhost', 6380))

send_request(sock, {'cmd': 'push', 'queue': 'tasks', 'payload': {'job': 1}})
response = recv_response(sock)
print(response)  # {'ok': True, 'id': '...', 'length': 1}
```

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2024-01 | Initial specification |
