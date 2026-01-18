# Message Acknowledgment & Dead Letter Queues

ZoldyQ supports message acknowledgment with visibility timeout and automatic dead letter queue (DLQ) routing.

## Enabling ACK Mode

Set the `ZOLDYQ_ACK_ENABLED` environment variable to enable message acknowledgment:

```bash
export ZOLDYQ_ACK_ENABLED=true
export ZOLDYQ_VISIBILITY_TIMEOUT=30  # seconds (default: 30)
export ZOLDYQ_MAX_RETRIES=3          # default: 3
./zoldyq
```

## How It Works

When ACK mode is enabled:

1. **Dequeue** (`RPOP`/`BRPOP`) returns both message ID and payload
2. Message enters "in-flight" state
3. Consumer must **ACK** the message within the visibility timeout
4. If not ACKed in time, message automatically returns to queue
5. After `max_retries` failed deliveries, message moves to Dead Letter Queue

```
┌─────────────────────────────────────────────────────────────┐
│                     Message Lifecycle                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Queue          In-Flight              DLQ                   │
│  ┌────┐         ┌─────────┐           ┌────────────┐        │
│  │ M1 │ ─RPOP─► │ M1      │ ─ACK────► (deleted)             │
│  │ M2 │         │         │                                  │
│  │ M3 │         │         │ ─NACK───► ┌────┐ (if retries    │
│  └────┘         │         │           │ M1 │  exceeded)      │
│                 │         │           └────┘                 │
│                 │         │ ─timeout─► back to Queue         │
│                 └─────────┘           (if retries left)      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Response Format (ACK Mode)

When ACK mode is enabled, responses include the message ID:

### RPOP

```bash
# Without ACK mode:
> RPOP tasks
"payload"

# With ACK mode:
> RPOP tasks
1) "550e8400-e29b-41d4-a716-446655440000"  # message ID
2) "payload"
```

### BRPOP

```bash
# Without ACK mode:
> BRPOP tasks 30
1) "tasks"
2) "payload"

# With ACK mode:
> BRPOP tasks 30
1) "tasks"
2) "550e8400-e29b-41d4-a716-446655440000"  # message ID
3) "payload"
```

## Commands

### ACK message_id

Acknowledge successful processing. Removes message from in-flight state.

```bash
> ACK 550e8400-e29b-41d4-a716-446655440000
OK
```

### NACK message_id

Reject message. Returns it to the queue (or DLQ if max retries exceeded).

```bash
> NACK 550e8400-e29b-41d4-a716-446655440000
OK
```

## Dead Letter Queue

Messages that fail processing multiple times are moved to a DLQ named `{queue_name}:dlq`.

```
Original Queue: tasks
DLQ: tasks:dlq
```

### Monitoring DLQ

```bash
# Check DLQ size
redis-cli -p 6379 LLEN tasks:dlq

# Process DLQ messages (same as regular queue)
redis-cli -p 6379 RPOP tasks:dlq
```

## Example: Python Consumer with ACK

```python
import redis

r = redis.Redis(host='localhost', port=6379)

while True:
    result = r.brpop('tasks', timeout=30)
    if result:
        queue, msg_id, payload = result  # ACK mode returns 3 elements
        
        try:
            process(payload)
            r.execute_command('ACK', msg_id)
        except Exception as e:
            print(f"Processing failed: {e}")
            r.execute_command('NACK', msg_id)
```

## Configuration Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_ACK_ENABLED` | false | Enable message acknowledgment |
| `ZOLDYQ_VISIBILITY_TIMEOUT` | 30 | Seconds before unacked message returns to queue |
| `ZOLDYQ_MAX_RETRIES` | 3 | Max delivery attempts before moving to DLQ |

## Message TTL

Messages can have an expiration time. Expired messages are automatically removed.

### Per-Queue Default (Not Yet Implemented)

```bash
export ZOLDYQ_MESSAGE_TTL=3600  # 1 hour
```

### Per-Message TTL (Not Yet Implemented)

```bash
LPUSH tasks '{"data": 1}' EX 60  # Expires in 60 seconds
```

## Notes

- Messages in-flight are not lost on server restart (when persistence is enabled)
- DLQ messages retain original payload
- ACK/NACK commands fail if ACK mode is not enabled
- Message ID is a UUID v4
