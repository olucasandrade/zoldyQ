# ZoldyQ Production Deployment Guide

This guide covers deploying ZoldyQ in production environments using AWS as the reference platform. Each use case includes architecture, configuration, monitoring, and operational procedures.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Use Case 1: Background Job Queue](#use-case-1-background-job-queue)
3. [Use Case 2: Event Buffer](#use-case-2-event-buffer)
4. [Use Case 3: Microservice Communication](#use-case-3-microservice-communication)
5. [Use Case 4: Development/Staging Environment](#use-case-4-developmentstaging-environment)
6. [Monitoring & Alerting](#monitoring--alerting)
7. [Backup & Recovery](#backup--recovery)
8. [Security Considerations](#security-considerations)
9. [Cost Estimation](#cost-estimation)

---

## Prerequisites

### Build the Binary

```bash
# On your build machine
git clone https://github.com/your-org/zoldyq.git
cd zoldyq
cargo build --release

# Binary location
ls -la target/release/zoldyq
```

### Create Docker Image

```dockerfile
# Dockerfile
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY target/release/zoldyq /usr/local/bin/zoldyq

EXPOSE 6379 6380 8080

ENTRYPOINT ["/usr/local/bin/zoldyq"]
```

```bash
docker build -t zoldyq:latest .
docker tag zoldyq:latest YOUR_AWS_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/zoldyq:latest
```

### Push to ECR

```bash
aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin YOUR_AWS_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com
docker push YOUR_AWS_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/zoldyq:latest
```

---

## Use Case 1: Background Job Queue

**Scenario**: Web application offloads tasks like image processing, email sending, or report generation to background workers.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         AWS VPC                                  │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────────┐   │
│  │  ALB        │     │  ECS        │     │  ECS            │   │
│  │  (public)   │────>│  Web App    │────>│  ZoldyQ         │   │
│  └─────────────┘     │  Service    │     │  Service        │   │
│                      └─────────────┘     │  (single task)  │   │
│                                          └────────┬────────┘   │
│                                                   │             │
│                      ┌─────────────┐              │             │
│                      │  ECS        │<─────────────┘             │
│                      │  Workers    │                            │
│                      │  (N tasks)  │                            │
│                      └──────┬──────┘                            │
│                             │                                   │
│                      ┌──────▼──────┐                            │
│                      │  S3 / RDS   │                            │
│                      │  (results)  │                            │
│                      └─────────────┘                            │
│                                                                  │
│  ┌─────────────┐                                                │
│  │  EBS Volume │  <- Snapshot storage                           │
│  │  (gp3)      │                                                │
│  └─────────────┘                                                │
└─────────────────────────────────────────────────────────────────┘
```

### ECS Task Definition

```json
{
  "family": "zoldyq-jobs",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "512",
  "memory": "1024",
  "executionRoleArn": "arn:aws:iam::YOUR_ACCOUNT:role/ecsTaskExecutionRole",
  "taskRoleArn": "arn:aws:iam::YOUR_ACCOUNT:role/zoldyqTaskRole",
  "containerDefinitions": [
    {
      "name": "zoldyq",
      "image": "YOUR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/zoldyq:latest",
      "essential": true,
      "portMappings": [
        {"containerPort": 6379, "protocol": "tcp"},
        {"containerPort": 6380, "protocol": "tcp"},
        {"containerPort": 8080, "protocol": "tcp"}
      ],
      "environment": [
        {"name": "ZOLDYQ_HOST", "value": "0.0.0.0"},
        {"name": "ZOLDYQ_PORT", "value": "6379"},
        {"name": "ZOLDYQ_ZQP_PORT", "value": "6380"},
        {"name": "ZOLDYQ_HTTP_PORT", "value": "8080"},
        {"name": "ZOLDYQ_ACK_ENABLED", "value": "true"},
        {"name": "ZOLDYQ_VISIBILITY_TIMEOUT", "value": "300"},
        {"name": "ZOLDYQ_MAX_RETRIES", "value": "3"},
        {"name": "ZOLDYQ_QUEUE_CAPACITY", "value": "100000"},
        {"name": "ZOLDYQ_SNAPSHOT_PATH", "value": "/data/snapshot.rdb"},
        {"name": "ZOLDYQ_SNAPSHOT_INTERVAL", "value": "300"},
        {"name": "ZOLDYQ_SNAPSHOT_ON_SHUTDOWN", "value": "true"}
      ],
      "secrets": [
        {
          "name": "ZOLDYQ_PASSWORD",
          "valueFrom": "arn:aws:secretsmanager:us-east-1:YOUR_ACCOUNT:secret:zoldyq/password"
        }
      ],
      "mountPoints": [
        {
          "sourceVolume": "zoldyq-data",
          "containerPath": "/data"
        }
      ],
      "healthCheck": {
        "command": ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"],
        "interval": 30,
        "timeout": 5,
        "retries": 3,
        "startPeriod": 10
      },
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/zoldyq",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "zoldyq"
        }
      }
    }
  ],
  "volumes": [
    {
      "name": "zoldyq-data",
      "efsVolumeConfiguration": {
        "fileSystemId": "fs-XXXXXXXXX",
        "rootDirectory": "/zoldyq",
        "transitEncryption": "ENABLED"
      }
    }
  ]
}
```

### Service Discovery (Cloud Map)

```bash
# Create namespace
aws servicediscovery create-private-dns-namespace \
  --name internal.local \
  --vpc vpc-XXXXXXXX

# Create service
aws servicediscovery create-service \
  --name zoldyq \
  --dns-config "NamespaceId=ns-XXXXXXXX,DnsRecords=[{Type=A,TTL=10}]" \
  --health-check-custom-config FailureThreshold=1
```

Workers connect to: `zoldyq.internal.local:6379`

### Worker Code Example (Python)

```python
import time
from zoldyq import ZoldyQ

def process_job(job_data):
    # Your job processing logic
    print(f"Processing: {job_data}")
    time.sleep(1)  # Simulate work
    return {"status": "completed"}

def main():
    client = ZoldyQ(
        host='zoldyq.internal.local',
        port=6379,
        password='your-secret-password'
    )
    client.connect()
    
    print("Worker started, waiting for jobs...")
    
    while True:
        try:
            msg = client.pop('jobs', timeout=30)
            if msg:
                try:
                    result = process_job(msg.payload)
                    client.ack(msg.id)
                    print(f"Job {msg.id} completed")
                except Exception as e:
                    print(f"Job {msg.id} failed: {e}")
                    client.nack(msg.id)
        except Exception as e:
            print(f"Connection error: {e}")
            time.sleep(5)
            client.connect()

if __name__ == '__main__':
    main()
```

### Auto Scaling Workers

```yaml
# CloudFormation snippet
WorkerAutoScaling:
  Type: AWS::ApplicationAutoScaling::ScalableTarget
  Properties:
    MaxCapacity: 10
    MinCapacity: 1
    ResourceId: !Sub service/${ECSCluster}/${WorkerService.Name}
    RoleARN: !GetAtt AutoScalingRole.Arn
    ScalableDimension: ecs:service:DesiredCount
    ServiceNamespace: ecs

WorkerScaleUpPolicy:
  Type: AWS::ApplicationAutoScaling::ScalingPolicy
  Properties:
    PolicyName: WorkerScaleUp
    PolicyType: TargetTrackingScaling
    ScalingTargetId: !Ref WorkerAutoScaling
    TargetTrackingScalingPolicyConfiguration:
      PredefinedMetricSpecification:
        PredefinedMetricType: ECSServiceAverageCPUUtilization
      TargetValue: 70
      ScaleInCooldown: 300
      ScaleOutCooldown: 60
```

### Queue-Based Auto Scaling (Custom Metric)

```python
# Lambda function to publish queue depth metric
import boto3
import redis

def handler(event, context):
    cloudwatch = boto3.client('cloudwatch')
    
    r = redis.Redis(host='zoldyq.internal.local', port=6379, password='xxx')
    queue_length = r.llen('jobs')
    
    cloudwatch.put_metric_data(
        Namespace='ZoldyQ',
        MetricData=[{
            'MetricName': 'QueueDepth',
            'Value': queue_length,
            'Unit': 'Count',
            'Dimensions': [{'Name': 'QueueName', 'Value': 'jobs'}]
        }]
    )
```

---

## Use Case 2: Event Buffer

**Scenario**: High-volume event ingestion that needs to be smoothed before processing or stored in a data warehouse.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         AWS VPC                                  │
│                                                                  │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────────┐   │
│  │  API GW     │     │  Lambda     │     │  ZoldyQ         │   │
│  │  (events)   │────>│  Ingest     │────>│  (buffer)       │   │
│  └─────────────┘     └─────────────┘     └────────┬────────┘   │
│                                                   │             │
│                      ┌─────────────────────────────┘             │
│                      │                                           │
│                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  ECS Service: Event Processor                            │   │
│  │  - Batch events (100 or 5 seconds)                       │   │
│  │  - Write to Kinesis Firehose / S3 / Redshift            │   │
│  └─────────────────────────────────────────────────────────┘   │
│                      │                                           │
│                      ▼                                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │  S3         │  │  Redshift   │  │  OpenSearch │            │
│  │  (raw)      │  │  (analytics)│  │  (search)   │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
└─────────────────────────────────────────────────────────────────┘
```

### ZoldyQ Configuration

```bash
# High-throughput event buffer settings
ZOLDYQ_QUEUE_CAPACITY=500000      # 500K events buffer
ZOLDYQ_MAX_QUEUES=100             # Multiple event types
ZOLDYQ_ACK_ENABLED=false          # No ACK needed for buffering
ZOLDYQ_SNAPSHOT_INTERVAL=60       # Frequent snapshots
ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=true
```

### Lambda Ingest Function

```python
import json
import redis
import os

# Connection pool (reused across invocations)
pool = redis.ConnectionPool(
    host=os.environ['ZOLDYQ_HOST'],
    port=6379,
    password=os.environ['ZOLDYQ_PASSWORD'],
    max_connections=10
)

def handler(event, context):
    r = redis.Redis(connection_pool=pool)
    
    # Process API Gateway events
    for record in event.get('Records', [event]):
        body = json.loads(record.get('body', '{}'))
        event_type = body.get('type', 'default')
        
        # Route to appropriate queue
        queue_name = f"events:{event_type}"
        r.lpush(queue_name, json.dumps(body))
    
    return {'statusCode': 200, 'body': 'OK'}
```

### Batch Processor

```python
import time
import json
import boto3
from zoldyq import ZoldyQ

BATCH_SIZE = 100
BATCH_TIMEOUT = 5  # seconds
S3_BUCKET = 'your-events-bucket'

def main():
    client = ZoldyQ(host='zoldyq.internal.local', port=6379)
    client.connect()
    
    s3 = boto3.client('s3')
    batch = []
    batch_start = time.time()
    
    while True:
        msg = client.pop('events:pageview', timeout=1)
        
        if msg:
            batch.append(msg.payload)
        
        # Flush batch if full or timeout reached
        should_flush = (
            len(batch) >= BATCH_SIZE or 
            (len(batch) > 0 and time.time() - batch_start > BATCH_TIMEOUT)
        )
        
        if should_flush:
            # Write to S3 as newline-delimited JSON
            key = f"events/{time.strftime('%Y/%m/%d/%H')}/{int(time.time())}.json"
            body = '\n'.join(json.dumps(e) for e in batch)
            s3.put_object(Bucket=S3_BUCKET, Key=key, Body=body)
            
            print(f"Flushed {len(batch)} events to s3://{S3_BUCKET}/{key}")
            batch = []
            batch_start = time.time()

if __name__ == '__main__':
    main()
```

### CloudWatch Dashboard

```json
{
  "widgets": [
    {
      "type": "metric",
      "properties": {
        "title": "Event Ingestion Rate",
        "metrics": [
          ["ZoldyQ", "EventsIngested", "Queue", "events:pageview"],
          ["ZoldyQ", "EventsProcessed", "Queue", "events:pageview"]
        ],
        "period": 60,
        "stat": "Sum"
      }
    },
    {
      "type": "metric",
      "properties": {
        "title": "Buffer Depth",
        "metrics": [
          ["ZoldyQ", "QueueDepth", "Queue", "events:pageview"]
        ],
        "period": 60,
        "stat": "Average"
      }
    }
  ]
}
```

---

## Use Case 3: Microservice Communication

**Scenario**: Services communicate asynchronously via message queues for decoupling and resilience.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         AWS VPC                                  │
│                                                                  │
│  ┌─────────────┐           ┌─────────────┐                     │
│  │  Order      │──────────>│             │                     │
│  │  Service    │           │             │                     │
│  └─────────────┘           │             │     ┌─────────────┐ │
│                            │   ZoldyQ    │────>│  Inventory  │ │
│  ┌─────────────┐           │   :6379     │     │  Service    │ │
│  │  Payment    │──────────>│             │     └─────────────┘ │
│  │  Service    │           │             │                     │
│  └─────────────┘           │             │     ┌─────────────┐ │
│                            │             │────>│  Shipping   │ │
│  ┌─────────────┐           │             │     │  Service    │ │
│  │  User       │──────────>│             │     └─────────────┘ │
│  │  Service    │           │             │                     │
│  └─────────────┘           └─────────────┘     ┌─────────────┐ │
│                                          │────>│  Email      │ │
│                                                │  Service    │ │
│                                                └─────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Queue Naming Convention

```
# Pattern: {service}.{action}.{priority}
order.created.high
order.shipped.normal
inventory.reserved.high
inventory.released.normal
notification.email.low
notification.sms.high
```

### ZoldyQ Configuration

```bash
ZOLDYQ_ACK_ENABLED=true
ZOLDYQ_VISIBILITY_TIMEOUT=60
ZOLDYQ_MAX_RETRIES=5
ZOLDYQ_MAX_QUEUES=500
ZOLDYQ_QUEUE_CAPACITY=50000
```

### Publisher Pattern (Go)

```go
package main

import (
    "encoding/json"
    "github.com/zoldyq/zoldyq-go/zoldyq"
)

type OrderCreatedEvent struct {
    OrderID   string `json:"order_id"`
    UserID    string `json:"user_id"`
    Amount    float64 `json:"amount"`
    Timestamp int64  `json:"timestamp"`
}

func PublishOrderCreated(client *zoldyq.Client, event OrderCreatedEvent) error {
    payload, _ := json.Marshal(event)
    
    _, err := client.Push("order.created.high", payload)
    return err
}

func main() {
    client := zoldyq.NewClient(zoldyq.Options{
        Host:     "zoldyq.internal.local",
        Port:     6379,
        Password: "secret",
    })
    
    if err := client.Connect(); err != nil {
        panic(err)
    }
    defer client.Close()
    
    event := OrderCreatedEvent{
        OrderID:   "ORD-12345",
        UserID:    "USR-67890",
        Amount:    99.99,
        Timestamp: time.Now().Unix(),
    }
    
    if err := PublishOrderCreated(client, event); err != nil {
        log.Printf("Failed to publish: %v", err)
    }
}
```

### Consumer Pattern (Go)

```go
package main

import (
    "encoding/json"
    "log"
    "github.com/zoldyq/zoldyq-go/zoldyq"
)

func main() {
    client := zoldyq.NewClient(zoldyq.Options{
        Host:     "zoldyq.internal.local",
        Port:     6379,
        Password: "secret",
    })
    
    if err := client.Connect(); err != nil {
        panic(err)
    }
    defer client.Close()
    
    log.Println("Inventory service started")
    
    for {
        msg, err := client.Pop("order.created.high", 30)
        if err != nil {
            log.Printf("Pop error: %v", err)
            continue
        }
        
        if msg == nil {
            continue  // Timeout, no message
        }
        
        var event OrderCreatedEvent
        if err := json.Unmarshal(msg.Payload.([]byte), &event); err != nil {
            log.Printf("Parse error: %v", err)
            client.Nack(msg.ID)
            continue
        }
        
        if err := reserveInventory(event.OrderID); err != nil {
            log.Printf("Reserve failed: %v", err)
            client.Nack(msg.ID)
            continue
        }
        
        client.Ack(msg.ID)
        log.Printf("Processed order %s", event.OrderID)
    }
}
```

### DLQ Processing

```go
func processDLQ(client *zoldyq.Client) {
    for {
        msg, _ := client.Pop("order.created.high:dlq", 30)
        if msg == nil {
            continue
        }
        
        // Log to error tracking
        logToSentry(msg)
        
        // Store in database for manual review
        storeFailedMessage(msg)
        
        client.Ack(msg.ID)
    }
}
```

---

## Use Case 4: Development/Staging Environment

**Scenario**: Fast, disposable queue for development and testing.

### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  zoldyq:
    image: zoldyq:latest
    ports:
      - "6379:6379"   # RESP
      - "6380:6380"   # ZQP
      - "8080:8080"   # Health
    environment:
      - ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=false  # Fast restarts
      - ZOLDYQ_SNAPSHOT_INTERVAL=0         # No snapshots
      - ZOLDYQ_ACK_ENABLED=true
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      retries: 3

  app:
    build: .
    depends_on:
      zoldyq:
        condition: service_healthy
    environment:
      - QUEUE_HOST=zoldyq
      - QUEUE_PORT=6379
```

### Kubernetes (EKS) Development

```yaml
# zoldyq-dev.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: zoldyq
  namespace: development
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
        image: YOUR_ACCOUNT.dkr.ecr.us-east-1.amazonaws.com/zoldyq:latest
        ports:
        - containerPort: 6379
        - containerPort: 6380
        - containerPort: 8080
        env:
        - name: ZOLDYQ_SNAPSHOT_ON_SHUTDOWN
          value: "false"
        - name: ZOLDYQ_SNAPSHOT_INTERVAL
          value: "0"
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "500m"
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
          initialDelaySeconds: 3
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: zoldyq
  namespace: development
spec:
  selector:
    app: zoldyq
  ports:
  - name: resp
    port: 6379
  - name: zqp
    port: 6380
  - name: http
    port: 8080
```

### Test Fixture Helper

```python
# tests/fixtures.py
import pytest
from zoldyq import ZoldyQ

@pytest.fixture
def queue_client():
    client = ZoldyQ(host='localhost', port=6379)
    client.connect()
    yield client
    # Cleanup all queues after test
    # Note: In real tests, use specific queue names per test
    client.close()

@pytest.fixture
def clean_queue(queue_client):
    """Provide a clean queue for each test"""
    import uuid
    queue_name = f"test-{uuid.uuid4()}"
    yield queue_name
    queue_client.delete(queue_name)

def test_job_processing(queue_client, clean_queue):
    # Push a job
    msg_id = queue_client.push(clean_queue, {"task": "test"})
    assert msg_id is not None
    
    # Pop and process
    msg = queue_client.pop(clean_queue, timeout=1)
    assert msg is not None
    assert msg.payload["task"] == "test"
    
    queue_client.ack(msg.id)
    
    # Queue should be empty
    assert queue_client.length(clean_queue) == 0
```

---

## Monitoring & Alerting

### CloudWatch Metrics (Custom)

```python
# metrics_publisher.py - Run as sidecar or Lambda
import boto3
import redis
import time

def publish_metrics():
    cloudwatch = boto3.client('cloudwatch')
    r = redis.Redis(host='zoldyq.internal.local', port=6379)
    
    while True:
        # Get INFO from ZoldyQ
        info = r.info()
        
        metrics = [
            {
                'MetricName': 'ConnectedClients',
                'Value': info.get('connected_clients', 0),
                'Unit': 'Count'
            },
            {
                'MetricName': 'TotalCommands',
                'Value': info.get('total_commands_processed', 0),
                'Unit': 'Count'
            },
            {
                'MetricName': 'UsedMemory',
                'Value': info.get('used_memory', 0),
                'Unit': 'Bytes'
            }
        ]
        
        # Get queue depths
        for key in r.keys('*'):
            if r.type(key) == b'list':
                queue_name = key.decode()
                depth = r.llen(key)
                metrics.append({
                    'MetricName': 'QueueDepth',
                    'Value': depth,
                    'Unit': 'Count',
                    'Dimensions': [{'Name': 'Queue', 'Value': queue_name}]
                })
        
        cloudwatch.put_metric_data(
            Namespace='ZoldyQ',
            MetricData=metrics
        )
        
        time.sleep(60)

if __name__ == '__main__':
    publish_metrics()
```

### CloudWatch Alarms

```yaml
# CloudFormation
QueueDepthAlarm:
  Type: AWS::CloudWatch::Alarm
  Properties:
    AlarmName: ZoldyQ-HighQueueDepth
    AlarmDescription: Queue depth exceeds threshold
    MetricName: QueueDepth
    Namespace: ZoldyQ
    Dimensions:
      - Name: Queue
        Value: jobs
    Statistic: Average
    Period: 300
    EvaluationPeriods: 2
    Threshold: 10000
    ComparisonOperator: GreaterThanThreshold
    AlarmActions:
      - !Ref AlertSNSTopic

HealthCheckAlarm:
  Type: AWS::CloudWatch::Alarm
  Properties:
    AlarmName: ZoldyQ-Unhealthy
    MetricName: HealthyHostCount
    Namespace: AWS/ApplicationELB
    Dimensions:
      - Name: TargetGroup
        Value: !Ref ZoldyQTargetGroup
    Statistic: Average
    Period: 60
    EvaluationPeriods: 2
    Threshold: 1
    ComparisonOperator: LessThanThreshold
    AlarmActions:
      - !Ref AlertSNSTopic
```

### Grafana Dashboard

```json
{
  "title": "ZoldyQ Dashboard",
  "panels": [
    {
      "title": "Throughput",
      "type": "graph",
      "targets": [
        {
          "expr": "rate(zoldyq_commands_total[5m])",
          "legendFormat": "Commands/sec"
        }
      ]
    },
    {
      "title": "Queue Depths",
      "type": "graph",
      "targets": [
        {
          "expr": "zoldyq_queue_depth",
          "legendFormat": "{{queue}}"
        }
      ]
    },
    {
      "title": "Memory Usage",
      "type": "gauge",
      "targets": [
        {
          "expr": "zoldyq_memory_used_bytes / zoldyq_memory_limit_bytes * 100"
        }
      ]
    }
  ]
}
```

---

## Backup & Recovery

### Automated Snapshots to S3

```python
# backup_to_s3.py - Run via cron or CloudWatch Events
import boto3
import subprocess
import os
from datetime import datetime

S3_BUCKET = 'your-backup-bucket'
SNAPSHOT_PATH = '/data/snapshot.rdb'

def backup():
    # Trigger snapshot
    subprocess.run(['redis-cli', '-p', '6379', 'BGSAVE'], check=True)
    
    # Wait for completion
    import time
    time.sleep(5)
    
    # Upload to S3
    s3 = boto3.client('s3')
    timestamp = datetime.utcnow().strftime('%Y%m%d-%H%M%S')
    key = f"zoldyq-backups/{timestamp}/snapshot.rdb"
    
    s3.upload_file(SNAPSHOT_PATH, S3_BUCKET, key)
    print(f"Backup uploaded to s3://{S3_BUCKET}/{key}")
    
    # Cleanup old backups (keep last 7 days)
    cleanup_old_backups(s3, S3_BUCKET, days=7)

def cleanup_old_backups(s3, bucket, days):
    from datetime import timedelta
    cutoff = datetime.utcnow() - timedelta(days=days)
    
    paginator = s3.get_paginator('list_objects_v2')
    for page in paginator.paginate(Bucket=bucket, Prefix='zoldyq-backups/'):
        for obj in page.get('Contents', []):
            if obj['LastModified'].replace(tzinfo=None) < cutoff:
                s3.delete_object(Bucket=bucket, Key=obj['Key'])
                print(f"Deleted old backup: {obj['Key']}")

if __name__ == '__main__':
    backup()
```

### Recovery Procedure

```bash
#!/bin/bash
# restore.sh

# 1. Stop ZoldyQ
aws ecs update-service --cluster prod --service zoldyq --desired-count 0

# 2. Download latest backup
LATEST=$(aws s3 ls s3://your-backup-bucket/zoldyq-backups/ | sort | tail -1 | awk '{print $2}')
aws s3 cp s3://your-backup-bucket/zoldyq-backups/${LATEST}snapshot.rdb /data/snapshot.rdb

# 3. Start ZoldyQ (will auto-load snapshot)
aws ecs update-service --cluster prod --service zoldyq --desired-count 1

# 4. Verify
sleep 30
curl http://zoldyq.internal.local:8080/health
```

---

## Security Considerations

### Network Security

```yaml
# Security Group
ZoldyQSecurityGroup:
  Type: AWS::EC2::SecurityGroup
  Properties:
    GroupDescription: ZoldyQ access
    VpcId: !Ref VPC
    SecurityGroupIngress:
      # RESP from app subnet only
      - IpProtocol: tcp
        FromPort: 6379
        ToPort: 6379
        SourceSecurityGroupId: !Ref AppSecurityGroup
      # ZQP from app subnet only
      - IpProtocol: tcp
        FromPort: 6380
        ToPort: 6380
        SourceSecurityGroupId: !Ref AppSecurityGroup
      # Health checks from ALB
      - IpProtocol: tcp
        FromPort: 8080
        ToPort: 8080
        SourceSecurityGroupId: !Ref ALBSecurityGroup
```

### Authentication

```bash
# Store password in Secrets Manager
aws secretsmanager create-secret \
  --name zoldyq/password \
  --secret-string "$(openssl rand -base64 32)"

# Reference in ECS task definition (see above)
```

### Encryption in Transit

For RESP protocol over TLS, use a sidecar proxy:

```yaml
# stunnel sidecar
containers:
  - name: stunnel
    image: stunnel:latest
    ports:
      - containerPort: 6378  # TLS port
    volumeMounts:
      - name: tls-certs
        mountPath: /etc/stunnel/certs
    command:
      - stunnel
      - /etc/stunnel/stunnel.conf
```

---

## Cost Estimation

### ECS Fargate (US-East-1)

| Use Case | vCPU | Memory | Monthly Cost |
|----------|------|--------|--------------|
| Background Jobs | 0.5 | 1 GB | ~$15 |
| Event Buffer | 1 | 2 GB | ~$35 |
| Microservices | 0.5 | 1 GB | ~$15 |
| Dev/Test | 0.25 | 0.5 GB | ~$8 |

### Additional Costs

| Resource | Use Case | Monthly Cost |
|----------|----------|--------------|
| EFS (1 GB) | Snapshot storage | ~$0.30 |
| CloudWatch Logs | 10 GB | ~$5 |
| CloudWatch Metrics | Custom metrics | ~$3 |
| NAT Gateway | Outbound traffic | ~$32 + data |

### Total Estimated Cost

| Environment | Monthly |
|-------------|---------|
| Development | ~$50 |
| Staging | ~$75 |
| Production (single AZ) | ~$100-150 |

**Note**: ZoldyQ is significantly cheaper than managed services like SQS ($0.40/million requests) or Amazon MQ ($0.30/hour) for high-volume workloads, but requires more operational overhead.

---

## Checklist: Production Readiness

Before going to production, verify:

- [ ] Password authentication enabled
- [ ] Network security groups configured
- [ ] Health checks configured in load balancer
- [ ] Snapshot backups automated and tested
- [ ] Recovery procedure documented and tested
- [ ] CloudWatch alarms configured
- [ ] Logging enabled and retained
- [ ] Resource limits set appropriately
- [ ] DLQ processing implemented
- [ ] Runbook documented for common issues
- [ ] On-call rotation established

---

## Troubleshooting

### High Memory Usage

```bash
# Check queue depths
redis-cli -p 6379 INFO queues

# Clear old/stuck messages
redis-cli -p 6379 DEL queue_name
```

### Slow Consumers

```bash
# Check in-flight messages
redis-cli -p 6379 INFO ack

# Reduce visibility timeout for faster retries
ZOLDYQ_VISIBILITY_TIMEOUT=30
```

### Connection Refused

```bash
# Check if process is running
curl http://localhost:8080/health

# Check logs
docker logs zoldyq

# Verify security group allows traffic
aws ec2 describe-security-groups --group-ids sg-XXXXXXXX
```

### Messages in DLQ

```bash
# Check DLQ depth
redis-cli -p 6379 LLEN queue_name:dlq

# Inspect messages
redis-cli -p 6379 LRANGE queue_name:dlq 0 10

# Reprocess DLQ
# (Move messages back to main queue)
while redis-cli -p 6379 RPOPLPUSH queue_name:dlq queue_name; do :; done
```
