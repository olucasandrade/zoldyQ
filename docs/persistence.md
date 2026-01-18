# Persistence (Snapshotting)

ZoldyQ supports periodic snapshots of queue state to disk for data recovery after restarts.

## Enabling Persistence

```bash
export ZOLDYQ_SNAPSHOT_PATH=./data/snapshot.rdb
export ZOLDYQ_SNAPSHOT_INTERVAL=300  # seconds (0 to disable periodic saves)
export ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=true
./zoldyq
```

## How It Works

1. **Periodic Saves**: If `ZOLDYQ_SNAPSHOT_INTERVAL > 0`, snapshots are saved automatically
2. **Shutdown Save**: If `ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=true`, a final snapshot is saved on graceful shutdown
3. **Startup Load**: On startup, if a snapshot exists, it's loaded to restore queue state
4. **Atomic Writes**: Snapshots are written to a temp file first, then renamed for crash safety

## Snapshot Format

```
Header:
  - Magic bytes: "ZQSN" (4 bytes)
  - Version: u32 (4 bytes)
  - CRC32 Checksum (4 bytes)
  
Body:
  - Bincode serialized snapshot data
```

## Commands

### BGSAVE

Trigger a background snapshot save.

```bash
redis-cli -p 6379 BGSAVE
# Background saving started
```

### LASTSAVE

Get Unix timestamp of last successful save.

```bash
redis-cli -p 6379 LASTSAVE
# (integer) 1704067200
```

### INFO persistence

Get persistence status.

```bash
redis-cli -p 6379 INFO persistence
# # Persistence
# persistence_enabled:true
# last_save:1704067200
```

## Configuration Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `ZOLDYQ_SNAPSHOT_PATH` | `./data/snapshot.rdb` | Path to snapshot file |
| `ZOLDYQ_SNAPSHOT_INTERVAL` | `0` | Seconds between automatic saves (0 = disabled) |
| `ZOLDYQ_SNAPSHOT_ON_SHUTDOWN` | `true` | Save snapshot on graceful shutdown |

## Data Recovery

On startup, ZoldyQ automatically:

1. Checks if snapshot file exists at `ZOLDYQ_SNAPSHOT_PATH`
2. Validates magic bytes and checksum
3. Deserializes and restores all messages to their queues
4. Skips expired messages (if TTL was set)

## Limitations

- **Point-in-time**: Only the last snapshot is kept
- **No AOF**: Unlike Redis, there's no append-only file option
- **In-flight messages**: Messages being processed (ACK pending) are NOT persisted
- **Memory**: Entire snapshot is loaded into memory during save/load

## Best Practices

1. **Set appropriate intervals**: Balance between data safety and I/O overhead
2. **Use SSD storage**: Faster writes reduce snapshot time
3. **Monitor disk space**: Ensure enough space for snapshot + temp file
4. **Test recovery**: Periodically verify snapshot recovery works

## Example: Kubernetes with Persistence

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: zoldyq-data
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zoldyq
spec:
  replicas: 1
  template:
    spec:
      containers:
      - name: zoldyq
        image: zoldyq:latest
        env:
        - name: ZOLDYQ_SNAPSHOT_PATH
          value: /data/snapshot.rdb
        - name: ZOLDYQ_SNAPSHOT_INTERVAL
          value: "300"
        - name: ZOLDYQ_SNAPSHOT_ON_SHUTDOWN
          value: "true"
        volumeMounts:
        - name: data
          mountPath: /data
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: zoldyq-data
```
