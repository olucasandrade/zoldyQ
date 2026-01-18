# ZoldyQ 🚀

**Ultra-fast, minimal, in-memory FIFO message queue with Redis protocol (RESP) support**

ZoldyQ is a blazing-fast message queue designed for simplicity and performance. It implements the Redis protocol (RESP), making it compatible with any Redis client while maintaining sub-millisecond latency.

## Features

- ⚡ **Ultra-fast**: Lock-free data structures, sub-millisecond latency
- 🔌 **Redis compatible**: Works with any Redis client
- 🔐 **Authentication**: Optional password-based auth
- ✅ **Message acknowledgment**: ACK/NACK with visibility timeout
- 💀 **Dead Letter Queue**: Automatic DLQ routing for failed messages
- 💾 **Persistence**: Snapshot-based persistence with automatic recovery
- 🏥 **Health checks**: HTTP `/health` and `/ready` endpoints
- 📊 **Monitoring**: `INFO` command with detailed statistics
- 🛑 **Graceful shutdown**: Clean shutdown with optional snapshot save

## Quick Start

```bash
# Build
cargo build --release

# Run
./target/release/zoldyq

# Connect with any Redis client
redis-cli -p 6379

# Enqueue
LPUSH tasks '{"job": "process-image", "id": 123}'

# Dequeue (blocking)
BRPOP tasks 30
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        ZoldyQ Server                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────┐    ┌──────────┐    ┌──────────────────┐       │
│  │   RESP   │    │   HTTP   │    │                  │       │
│  │  Server  │    │  Health  │    │  Queue Manager   │       │
│  │  :6379   │    │  :8080   │    │                  │       │
│  └────┬─────┘    └────┬─────┘    └────────┬─────────┘       │
│       │               │                    │                 │
│       └───────────────┴────────────────────┘                 │
│                       │                                      │
│       ┌───────────────┼───────────────┐                      │
│       │               │               │                      │
│  ┌────▼────┐    ┌────▼────┐    ┌────▼────┐                  │
│  │  Queue  │    │  Queue  │    │   ...   │                  │
│  │ "tasks" │    │"events" │    │         │                  │
│  └─────────┘    └─────────┘    └─────────┘                  │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐                       │
│  │ ACK Manager  │    │  Snapshot    │                       │
│  │ (optional)   │    │  Manager     │                       │
│  └──────────────┘    └──────────────┘                       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Supported Commands

| Command | Description |
|---------|-------------|
| `PING [message]` | Test connection |
| `AUTH password` | Authenticate |
| `LPUSH queue value [value ...]` | Enqueue messages |
| `RPOP queue` | Dequeue (non-blocking) |
| `BRPOP queue timeout` | Dequeue (blocking) |
| `LLEN queue` | Get queue length |
| `DEL queue [queue ...]` | Delete queues |
| `ACK message_id` | Acknowledge message (ACK mode) |
| `NACK message_id` | Reject message (ACK mode) |
| `BGSAVE` | Trigger snapshot save |
| `LASTSAVE` | Get last save timestamp |
| `INFO [section]` | Server statistics |
| `HEALTH` | Health check |
| `COMMAND` | List commands |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_HOST` | `0.0.0.0` | Bind host |
| `ZOLDYQ_PORT` | `6379` | RESP port |
| `ZOLDYQ_HTTP_PORT` | `8080` | HTTP health port |
| `ZOLDYQ_PASSWORD` | (none) | Auth password |
| `ZOLDYQ_QUEUE_CAPACITY` | `100000` | Max messages per queue |
| `ZOLDYQ_MAX_QUEUES` | `1000` | Max queues |
| `ZOLDYQ_ACK_ENABLED` | `false` | Enable ACK mode |
| `ZOLDYQ_VISIBILITY_TIMEOUT` | `30` | ACK timeout (seconds) |
| `ZOLDYQ_MAX_RETRIES` | `3` | Max retries before DLQ |
| `ZOLDYQ_SNAPSHOT_PATH` | `./data/snapshot.rdb` | Snapshot file path |
| `ZOLDYQ_SNAPSHOT_INTERVAL` | `0` | Auto-save interval (0=disabled) |
| `ZOLDYQ_SNAPSHOT_ON_SHUTDOWN` | `true` | Save on shutdown |

## Example: Production Setup

```bash
export ZOLDYQ_PASSWORD=secure-password
export ZOLDYQ_ACK_ENABLED=true
export ZOLDYQ_VISIBILITY_TIMEOUT=60
export ZOLDYQ_MAX_RETRIES=5
export ZOLDYQ_SNAPSHOT_PATH=/data/snapshot.rdb
export ZOLDYQ_SNAPSHOT_INTERVAL=300
./target/release/zoldyq
```

## HTTP Health Endpoints

- `GET /health` - Liveness probe
- `GET /ready` - Readiness probe
- `GET /metrics` - Basic metrics (JSON)

## Use Cases

**Good for:**
- Background job queues
- Task distribution
- Event buffering
- Microservice communication

**Not for:**
- Complex routing (use RabbitMQ)
- Stream processing (use Kafka)
- Pub/Sub fan-out (use Redis)
- Distributed clustering (single-node only)

## Documentation

- [Configuration](docs/configuration.md)
- [Commands](docs/commands.md)
- [Authentication](docs/authentication.md)
- [Message ACK & DLQ](docs/ack-and-dlq.md)
- [Persistence](docs/persistence.md)
- [Monitoring](docs/monitoring.md)
- [Deployment](docs/deployment.md)

## Benchmarks

```bash
# Run benchmarks
cargo bench

# Python comparison benchmarks
python benchmarks/compare_redis.py
python benchmarks/ack_overhead.py
```

## Testing

```bash
cargo test
```

## License

MIT
