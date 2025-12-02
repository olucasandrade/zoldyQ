# ZoldyQ - Implementation Roadmap

**Minimal, ultra-fast, in-memory FIFO message queue**

---

## Table of Contents

1. [Project Scope](#project-scope)
2. [Architecture](#architecture)
3. [Implementation Plan](#implementation-plan)
4. [API Specification](#api-specification)
5. [Testing Strategy](#testing-strategy)
6. [Performance Benchmarks](#performance-benchmarks)
7. [How to Use](#how-to-use)
8. [Deployment Guide](#deployment-guide)

---

## Project Scope

### What ZoldyQ IS

- In-memory FIFO message queue
- Multiple named queues (e.g., "emails", "webhooks", "images")
- HTTP REST and WebSocket interfaces for enqueue/dequeue
- WebSocket streaming for continuous message consumption
- Bounded queues with backpressure
- Sub-millisecond latency
- Thread-safe concurrent access

### What ZoldyQ IS NOT

- ❌ Not a persistent queue
- ❌ Not a distributed system
- ❌ Not a pub/sub system
- ❌ No topics or partitions
- ❌ No message replay
- ❌ No offset tracking
- ❌ No consumer groups
- ❌ No acknowledgements
- ❌ No retention policies
- ❌ No replication

---

## Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────┐
│           HTTP/WebSocket Server                  │
│              (Axum + Tokio)                      │
└────────────┬────────────────┬───────────────────┘
             │                │
     ┌───────▼─────┐   ┌─────▼──────┐
     │   Enqueue   │   │  Dequeue   │
     │   Handler   │   │  Handler   │
     └───────┬─────┘   └─────┬──────┘
             │               │
             ▼               ▼
    ┌─────────────────────────────────────────┐
    │        Queue Manager                    │
    │      (DashMap<String, Queue>)           │
    │                                         │
    │  ┌────────────────┐  ┌────────────────┐│
    │  │ Queue: "emails"│  │Queue: "webhooks"││
    │  │ ArrayQueue     │  │ ArrayQueue     ││
    │  │ Capacity: 100k │  │ Capacity: 100k ││
    │  └────────────────┘  └────────────────┘│
    └─────────────────────────────────────────┘
```

### Component Breakdown

#### 1. HTTP/WebSocket Server
- **Framework:** `axum` (high-performance, tokio-based)
- **Runtime:** `tokio` (async runtime)
- **Endpoints:**
  - `POST /queues/{queue_name}/enqueue` - Add message to named queue
  - `GET /queues/{queue_name}/dequeue` - Remove message from named queue
  - `GET /health` - All queues status
  - `GET /metrics` - Prometheus metrics
  - `WS /queues/{queue_name}/enqueue` - WebSocket enqueue stream
  - `WS /queues/{queue_name}/dequeue` - WebSocket dequeue stream (continuous)

#### 2. Queue Manager & Named Queues
- **Manager:** `DashMap<String, Arc<MessageQueue>>` (concurrent hashmap)
- **Queue Implementation:** `crossbeam::queue::ArrayQueue` or `tokio::sync::mpsc`
- **Auto-creation:** Queues created on first enqueue
- **Capacity:** Configurable per queue (default: 100k messages)
- **Behavior:** 
  - Enqueue when full → Reject (429 error)
  - Dequeue when empty → Wait (blocking with timeout) or stream via WebSocket

#### 3. Message Format
```rust
pub struct Message {
    pub payload: serde_json::Value,  // Arbitrary JSON
    pub enqueued_at: u64,             // Timestamp (microseconds)
}
```

---

## Implementation Plan

### Phase 1: Core Queue (Week 1)

#### Day 1-2: Project Setup
- [x] Initialize Rust project (`cargo new zoldyq`)
- [x] Add dependencies:
  - `tokio = { version = "1", features = ["full"] }`
  - `axum = "0.7"`
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `crossbeam = "0.8"`
  - `tracing = "0.1"`
  - `tracing-subscriber = "0.3"`
- [x] Setup CI/CD (GitHub Actions)
- [x] Create Dockerfile
- [x] Write README

#### Day 3-4: Core Queue Implementation

**Tests Created:** ✅
- `tests/message_queue_tests.rs` - 25 unit tests
- `tests/queue_manager_tests.rs` - 25 unit tests  
- `tests/integration_tests.rs` - 15 integration tests

**Implementation Tasks:**
- [ ] Implement `Message` struct in `src/queue/message.rs`
  ```rust
  pub struct Message {
      pub payload: serde_json::Value,
      pub enqueued_at: u64,  // Microseconds since epoch
  }
  ```
- [ ] Implement `MessageQueue` struct in `src/queue/message_queue.rs`
  ```rust
  pub struct MessageQueue {
      name: String,
      queue: Arc<ArrayQueue<Message>>,
      capacity: usize,
      stats: Arc<QueueStats>,
      notify: Arc<Notify>,  // For dequeue waiting
  }
  ```
- [ ] Implement `QueueManager` struct in `src/queue/manager.rs`
  ```rust
  pub struct QueueManager {
      queues: Arc<DashMap<String, Arc<MessageQueue>>>,
      default_capacity: usize,
      max_queues: usize,
  }
  ```
- [ ] Implement `enqueue()` method
  - Add message to queue
  - Notify waiting consumers
  - Update statistics
  - Return success/failure
- [ ] Implement `dequeue()` method
  - Remove message from queue
  - Support timeout with async wait
  - Update statistics
- [ ] Add queue statistics tracking
  ```rust
  pub struct QueueStats {
      enqueued_total: AtomicU64,
      dequeued_total: AtomicU64,
      queue_full_count: AtomicU64,
  }
  ```
- [ ] Run tests: `cargo test` (all should pass when implementation complete)

#### Day 5-6: HTTP API
- [ ] Setup Axum server
- [ ] Implement `POST /queues/:queue_name/enqueue` handler
  - Extract queue name from path
  - Parse JSON body
  - Get or create queue
  - Call `queue.enqueue()`
  - Return 200 OK or 429 Too Many Requests
- [ ] Implement `GET /queues/:queue_name/dequeue` handler
  - Extract queue name from path
  - Parse `timeout` query param
  - Get queue (or 404 if not exists)
  - Call `queue.dequeue()`
  - Return message or 204 No Content
- [ ] Implement `GET /health` handler
  - Return all queues stats
  - Return server status
- [ ] Add error handling middleware
- [ ] Add request logging

#### Day 7: Testing
- [ ] Unit tests for queue operations
- [ ] Integration tests for HTTP endpoints
- [ ] Concurrent access tests
- [ ] Backpressure tests
- [ ] Timeout tests

---

### Phase 2: WebSocket & Performance (Week 2)

#### Day 8-9: WebSocket Implementation
- [ ] Add WebSocket support to Axum (built-in)
- [ ] Implement WebSocket upgrade handler
- [ ] Implement `WS /queues/:queue_name/enqueue` handler
  - Receive messages from WebSocket
  - Enqueue continuously
  - Handle backpressure
  - Send confirmations back
- [ ] Implement `WS /queues/:queue_name/dequeue` handler (PRIORITY)
  - Continuous message streaming
  - Wait for messages if queue empty
  - Send messages as they arrive
  - Handle slow consumers (disconnect if can't keep up)
  - Graceful backpressure
- [ ] Add connection management
- [ ] Add graceful disconnect handling
- [ ] Add ping/pong for connection health

#### Day 10-11: Performance Optimization
- [ ] Replace `std::sync` with `parking_lot` if needed
- [ ] Optimize message serialization
- [ ] Add batch enqueue/dequeue (optional)
- [ ] Memory allocator tuning (`mimalloc`)
- [ ] Profile with `perf` and `flamegraph`

#### Day 12-13: Benchmarking
- [ ] Setup `criterion` benchmarks
- [ ] Benchmark enqueue throughput
- [ ] Benchmark dequeue latency
- [ ] Benchmark concurrent access
- [ ] Compare with Redis (baseline)

#### Day 14: Configuration & Logging
- [ ] Add configuration file support (TOML)
- [ ] Add environment variable support
- [ ] Structured logging with `tracing`
- [ ] Log levels (debug, info, warn, error)
- [ ] Metrics endpoint (Prometheus format)

---

### Phase 3: Production Ready (Week 3)

#### Day 15-16: Observability
- [ ] Add Prometheus metrics
  - `zoldyq_queue_size` - Current queue size
  - `zoldyq_queue_capacity` - Max capacity
  - `zoldyq_enqueued_total` - Total enqueued
  - `zoldyq_dequeued_total` - Total dequeued
  - `zoldyq_queue_full_total` - Times queue was full
  - `zoldyq_dequeue_timeout_total` - Times dequeue timed out
  - `zoldyq_enqueue_duration_seconds` - Enqueue latency histogram
  - `zoldyq_dequeue_duration_seconds` - Dequeue latency histogram
- [ ] Add health check endpoint details
- [ ] Add graceful shutdown

#### Day 17-18: Documentation
- [ ] Complete README
  - Quick start
  - Docker instructions
  - API reference
  - Examples
- [ ] API documentation (OpenAPI/Swagger)
- [ ] Architecture documentation
- [ ] Performance benchmarks document
- [ ] Use cases and anti-patterns

#### Day 19-20: Deployment & Packaging
- [ ] Build Docker image
- [ ] Optimize Docker image size
- [ ] Create docker-compose.yml example
- [ ] Build release binaries (Linux, macOS, Windows)
- [ ] Create GitHub release
- [ ] Publish Docker image to Docker Hub
- [ ] Publish to crates.io

#### Day 21: Final Testing & Launch
- [ ] End-to-end testing
- [ ] Load testing (sustained load)
- [ ] Stress testing (queue full, timeouts)
- [ ] Memory leak testing
- [ ] Security audit (`cargo audit`)
- [ ] Tag v0.1.0
- [ ] Launch announcement

---

## API Specification

### REST API

#### POST /queues/{queue_name}/enqueue

**Description:** Add a message to a named queue

**Request:**
```http
POST /queues/emails/enqueue HTTP/1.1
Content-Type: application/json

{
  "payload": {
    "email": "user@example.com",
    "subject": "Welcome",
    "body": "Thanks for signing up!"
  }
}
```

**Response (Success):**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "queued": true,
  "queue_name": "emails",
  "queue_size": 1234,
  "enqueued_at": 1701234567890123
}
```

**Response (Queue Full):**
```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json

{
  "error": "queue_full",
  "message": "Queue 'emails' is at capacity",
  "queue_name": "emails",
  "queue_capacity": 100000
}
```

---

#### GET /queues/{queue_name}/dequeue

**Description:** Remove and return a message from a named queue

**Request:**
```http
GET /queues/emails/dequeue?timeout=5 HTTP/1.1
```

**Query Parameters:**
- `timeout` (optional) - Max seconds to wait for message (default: 0, max: 30)

**Response (Success):**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "payload": {
    "email": "user@example.com",
    "subject": "Welcome",
    "body": "Thanks for signing up!"
  },
  "queue_name": "emails",
  "enqueued_at": 1701234567890123,
  "queue_size": 1233
}
```

**Response (Empty Queue):**
```http
HTTP/1.1 204 No Content
```

**Response (Queue Not Found):**
```http
HTTP/1.1 404 Not Found
Content-Type: application/json

{
  "error": "queue_not_found",
  "message": "Queue 'emails' does not exist"
}
```

---

#### GET /health

**Description:** Get queue status and health

**Request:**
```http
GET /health HTTP/1.1
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "healthy",
  "uptime_seconds": 86400,
  "total_queues": 3,
  "queues": {
    "emails": {
      "size": 1234,
      "capacity": 100000,
      "utilization": 0.01234,
      "enqueued_total": 5678901,
      "dequeued_total": 5677667,
      "queue_full_count": 42
    },
    "webhooks": {
      "size": 56,
      "capacity": 100000,
      "utilization": 0.00056,
      "enqueued_total": 12345,
      "dequeued_total": 12289,
      "queue_full_count": 0
    }
  }
}
```

---

#### GET /metrics

**Description:** Prometheus metrics

**Request:**
```http
GET /metrics HTTP/1.1
```

**Response:**
```http
HTTP/1.1 200 OK
Content-Type: text/plain

# HELP zoldyq_queue_size Current queue size
# TYPE zoldyq_queue_size gauge
zoldyq_queue_size 1234

# HELP zoldyq_queue_capacity Maximum queue capacity
# TYPE zoldyq_queue_capacity gauge
zoldyq_queue_capacity 100000

# HELP zoldyq_enqueued_total Total messages enqueued
# TYPE zoldyq_enqueued_total counter
zoldyq_enqueued_total 5678901

# HELP zoldyq_dequeued_total Total messages dequeued
# TYPE zoldyq_dequeued_total counter
zoldyq_dequeued_total 5677667

# HELP zoldyq_queue_full_total Times queue was full
# TYPE zoldyq_queue_full_total counter
zoldyq_queue_full_total 42

# HELP zoldyq_enqueue_duration_seconds Enqueue operation latency
# TYPE zoldyq_enqueue_duration_seconds histogram
zoldyq_enqueue_duration_seconds_bucket{le="0.0001"} 9500
zoldyq_enqueue_duration_seconds_bucket{le="0.0005"} 9950
zoldyq_enqueue_duration_seconds_bucket{le="0.001"} 9990
```

---

### WebSocket API

#### WS /queues/{queue_name}/enqueue

**Description:** Stream messages to enqueue

**Protocol:**
```javascript
const ws = new WebSocket('ws://localhost:9000/queues/emails/enqueue');

// Send message
ws.send(JSON.stringify({
  payload: { email: "user@example.com" }
}));

// Receive confirmation
ws.onmessage = (event) => {
  const response = JSON.parse(event.data);
  // { "queued": true, "queue_name": "emails", "queue_size": 1235 }
};

// Handle errors
ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};
```

---

#### WS /queues/{queue_name}/dequeue

**Description:** Stream messages from queue continuously

**Protocol:**
```javascript
const ws = new WebSocket('ws://localhost:9000/queues/emails/dequeue');

// Receive messages continuously
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  // { "payload": {...}, "queue_name": "emails", "enqueued_at": 1701234567890123 }
  
  // Process message
  processEmail(message.payload);
};

// Connection stays open, messages stream as they arrive
ws.onclose = () => {
  console.log('Connection closed, reconnecting...');
  // Implement reconnection logic
};

// Handle errors
ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};
```

**Behavior:**
- Connection stays open indefinitely
- Messages sent immediately when available
- If queue is empty, waits for new messages
- Client can close connection anytime
- Server may close if client is too slow (backpressure)

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_dequeue() {
        let queue = MessageQueue::new(10);
        let msg = Message::new(json!({"test": "data"}));
        
        assert!(queue.enqueue(msg.clone()).is_ok());
        assert_eq!(queue.size(), 1);
        
        let dequeued = queue.dequeue(Duration::from_secs(0)).unwrap();
        assert_eq!(dequeued.payload, msg.payload);
        assert_eq!(queue.size(), 0);
    }

    #[test]
    fn test_queue_full() {
        let queue = MessageQueue::new(2);
        let msg = Message::new(json!({"test": "data"}));
        
        assert!(queue.enqueue(msg.clone()).is_ok());
        assert!(queue.enqueue(msg.clone()).is_ok());
        assert!(queue.enqueue(msg.clone()).is_err()); // Should fail
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let queue = Arc::new(MessageQueue::new(1000));
        
        // Spawn 10 producers
        let producers: Vec<_> = (0..10)
            .map(|i| {
                let queue = queue.clone();
                tokio::spawn(async move {
                    for j in 0..100 {
                        let msg = Message::new(json!({"id": i * 100 + j}));
                        queue.enqueue(msg).ok();
                    }
                })
            })
            .collect();
        
        // Wait for all producers
        for p in producers {
            p.await.unwrap();
        }
        
        // Verify count
        assert!(queue.stats.enqueued_total.load(Ordering::SeqCst) <= 1000);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_http_enqueue_dequeue() {
    let app = create_app();
    
    // Enqueue
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/enqueue")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"payload": {"test": "data"}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Dequeue
    let response = app
        .oneshot(
            Request::builder()
                .uri("/dequeue")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}
```

### Load Tests

```bash
# Enqueue load test
wrk -t4 -c100 -d30s --latency \
  -s scripts/enqueue.lua \
  http://localhost:9000/enqueue

# Dequeue load test
wrk -t4 -c100 -d30s --latency \
  http://localhost:9000/dequeue
```

---

## Performance Benchmarks

### Target Metrics

```
Throughput:
  - Enqueue:  500k-1M msg/sec
  - Dequeue:  500k-1M msg/sec
  - Combined: 500k msg/sec (concurrent)

Latency:
  - Enqueue P50:  <100μs
  - Enqueue P99:  <500μs
  - Dequeue P50:  <100μs
  - Dequeue P99:  <1ms

Memory:
  - Idle:         10-50MB
  - 100k msgs:    ~100MB
  - 1M msgs:      ~1GB

CPU:
  - Idle:         <1%
  - 500k msg/s:   30-50%
```

### Benchmark Suite

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_enqueue(c: &mut Criterion) {
    let queue = MessageQueue::new(100_000);
    let msg = Message::new(json!({"test": "data"}));
    
    c.bench_function("enqueue", |b| {
        b.iter(|| {
            queue.enqueue(black_box(msg.clone())).ok();
        });
    });
}

fn bench_dequeue(c: &mut Criterion) {
    let queue = MessageQueue::new(100_000);
    
    // Pre-fill queue
    for _ in 0..10_000 {
        queue.enqueue(Message::new(json!({"test": "data"}))).ok();
    }
    
    c.bench_function("dequeue", |b| {
        b.iter(|| {
            queue.dequeue(Duration::from_secs(0));
        });
    });
}

criterion_group!(benches, bench_enqueue, bench_dequeue);
criterion_main!(benches);
```

---

## How to Use

### Example 1: Email Sending Queue

**Producer (Web Server):**
```python
import requests

def signup_handler(email, name):
    # Save user to database
    user = create_user(email, name)
    
    # Queue welcome email (async)
    requests.post('http://localhost:9000/enqueue', json={
        'payload': {
            'type': 'welcome_email',
            'email': email,
            'name': name
        }
    })
    
    return {"status": "success"}
```

**Consumer (Email Worker):**
```python
import requests
import time
from send_email import send_welcome_email

while True:
    # Dequeue message (wait up to 5 seconds)
    response = requests.get('http://localhost:9000/dequeue?timeout=5')
    
    if response.status_code == 200:
        message = response.json()
        payload = message['payload']
        
        # Send email
        send_welcome_email(payload['email'], payload['name'])
        print(f"Sent email to {payload['email']}")
    elif response.status_code == 204:
        # No messages, wait and retry
        time.sleep(1)
```

---

### Example 2: Image Processing Pipeline

**Producer (Upload Handler):**
```javascript
const axios = require('axios');

async function handleUpload(imageUrl, userId) {
  // Queue image processing
  await axios.post('http://localhost:9000/enqueue', {
    payload: {
      type: 'process_image',
      imageUrl: imageUrl,
      userId: userId,
      operations: ['resize', 'compress', 'watermark']
    }
  });
  
  return { status: 'queued' };
}
```

**Consumer (Image Worker):**
```javascript
const axios = require('axios');
const { processImage } = require('./image-processor');

async function worker() {
  while (true) {
    try {
      const response = await axios.get('http://localhost:9000/dequeue?timeout=5');
      
      if (response.status === 200) {
        const { payload } = response.data;
        
        // Process image
        await processImage(payload.imageUrl, payload.operations);
        console.log(`Processed image for user ${payload.userId}`);
      }
    } catch (error) {
      if (error.response?.status !== 204) {
        console.error('Error:', error.message);
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
  }
}

worker();
```

---

### Example 3: Webhook Delivery

**Producer:**
```go
package main

import (
    "bytes"
    "encoding/json"
    "net/http"
)

func triggerWebhook(eventType string, data map[string]interface{}) error {
    payload := map[string]interface{}{
        "payload": map[string]interface{}{
            "event": eventType,
            "data": data,
            "webhookUrl": "https://customer.com/webhook",
        },
    }
    
    body, _ := json.Marshal(payload)
    _, err := http.Post("http://localhost:9000/enqueue", 
        "application/json", bytes.NewReader(body))
    
    return err
}
```

**Consumer:**
```go
package main

import (
    "encoding/json"
    "net/http"
    "time"
)

func webhookWorker() {
    client := &http.Client{Timeout: 10 * time.Second}
    
    for {
        // Dequeue message
        resp, err := client.Get("http://localhost:9000/dequeue?timeout=5")
        if err != nil {
            time.Sleep(1 * time.Second)
            continue
        }
        
        if resp.StatusCode == 200 {
            var message struct {
                Payload map[string]interface{} `json:"payload"`
            }
            json.NewDecoder(resp.Body).Decode(&message)
            resp.Body.Close()
            
            // Send webhook
            webhookUrl := message.Payload["webhookUrl"].(string)
            data, _ := json.Marshal(message.Payload["data"])
            http.Post(webhookUrl, "application/json", bytes.NewReader(data))
        }
        resp.Body.Close()
    }
}
```

---

### Example 4: Rate-Limited API Calls

**Producer:**
```ruby
require 'net/http'
require 'json'

def queue_api_call(endpoint, params)
  uri = URI('http://localhost:9000/enqueue')
  
  request = Net::HTTP::Post.new(uri, 'Content-Type' => 'application/json')
  request.body = {
    payload: {
      endpoint: endpoint,
      params: params,
      retry_count: 0
    }
  }.to_json
  
  Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(request) }
end
```

**Consumer (Rate Limited):**
```ruby
require 'net/http'
require 'json'

def rate_limited_worker
  loop do
    uri = URI('http://localhost:9000/dequeue?timeout=5')
    response = Net::HTTP.get_response(uri)
    
    if response.code == '200'
      message = JSON.parse(response.body)
      payload = message['payload']
      
      # Make API call
      make_external_api_call(payload['endpoint'], payload['params'])
      
      # Rate limit: 10 calls per second
      sleep 0.1
    else
      sleep 1
    end
  end
end
```

---

## Deployment Guide

### Docker Compose Example

```yaml
version: '3.8'

services:
  zoldyq:
    image: zoldyq/zoldyq:latest
    ports:
      - "9000:9000"
    environment:
      - ZOLDYQ_QUEUE_CAPACITY=100000
      - ZOLDYQ_LOG_LEVEL=info
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9000/health"]
      interval: 10s
      timeout: 5s
      retries: 3

  producer:
    build: ./producer
    depends_on:
      - zoldyq
    environment:
      - QUEUE_URL=http://zoldyq:9000

  consumer:
    build: ./consumer
    depends_on:
      - zoldyq
    environment:
      - QUEUE_URL=http://zoldyq:9000
    deploy:
      replicas: 3  # Scale consumers
```

### Kubernetes Example

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zoldyq
spec:
  replicas: 1  # Single instance (in-memory, not distributed)
  selector:
    matchLabels:
      app: zoldyq
  template:
    metadata:
      labels:
        app: zoldyq
    spec:
      containers:
      - name: zoldyq
        image: zoldyq/zoldyq:latest
        ports:
        - containerPort: 9000
        env:
        - name: ZOLDYQ_QUEUE_CAPACITY
          value: "100000"
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 9000
          initialDelaySeconds: 5
          periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: zoldyq
spec:
  selector:
    app: zoldyq
  ports:
  - port: 9000
    targetPort: 9000
```

### Monitoring with Prometheus

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'zoldyq'
    static_configs:
      - targets: ['localhost:9000']
    metrics_path: '/metrics'
    scrape_interval: 5s
```

### Grafana Dashboard

```json
{
  "dashboard": {
    "title": "ZoldyQ Metrics",
    "panels": [
      {
        "title": "Queue Size",
        "targets": [{"expr": "zoldyq_queue_size"}]
      },
      {
        "title": "Throughput",
        "targets": [
          {"expr": "rate(zoldyq_enqueued_total[1m])", "legendFormat": "Enqueue"},
          {"expr": "rate(zoldyq_dequeued_total[1m])", "legendFormat": "Dequeue"}
        ]
      },
      {
        "title": "Latency P99",
        "targets": [
          {"expr": "histogram_quantile(0.99, zoldyq_enqueue_duration_seconds)"}
        ]
      }
    ]
  }
}
```

---

## Configuration

### Environment Variables

```bash
# Server
ZOLDYQ_HOST=0.0.0.0                   # Bind address
ZOLDYQ_PORT=9000                      # HTTP port

# Queues
ZOLDYQ_DEFAULT_QUEUE_CAPACITY=100000  # Max messages per queue
ZOLDYQ_MAX_QUEUES=100                 # Max number of queues

# Timeouts
ZOLDYQ_DEQUEUE_MAX_TIMEOUT=30         # Max dequeue timeout (seconds)

# Logging
ZOLDYQ_LOG_LEVEL=info                 # debug|info|warn|error
ZOLDYQ_LOG_FORMAT=json                # json|pretty
```

### TOML Configuration

```toml
# zoldyq.toml
[server]
host = "0.0.0.0"
port = 9000

[queues]
default_capacity = 100000
max_queues = 100

[timeouts]
dequeue_max = 30  # seconds

[logging]
level = "info"
format = "json"
```

---

## Success Criteria

### Must Have (v0.1.0)

- [ ] Multiple named queues (queue manager)
- [ ] In-memory FIFO per queue
- [ ] HTTP enqueue/dequeue endpoints with queue names
- [ ] **WebSocket dequeue for continuous consumption**
- [ ] Backpressure handling (429 when full)
- [ ] Dequeue with timeout
- [ ] Health check endpoint (all queues)
- [ ] Prometheus metrics
- [ ] Docker image
- [ ] Documentation
- [ ] Basic tests

### Should Have

- [ ] WebSocket enqueue support
- [ ] Graceful shutdown
- [ ] Configurable capacity per queue
- [ ] Max queues limit
- [ ] Structured logging
- [ ] Load test validation

### Nice to Have

- [ ] Batch operations
- [ ] Queue deletion/management API
- [ ] Admin UI
- [ ] Queue-specific capacity configuration

---

## Non-Goals

**Explicitly out of scope:**

- Persistence/durability
- Distributed architecture
- High availability
- Replication
- Consumer groups
- Message replay
- Exactly-once delivery
- Complex routing
- Message TTL/expiry
- Dead letter queues

---

## Timeline

```
Week 1: Core Queue + HTTP API
Week 2: WebSocket + Performance
Week 3: Production Ready + Launch
```

---

## License

Apache 2.0 or MIT (dual-licensed)
