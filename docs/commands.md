# Command Reference

ZoldyQ implements a subset of Redis commands plus custom extensions.

## Queue Operations

### LPUSH queue value [value ...]

Push one or more values to the head of a queue (enqueue).

```bash
LPUSH tasks '{"job": "process", "id": 1}'
# (integer) 1

LPUSH tasks '{"job": "send"}' '{"job": "cleanup"}'
# (integer) 2
```

### RPOP queue

Pop and return a value from the tail of a queue (dequeue, non-blocking).

```bash
RPOP tasks
# {"job": "process", "id": 1}

RPOP empty_queue
# (nil)
```

**With ACK mode enabled**, returns `[message_id, payload]`:
```bash
RPOP tasks
# 1) "550e8400-e29b-41d4-a716-446655440000"
# 2) {"job": "process"}
```

### BRPOP queue timeout

Blocking pop - waits up to `timeout` seconds for a message.

```bash
BRPOP tasks 30
# 1) "tasks"
# 2) {"job": "process"}

BRPOP tasks 0  # Wait forever
```

**With ACK mode**, returns `[queue, message_id, payload]`.

### LLEN queue

Get the number of messages in a queue.

```bash
LLEN tasks
# (integer) 5
```

### DEL queue [queue ...]

Delete one or more queues.

```bash
DEL tasks events
# (integer) 2
```

## Message Acknowledgment

### ACK message_id

Acknowledge successful processing of a message (requires ACK mode).

```bash
ACK 550e8400-e29b-41d4-a716-446655440000
# OK
```

### NACK message_id

Reject a message, returning it to the queue or DLQ (requires ACK mode).

```bash
NACK 550e8400-e29b-41d4-a716-446655440000
# OK
```

## Persistence

### BGSAVE

Trigger a background snapshot save.

```bash
BGSAVE
# Background saving started
```

### LASTSAVE

Get Unix timestamp of last successful snapshot.

```bash
LASTSAVE
# (integer) 1704067200
```

## Connection & Server

### PING [message]

Test connection, optionally echo a message.

```bash
PING
# PONG

PING hello
# hello
```

### AUTH password

Authenticate to the server (if password is configured).

```bash
AUTH mysecretpassword
# OK
```

### HEALTH

Check server health.

```bash
HEALTH
# OK
```

### INFO [section]

Get server information. Sections: `server`, `queues`, `persistence`, `memory`.

```bash
INFO server
# # Server
# zoldyq_version:0.1.0
# uptime_in_seconds:3600
# ...
```

### COMMAND

List supported commands.

```bash
COMMAND
# 1) PING
# 2) AUTH
# 3) INFO
# ...
```

### QUIT

Close the connection.

```bash
QUIT
# OK
```

## Error Responses

| Error | Description |
|-------|-------------|
| `NOAUTH Authentication required.` | Must authenticate first |
| `WRONGPASS invalid password` | Invalid AUTH password |
| `ERR unknown command` | Command not supported |
| `ERR wrong number of arguments` | Invalid argument count |
| `ERR ACK not enabled` | ACK command requires ACK mode |
| `ERR persistence not enabled` | BGSAVE requires persistence config |
