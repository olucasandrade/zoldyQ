# Configuration Reference

ZoldyQ is configured via environment variables.

## Core Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_HOST` | `0.0.0.0` | Host to bind RESP server |
| `ZOLDYQ_PORT` | `6379` | Port for RESP server |
| `ZOLDYQ_HTTP_PORT` | `8080` | Port for HTTP health server |
| `ZOLDYQ_QUEUE_CAPACITY` | `100000` | Max messages per queue |
| `ZOLDYQ_MAX_QUEUES` | `1000` | Max number of queues |

## Authentication

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_PASSWORD` | (none) | Password for AUTH command (unset = no auth) |

## Message Acknowledgment

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_ACK_ENABLED` | `false` | Enable ACK/NACK message acknowledgment |
| `ZOLDYQ_VISIBILITY_TIMEOUT` | `30` | Seconds before unacked message returns to queue |
| `ZOLDYQ_MAX_RETRIES` | `3` | Max delivery attempts before moving to DLQ |

## Persistence

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_SNAPSHOT_PATH` | `./data/snapshot.rdb` | Path to snapshot file |
| `ZOLDYQ_SNAPSHOT_INTERVAL` | `0` | Seconds between automatic saves (0 = disabled) |
| `ZOLDYQ_SNAPSHOT_ON_SHUTDOWN` | `true` | Save snapshot on graceful shutdown |

## Example Configurations

### Development

```bash
export ZOLDYQ_PORT=6380
./target/debug/zoldyq
```

### Production (minimal)

```bash
export ZOLDYQ_PASSWORD=secure-password-here
export ZOLDYQ_QUEUE_CAPACITY=1000000
./target/release/zoldyq
```

### Production (with ACK and persistence)

```bash
export ZOLDYQ_PASSWORD=secure-password-here
export ZOLDYQ_ACK_ENABLED=true
export ZOLDYQ_VISIBILITY_TIMEOUT=60
export ZOLDYQ_MAX_RETRIES=5
export ZOLDYQ_SNAPSHOT_PATH=/data/zoldyq/snapshot.rdb
export ZOLDYQ_SNAPSHOT_INTERVAL=300
export ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=true
./target/release/zoldyq
```

### Docker

```bash
docker run -d \
  -p 6379:6379 \
  -p 8080:8080 \
  -e ZOLDYQ_PASSWORD=secret \
  -e ZOLDYQ_ACK_ENABLED=true \
  -v zoldyq-data:/data \
  -e ZOLDYQ_SNAPSHOT_PATH=/data/snapshot.rdb \
  zoldyq:latest
```
