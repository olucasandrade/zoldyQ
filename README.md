# ZoldyQ 🚀

**Ultra-fast, minimal, in-memory FIFO message queue with Redis protocol (RESP) support**

ZoldyQ is a blazing-fast message queue designed for simplicity and performance. It implements the Redis protocol (RESP), making it compatible with any Redis client while maintaining sub-millisecond latency.

## ✨ Features

- **🚀 Ultra-Fast**: Sub-50 microsecond latency (~0.05ms)
- **📦 Simple**: Only queue operations - no complex features
- **🔌 Redis Compatible**: Works with ANY Redis client
- **💾 In-Memory**: Zero persistence overhead
- **🔒 Thread-Safe**: Concurrent access with lock-free queues
- **⚡ High Throughput**: ~20,000 messages/second
- **📊 Bounded Queues**: Configurable capacity with backpressure

## 🎯 Use Cases

Perfect for:
- High-frequency task queues
- Real-time event processing
- Microservice communication
- Job scheduling
- Request buffering

**Not for:**
- Message persistence (use Redis or RabbitMQ)
- Complex routing (use RabbitMQ)
- Distributed systems (use Kafka)

## 🚀 Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/zoldyq.git
cd zoldyq

# Build release binary
cargo build --release

# Run the server
./target/release/zoldyq
```

### Configuration

Configure via environment variables:

```bash
export ZOLDYQ_HOST="0.0.0.0"           # Bind address (default: 0.0.0.0)
export ZOLDYQ_PORT="6379"              # RESP port (default: 6379)
export ZOLDYQ_QUEUE_CAPACITY="100000"  # Max messages per queue (default: 100,000)
export ZOLDYQ_MAX_QUEUES="1000"        # Max number of queues (default: 1,000)
```

## 📚 Usage

### Python

```python
import redis

# Connect to ZoldyQ (just like Redis!)
r = redis.Redis(host='localhost', port=6379)

# Enqueue messages
r.lpush('tasks', 'task1', 'task2', 'task3')

# Dequeue (non-blocking)
task = r.rpop('tasks')

# Dequeue (blocking, waits up to 30 seconds)
result = r.brpop('tasks', timeout=30)

# Get queue size
length = r.llen('tasks')

# Delete queue
r.delete('tasks')
```

### Node.js

```javascript
const redis = require('redis');

const client = redis.createClient({
  host: 'localhost',
  port: 6379
});

// Enqueue
await client.lPush('tasks', 'task1');

// Dequeue
const task = await client.rPop('tasks');

// Blocking dequeue
const result = await client.brPop('tasks', 30);
```

### Rust

```rust
use redis::Commands;

let client = redis::Client::open("redis://127.0.0.1:6379/")?;
let mut con = client.get_connection()?;

// Enqueue
con.lpush("tasks", "task1")?;

// Dequeue
let task: Option<String> = con.rpop("tasks")?;

// Blocking dequeue
let result: Option<(String, String)> = con.brpop("tasks", 30)?;
```

### Command Line (redis-cli)

```bash
# Connect
redis-cli -p 6379

# Enqueue
LPUSH tasks "task1" "task2" "task3"

# Dequeue
RPOP tasks

# Queue size
LLEN tasks
```

## 🔌 Supported Commands

| Command | Description | Example |
|---------|-------------|---------|
| `PING` | Test connection | `PING` → `PONG` |
| `LPUSH` | Enqueue messages | `LPUSH queue msg1 msg2` |
| `RPOP` | Dequeue (non-blocking) | `RPOP queue` |
| `LLEN` | Get queue size | `LLEN queue` |
| `DEL` | Delete queues | `DEL queue1 queue2` |

## 📊 Performance

**Key Advantages:**
- ✅ **Simpler than Redis** (only queue operations)
- ✅ **Faster than RabbitMQ** (simpler protocol)
- ✅ **Pure in-memory** (no disk overhead)
- ✅ **Single binary** (easy deployment)

### Run Benchmarks

```bash
# Internal Rust benchmarks
cargo bench

# Comparison benchmarks (requires Docker)
cd benchmarks
./run_all.sh
```

## 🏗️ Architecture

```
┌──────────────────────────────────┐
│       RESP Server (Port 6379)    │
│  ┌────────────────────────────┐  │
│  │  RESP Parser               │  │
│  │  Command Handler           │  │
│  │  RESP Serializer           │  │
│  └────────────────────────────┘  │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│        Queue Manager             │
│  ┌────────────────────────────┐  │
│  │  DashMap<String, Queue>    │  │
│  └────────────────────────────┘  │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│        Message Queue             │
│  ┌────────────────────────────┐  │
│  │  ArrayQueue (Lock-free)    │  │
│  │  Tokio::Notify (Async)     │  │
│  └────────────────────────────┘  │
└──────────────────────────────────┘
```

## 🔧 Development

### Build

```bash
cargo build --release
```

### Test

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# All tests
cargo test --all
```

### Run

```bash
# Development mode
cargo run

# Release mode
cargo run --release

# With custom config
ZOLDYQ_PORT=7000 cargo run --release
```

## 📖 Documentation

- **[benchmarks/README.md](benchmarks/README.md)** - Benchmarking guide
- **[CHANGELOG.md](CHANGELOG.md)** - Version history

## 🐳 Docker

```bash
# Build image
docker build -t zoldyq .

# Run container
docker run -p 6379:6379 zoldyq

# With environment variables
docker run -p 6379:6379 -e ZOLDYQ_QUEUE_CAPACITY=50000 zoldyq
```

## 🤝 Contributing

Contributions welcome! Please:

1. Keep it minimal - no feature creep
2. Maintain sub-millisecond latency
3. Add tests for new features
4. Update documentation

## 📄 License

MIT License - see LICENSE file

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs/) async runtime
- Uses [crossbeam](https://github.com/crossbeam-rs/crossbeam) lock-free queues
- Implements [RESP](https://redis.io/docs/reference/protocol-spec/) (Redis Protocol)

## 🔗 Links

- **GitHub**: https://github.com/yourusername/zoldyq
- **Issues**: https://github.com/yourusername/zoldyq/issues
- **RESP Spec**: https://redis.io/docs/reference/protocol-spec/

---

**Made with ⚡ by the ZoldyQ team**

**ZoldyQ: Redis-compatible, in-memory message queue - Simple, Fast, Minimal**
