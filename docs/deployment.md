# Deployment Guide

## Building from Source

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release
```

## Docker

### Dockerfile

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/zoldyq /usr/local/bin/
EXPOSE 6379 8080
CMD ["zoldyq"]
```

### Build and Run

```bash
docker build -t zoldyq:latest .
docker run -d -p 6379:6379 -p 8080:8080 zoldyq:latest
```

## Kubernetes

### Basic Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zoldyq
spec:
  replicas: 1
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
        image: zoldyq:latest
        ports:
        - containerPort: 6379
          name: resp
        - containerPort: 8080
          name: http
        env:
        - name: ZOLDYQ_PASSWORD
          valueFrom:
            secretKeyRef:
              name: zoldyq-secret
              key: password
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        resources:
          requests:
            memory: "64Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: zoldyq
spec:
  selector:
    app: zoldyq
  ports:
  - name: resp
    port: 6379
    targetPort: 6379
  - name: http
    port: 8080
    targetPort: 8080
```

### With Persistence

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

## AWS ECS

### Task Definition

```json
{
  "family": "zoldyq",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "256",
  "memory": "512",
  "containerDefinitions": [
    {
      "name": "zoldyq",
      "image": "your-ecr-repo/zoldyq:latest",
      "portMappings": [
        {"containerPort": 6379, "protocol": "tcp"},
        {"containerPort": 8080, "protocol": "tcp"}
      ],
      "environment": [
        {"name": "ZOLDYQ_ACK_ENABLED", "value": "true"}
      ],
      "secrets": [
        {
          "name": "ZOLDYQ_PASSWORD",
          "valueFrom": "arn:aws:secretsmanager:region:account:secret:zoldyq-password"
        }
      ],
      "healthCheck": {
        "command": ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"],
        "interval": 30,
        "timeout": 5,
        "retries": 3
      },
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/zoldyq",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      }
    }
  ]
}
```

## systemd Service

```ini
[Unit]
Description=ZoldyQ Message Queue
After=network.target

[Service]
Type=simple
User=zoldyq
Group=zoldyq
Environment=ZOLDYQ_PORT=6379
Environment=ZOLDYQ_HTTP_PORT=8080
Environment=ZOLDYQ_SNAPSHOT_PATH=/var/lib/zoldyq/snapshot.rdb
Environment=ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=true
ExecStart=/usr/local/bin/zoldyq
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Security Considerations

1. **Network isolation**: Run ZoldyQ in a private subnet
2. **TLS termination**: Use a load balancer or sidecar for TLS
3. **Authentication**: Always set `ZOLDYQ_PASSWORD` in production
4. **Resource limits**: Configure memory limits to prevent OOM
5. **Monitoring**: Use HTTP health endpoints for alerting
