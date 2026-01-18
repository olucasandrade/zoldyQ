# Monitoring

ZoldyQ provides multiple endpoints for monitoring health and metrics.

## HTTP Health Endpoints

HTTP server runs on port 8080 by default (configurable via `ZOLDYQ_HTTP_PORT`).

### GET /health

Liveness probe - returns 200 if the process is running.

```bash
curl http://localhost:8080/health
```

Response:
```json
{"status":"ok"}
```

### GET /ready

Readiness probe - returns 200 if the server can accept traffic.

```bash
curl http://localhost:8080/ready
```

Response:
```json
{"status":"ready"}
```

### GET /metrics

Returns server metrics in JSON format.

```bash
curl http://localhost:8080/metrics
```

Response:
```json
{
  "connected_clients": 5,
  "total_connections": 150,
  "total_commands": 10500,
  "queue_count": 3
}
```

## RESP Commands

### HEALTH

Test server health via RESP protocol.

```bash
redis-cli -p 6379 HEALTH
```

Response: `OK`

### INFO

Get detailed server information.

```bash
redis-cli -p 6379 INFO
```

Returns:
```
# Server
zoldyq_version:0.1.0
uptime_in_seconds:3600
process_id:12345

# Queues
queue_count:3
max_queues:1000
default_capacity:100000
queue_tasks:size=50,enqueued=1000,dequeued=950
queue_events:size=0,enqueued=500,dequeued=500

# Memory
used_memory:0
```

Filter by section:
```bash
redis-cli -p 6379 INFO server
redis-cli -p 6379 INFO queues
redis-cli -p 6379 INFO memory
```

## Kubernetes Integration

### Liveness Probe

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
```

### Readiness Probe

```yaml
readinessProbe:
  httpGet:
    path: /ready
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 5
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_HTTP_PORT` | 8080 | HTTP health server port |

## Metrics Reference

| Metric | Type | Description |
|--------|------|-------------|
| `connected_clients` | Gauge | Current number of connected clients |
| `total_connections` | Counter | Total connections since startup |
| `total_commands` | Counter | Total commands processed |
| `queue_count` | Gauge | Number of active queues |
