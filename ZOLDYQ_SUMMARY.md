# ZoldyQ - Project Summary

**Ultra-fast, in-memory FIFO message queue in Rust**

---

## Overview

ZoldyQ is a minimal, high-performance, in-memory message queue built in Rust. It provides a simple FIFO queue with a publisher that pushes messages and a consumer that pulls messages. Messages are processed once and immediately removed from memory.

---

## Project Structure

```
zoldyq/
├── src/
│   ├── main.rs              # Application entry point
│   ├── lib.rs               # Library exports
│   ├── queue/
│   │   ├── mod.rs           # Queue module exports
│   │   ├── message.rs       # Message struct
│   │   ├── message_queue.rs # MessageQueue implementation
│   │   └── manager.rs       # QueueManager implementation
│   ├── server/
│   │   ├── mod.rs           # Server module exports
│   │   ├── http.rs          # HTTP handlers
│   │   ├── websocket.rs     # WebSocket handlers
│   │   └── health.rs        # Health check endpoints
│   ├── config/
│   │   ├── mod.rs           # Config module
│   │   └── settings.rs      # Configuration structs
│   └── metrics/
│       ├── mod.rs           # Metrics module
│       └── prometheus.rs    # Prometheus metrics
├── tests/
│   ├── message_queue_tests.rs  # MessageQueue unit tests
│   ├── queue_manager_tests.rs  # QueueManager unit tests
│   └── integration_tests.rs    # End-to-end integration tests
├── benches/
│   └── throughput.rs        # Performance benchmarks
├── Cargo.toml               # Dependencies and metadata
├── Dockerfile               # Container image
├── docker-compose.yml       # Local development setup
├── README.md                # Main documentation
├── ZOLDYQ_SUMMARY.md        # Project summary
└── ZOLDYQ_ROADMAP.md        # Implementation roadmap
```

---

## Core Principles

- **In-Memory Only** - No persistence, no disk I/O
- **FIFO Semantics** - First in, first out
- **Single Publisher** - One endpoint to enqueue messages
- **Single Consumer** - One endpoint to dequeue messages
- **Process-Once** - Messages consumed are removed from memory
- **Ultra-Low Latency** - Sub-millisecond message delivery
- **Minimal Design** - No topics, partitions, offsets, or replay

---

## Architecture

```
┌──────────────┐
│  Publisher   │
└──────┬───────┘
       │ enqueue to "emails"
       ▼
┌─────────────────────────────────────────┐
│        Queue Manager                    │
│                                         │
│  ┌─────────────┐  ┌─────────────┐     │
│  │ Queue:      │  │ Queue:      │     │
│  │ "emails"    │  │ "webhooks"  │ ... │
│  │ FIFO Buffer │  │ FIFO Buffer │     │
│  └──────┬──────┘  └──────┬──────┘     │
│         │                 │             │
└─────────┼─────────────────┼─────────────┘
          │ dequeue         │ dequeue
          ▼                 ▼
   ┌──────────────┐  ┌──────────────┐
   │  Consumer A  │  │  Consumer B  │
   └──────────────┘  └──────────────┘
```

**Key Components:**
- **Queue Manager** - Manages multiple named queues
- **Named Queues** - Independent FIFO queues (e.g., "emails", "webhooks", "images")
- **Publisher API** - HTTP/WebSocket endpoint to enqueue to specific queue
- **Consumer API** - HTTP/WebSocket endpoint to dequeue from specific queue
- **FIFO Buffer** - Each queue has its own in-memory bounded buffer
- **Backpressure** - Publisher blocks or rejects when queue is full

---

## When to Use ZoldyQ

### ✅ Ideal Use Cases

**ZoldyQ excels at:**

1. **Asynchronous Task Queues**
   - Email sending queue
   - SMS/notification delivery
   - Image processing pipeline
   - PDF generation tasks
   - Webhook dispatch

2. **Rate Limiting & Throttling**
   - API request buffering
   - Database write batching
   - Third-party API call queuing

3. **Background Job Processing**
   - Short-lived jobs (<1 minute)
   - Non-critical tasks
   - Retry-safe operations

4. **Decoupling Microservices**
   - HTTP request offloading
   - Service-to-service messaging
   - Event-driven architectures (transient events)

5. **High-Throughput Scenarios**
   - Real-time analytics ingestion
   - Log aggregation (temporary buffer)
   - Metrics collection

### 🟡 Acceptable Use Cases (with caveats)

**ZoldyQ can work for:**

1. **Development/Testing**
   - Local development queues
   - Integration testing
   - Staging environments with understood risks

2. **Low-Volume Critical Tasks**
   - When combined with proper monitoring
   - When paired with request retries
   - When message loss is tolerable

### ❌ Not Suitable For

**ZoldyQ is NOT appropriate for:**

1. **Critical Business Data**
   - Financial transactions
   - Payment processing
   - Order processing
   - Inventory updates

2. **Audit/Compliance Scenarios**
   - Audit logs
   - Legal records
   - Compliance data
   - Regulatory reporting

3. **Long-Running Tasks**
   - Video transcoding
   - Large data exports
   - ML model training
   - Multi-hour jobs

4. **Guaranteed Delivery**
   - When exactly-once processing is required
   - When message loss is unacceptable
   - When replay/retry from history is needed

5. **Distributed Systems**
   - Multi-node deployments
   - High-availability requirements
   - Geographic distribution

### ⚠️ Dangerous Use Cases

**Never use ZoldyQ for:**

- **Financial transactions** - Risk of monetary loss
- **Medical/healthcare data** - Patient safety risk
- **Critical infrastructure** - Service outage risk
- **User-generated content** - Data loss will upset users
- **Anything requiring persistence** - Data loss on restart/crash

---

## Design Decisions

### Why In-Memory Only?

- **Speed** - No disk I/O, sub-millisecond latency
- **Simplicity** - No storage engine, no file management
- **Ephemeral workloads** - Many tasks are retry-safe
- **Clear trade-off** - Users know what they're getting

### Why No Persistence?

- **Complexity** - Persistence adds significant code complexity
- **Performance** - Disk I/O introduces latency
- **Use case fit** - Target workloads are transient
- **Alternatives exist** - RabbitMQ, Kafka, Redis Streams for persistence

### Why Named Queues (Not Topics)?

- **Simplicity** - Named queues, no routing, no pub/sub broadcast
- **Isolation** - Each queue is independent (emails, webhooks, images, etc.)
- **Performance** - Lock-free per-queue operations
- **Flexibility** - Consumers pull from specific queues they care about

### Why No Acknowledgements?

- **Throughput** - No round-trip confirmation delays
- **Simplicity** - Pull means consumed
- **Consumer responsibility** - Consumer should be idempotent

---

## Performance Goals

```
Throughput:     500k-1M messages/sec
Latency P99:    <1ms (enqueue + dequeue)
Memory:         10-50MB idle, ~1GB at capacity
Queue Capacity: 100k-1M messages (configurable)
```

---

## Technical Stack

- **Language** - Rust (memory safety + performance)
- **Concurrency** - `tokio` async runtime
- **Queue** - `crossbeam-queue` or `tokio::sync::mpsc`
- **HTTP** - `axum` web framework
- **WebSocket** - `tokio-tungstenite`
- **Serialization** - `serde_json`

---

## API Design

### Publisher

**HTTP REST:**
```bash
POST /queues/{queue_name}/enqueue
Content-Type: application/json

{
  "payload": { "email": "user@example.com", "subject": "Welcome" }
}

# Response
{
  "queued": true,
  "queue_name": "emails",
  "queue_size": 1234
}
```

**WebSocket:**
```
ws://localhost:9000/queues/{queue_name}/enqueue
→ {"payload": {...}}
← {"queued": true, "queue_size": 1234}
```

### Consumer

**HTTP REST (Single Message):**
```bash
GET /queues/{queue_name}/dequeue?timeout=5

# Response (if message available)
{
  "payload": { "email": "user@example.com", "subject": "Welcome" },
  "queue_name": "emails",
  "queue_size": 1233
}

# Response (if timeout - 204 No Content)
```

**WebSocket (Streaming):**
```
ws://localhost:9000/queues/{queue_name}/dequeue
← {"payload": {...}}
← {"payload": {...}}
← {"payload": {...}}
```

The consumer receives messages continuously as they become available. Connection stays open until closed by client.

### Health

```bash
GET /health

# Response
{
  "status": "healthy",
  "queues": {
    "emails": {
      "size": 1234,
      "capacity": 100000,
      "enqueued_total": 5678901,
      "dequeued_total": 5677667
    },
    "webhooks": {
      "size": 42,
      "capacity": 100000,
      "enqueued_total": 12345,
      "dequeued_total": 12303
    }
  },
  "uptime_seconds": 86400
}
```

---

## Deployment

### Docker (Production)

```bash
docker run -d \
  -p 9000:9000 \
  -e ZOLDYQ_DEFAULT_QUEUE_CAPACITY=100000 \
  -e ZOLDYQ_MAX_QUEUES=100 \
  zoldyq/zoldyq:latest
```

### Binary

```bash
# Download
curl -L https://github.com/zoldyq/zoldyq/releases/latest/download/zoldyq-linux -o zoldyq
chmod +x zoldyq

# Run
./zoldyq --port 9000 --capacity 100000
```

### Environment Variables

```bash
ZOLDYQ_PORT=9000                      # Server port
ZOLDYQ_HOST=0.0.0.0                   # Bind address
ZOLDYQ_DEFAULT_QUEUE_CAPACITY=100000  # Max messages per queue
ZOLDYQ_MAX_QUEUES=100                 # Max number of queues
ZOLDYQ_LOG_LEVEL=info                 # debug|info|warn|error
```

---

## Operational Characteristics

### What Happens on Restart?

- ❌ **All messages are lost** - No persistence
- Use ZoldyQ for retry-safe workloads only

### What Happens When Queue is Full?

- **HTTP enqueue** - Returns 429 Too Many Requests
- **Consumer responsibility** - Consume faster or scale consumers

### What Happens on Crash?

- ❌ **All messages are lost** - No crash recovery
- Monitor process health and restart automatically

### Monitoring

```
Metrics to track:
- Queue size (current)
- Queue capacity (max)
- Enqueue rate (msg/sec)
- Dequeue rate (msg/sec)
- Queue full events (count)
- Dequeue timeout events (count)
```

---

## Comparison

| Feature | ZoldyQ | Redis Pub/Sub | RabbitMQ | Kafka |
|---------|--------|---------------|----------|-------|
| Persistence | ❌ None | ❌ None | ✅ Yes | ✅ Yes |
| Latency | <1ms | <2ms | 5-10ms | 10-20ms |
| Throughput | 1M/s | 100k/s | 50k/s | 500k/s |
| Setup | 30 sec | 30 sec | 5 min | 30 min |
| Complexity | Minimal | Low | Medium | High |
| FIFO Queue | ✅ Yes | ❌ No | ✅ Yes | ✅ Yes |
| Named Queues | ✅ Yes | ❌ No | ✅ Yes | ✅ Yes |
| WebSocket | ✅ Yes | ❌ No | ✅ Yes | ❌ No |
| Consumer Groups | ❌ No | ❌ No | ✅ Yes | ✅ Yes |
| Replay | ❌ No | ❌ No | ❌ No | ✅ Yes |

---

## Alternatives

**When ZoldyQ is not enough:**

- **Need persistence?** → RabbitMQ, Redis Streams, Kafka
- **Need replay?** → Kafka, Redis Streams
- **Need HA/clustering?** → RabbitMQ, Kafka, NATS JetStream
- **Need exactly-once?** → Kafka with transactions
- **Need routing?** → RabbitMQ, NATS

---

## License

Apache 2.0 or MIT (dual-licensed)

---

## Tagline

**ZoldyQ: Fast, ephemeral, no regrets.**
