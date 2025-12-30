#!/usr/bin/env python3
"""
ZoldyQ vs RabbitMQ Performance Comparison

ZoldyQ uses RESP, RabbitMQ uses AMQP.

Requirements:
    pip install redis pika numpy tabulate

Usage:
    python benchmarks/compare_rabbitmq.py
"""

import redis
import pika
import time
import json
import numpy as np
from tabulate import tabulate
import sys

# Configuration
ZOLDYQ_HOST = 'localhost'
ZOLDYQ_PORT = 6379
RABBITMQ_HOST = 'localhost'
RABBITMQ_PORT = 5672
QUEUE_NAME = 'benchmark_queue'
MESSAGE_COUNTS = [1000, 10000, 100000, 1000000]

def setup_zoldyq():
    """Setup ZoldyQ connection (RESP)"""
    try:
        client = redis.Redis(
            host=ZOLDYQ_HOST,
            port=ZOLDYQ_PORT,
            decode_responses=False,
            socket_connect_timeout=3
        )
        client.ping()
        return client
    except Exception as e:
        print(f"❌ Failed to connect to ZoldyQ: {e}")
        print(f"   Make sure ZoldyQ is running on port {ZOLDYQ_PORT}")
        sys.exit(1)

def setup_rabbitmq():
    """Setup RabbitMQ connection (AMQP)"""
    try:
        connection = pika.BlockingConnection(
            pika.ConnectionParameters(
                host=RABBITMQ_HOST,
                port=RABBITMQ_PORT,
                connection_attempts=3,
                retry_delay=1
            )
        )
        channel = connection.channel()
        channel.queue_declare(queue=QUEUE_NAME)
        return connection, channel
    except Exception as e:
        print(f"❌ Failed to connect to RabbitMQ: {e}")
        print(f"   Make sure RabbitMQ is running on port {RABBITMQ_PORT}")
        print(f"   Run: docker run -d --name rabbitmq -p {RABBITMQ_PORT}:5672 rabbitmq:3")
        sys.exit(1)

def benchmark_zoldyq_enqueue(client, num_messages):
    """Benchmark ZoldyQ LPUSH"""
    latencies = []
    
    for i in range(num_messages):
        message = json.dumps({
            "id": i,
            "data": f"test message {i}",
            "timestamp": time.time()
        })
        
        start = time.perf_counter()
        client.lpush(QUEUE_NAME.encode('utf-8'), message.encode('utf-8'))
        end = time.perf_counter()
        
        latencies.append((end - start) * 1000)
    
    return latencies

def benchmark_rabbitmq_enqueue(channel, num_messages):
    """Benchmark RabbitMQ publish"""
    latencies = []
    
    for i in range(num_messages):
        message = json.dumps({
            "id": i,
            "data": f"test message {i}",
            "timestamp": time.time()
        })
        
        start = time.perf_counter()
        channel.basic_publish(
            exchange='',
            routing_key=QUEUE_NAME,
            body=message
        )
        end = time.perf_counter()
        
        latencies.append((end - start) * 1000)
    
    return latencies

def benchmark_zoldyq_dequeue(client, num_messages):
    """Benchmark ZoldyQ RPOP"""
    latencies = []
    
    for i in range(num_messages):
        start = time.perf_counter()
        message = client.rpop(QUEUE_NAME.encode('utf-8'))
        end = time.perf_counter()
        
        if message:
            latencies.append((end - start) * 1000)
        else:
            print(f"⚠️  No message available at iteration {i}")
            break
    
    return latencies

def benchmark_rabbitmq_dequeue(channel, num_messages):
    """Benchmark RabbitMQ basic_get"""
    latencies = []
    
    for i in range(num_messages):
        start = time.perf_counter()
        method_frame, header_frame, body = channel.basic_get(queue=QUEUE_NAME)
        end = time.perf_counter()
        
        if method_frame:
            channel.basic_ack(method_frame.delivery_tag)
            latencies.append((end - start) * 1000)
        else:
            print(f"⚠️  No message available at iteration {i}")
            break
    
    return latencies

def calculate_stats(latencies):
    """Calculate statistics"""
    if not latencies:
        return {
            'min': 0,
            'mean': 0,
            'median': 0,
            'p95': 0,
            'p99': 0
        }
    
    return {
        'min': np.min(latencies),
        'mean': np.mean(latencies),
        'median': np.median(latencies),
        'p95': np.percentile(latencies, 95),
        'p99': np.percentile(latencies, 99)
    }

def print_comparison_table(title, zoldyq_stats, rabbitmq_stats):
    """Print comparison table"""
    print(f"\n{title}:")
    
    rows = []
    for metric in ['min', 'mean', 'median', 'p95', 'p99']:
        zoldyq_val = zoldyq_stats[metric]
        rabbitmq_val = rabbitmq_stats[metric]
        
        if zoldyq_val < rabbitmq_val:
            winner = "ZoldyQ"
        elif rabbitmq_val < zoldyq_val:
            winner = "RabbitMQ"
        else:
            winner = "Tie"
        
        rows.append([
            metric.capitalize(),
            f"{zoldyq_val:.3f}",
            f"{rabbitmq_val:.3f}",
            winner
        ])
    
    print(tabulate(rows, headers=['Metric', 'ZoldyQ', 'RabbitMQ', 'Winner'], tablefmt='grid'))

def main():
    print("=" * 80)
    print("ZoldyQ (RESP) vs RabbitMQ (AMQP) Performance Comparison")
    print("=" * 80)
    
    # Setup
    print("\n🔌 Connecting to services...")
    zoldyq_client = setup_zoldyq()
    rabbitmq_conn, rabbitmq_channel = setup_rabbitmq()
    print("✓ Connected to both services")
    
    # Warmup
    print("\n🔥 Warming up...")
    try:
        for _ in range(100):
            zoldyq_client.lpush(QUEUE_NAME.encode('utf-8'), b'warmup')
            rabbitmq_channel.basic_publish(exchange='', routing_key=QUEUE_NAME, body='warmup')
        
        zoldyq_client.delete(QUEUE_NAME.encode('utf-8'))
        while True:
            method = rabbitmq_channel.basic_get(queue=QUEUE_NAME)
            if not method[0]:
                break
            rabbitmq_channel.basic_ack(method[0].delivery_tag)
        
        print("✓ Warmup complete")
    except Exception as e:
        print(f"❌ Warmup failed: {e}")
        sys.exit(1)
    
    # Store all results
    all_results = []
    
    # Benchmark with different message counts
    for num_messages in MESSAGE_COUNTS:
        print("\n" + "=" * 80)
        print(f"📊 Benchmarking with {num_messages:,} messages")
        print("=" * 80)
        
        # Benchmark Enqueue
        print("\n📤 Benchmarking enqueue latency...")
        
        # Clear queues before enqueue benchmark
        try:
            zoldyq_client.delete(QUEUE_NAME.encode('utf-8'))
        except Exception:
            pass
        
        # Clear RabbitMQ queue
        try:
            rabbitmq_channel.queue_delete(queue=QUEUE_NAME)
            rabbitmq_channel.queue_declare(queue=QUEUE_NAME)
        except Exception:
            rabbitmq_channel.queue_declare(queue=QUEUE_NAME)
        
        print("   Testing ZoldyQ (RESP)...")
        zoldyq_enqueue_latencies = benchmark_zoldyq_enqueue(zoldyq_client, num_messages)
        zoldyq_enqueue_stats = calculate_stats(zoldyq_enqueue_latencies)
        
        print("   Testing RabbitMQ (AMQP)...")
        rabbitmq_enqueue_latencies = benchmark_rabbitmq_enqueue(rabbitmq_channel, num_messages)
        rabbitmq_enqueue_stats = calculate_stats(rabbitmq_enqueue_latencies)
        
        print_comparison_table("Enqueue Latency (milliseconds)", zoldyq_enqueue_stats, rabbitmq_enqueue_stats)
        
        # Benchmark Dequeue
        print("\n📥 Benchmarking dequeue latency...")
        
        print("   Testing ZoldyQ (RESP)...")
        zoldyq_dequeue_latencies = benchmark_zoldyq_dequeue(zoldyq_client, num_messages)
        zoldyq_dequeue_stats = calculate_stats(zoldyq_dequeue_latencies)
        
        print("   Testing RabbitMQ (AMQP)...")
        rabbitmq_dequeue_latencies = benchmark_rabbitmq_dequeue(rabbitmq_channel, num_messages)
        rabbitmq_dequeue_stats = calculate_stats(rabbitmq_dequeue_latencies)
        
        print_comparison_table("Dequeue Latency (milliseconds)", zoldyq_dequeue_stats, rabbitmq_dequeue_stats)
        
        # Calculate throughput
        zoldyq_enqueue_total = sum(zoldyq_enqueue_latencies) / 1000
        rabbitmq_enqueue_total = sum(rabbitmq_enqueue_latencies) / 1000
        zoldyq_dequeue_total = sum(zoldyq_dequeue_latencies) / 1000
        rabbitmq_dequeue_total = sum(rabbitmq_dequeue_latencies) / 1000
        
        zoldyq_enqueue_throughput = (
            num_messages / zoldyq_enqueue_total if zoldyq_enqueue_latencies else 0
        )
        rabbitmq_enqueue_throughput = (
            num_messages / rabbitmq_enqueue_total if rabbitmq_enqueue_latencies else 0
        )
        zoldyq_dequeue_throughput = (
            num_messages / zoldyq_dequeue_total if zoldyq_dequeue_latencies else 0
        )
        rabbitmq_dequeue_throughput = (
            num_messages / rabbitmq_dequeue_total if rabbitmq_dequeue_latencies else 0
        )
        
        print("\n📈 Throughput:")
        print(
            f"   Enqueue: ZoldyQ {zoldyq_enqueue_throughput:,.0f} msg/s vs "
            f"RabbitMQ {rabbitmq_enqueue_throughput:,.0f} msg/s"
        )
        print(
            f"   Dequeue: ZoldyQ {zoldyq_dequeue_throughput:,.0f} msg/s vs "
            f"RabbitMQ {rabbitmq_dequeue_throughput:,.0f} msg/s"
        )
        
        # Store results
        all_results.append({
            'count': num_messages,
            'zoldyq_enqueue': zoldyq_enqueue_stats,
            'rabbitmq_enqueue': rabbitmq_enqueue_stats,
            'zoldyq_dequeue': zoldyq_dequeue_stats,
            'rabbitmq_dequeue': rabbitmq_dequeue_stats,
            'zoldyq_enqueue_throughput': zoldyq_enqueue_throughput,
            'rabbitmq_enqueue_throughput': rabbitmq_enqueue_throughput,
            'zoldyq_dequeue_throughput': zoldyq_dequeue_throughput,
            'rabbitmq_dequeue_throughput': rabbitmq_dequeue_throughput,
        })
    
    # Overall Summary
    print("\n" + "=" * 80)
    print("Overall Summary")
    print("=" * 80)
    
    total_zoldyq_wins = 0
    total_rabbitmq_wins = 0
    
    for result in all_results:
        # Count wins for enqueue
        for metric in ['min', 'mean', 'median', 'p95', 'p99']:
            if result['zoldyq_enqueue'][metric] < result['rabbitmq_enqueue'][metric]:
                total_zoldyq_wins += 1
            elif result['rabbitmq_enqueue'][metric] < result['zoldyq_enqueue'][metric]:
                total_rabbitmq_wins += 1
        
        # Count wins for dequeue
        for metric in ['min', 'mean', 'median', 'p95', 'p99']:
            if result['zoldyq_dequeue'][metric] < result['rabbitmq_dequeue'][metric]:
                total_zoldyq_wins += 1
            elif result['rabbitmq_dequeue'][metric] < result['zoldyq_dequeue'][metric]:
                total_rabbitmq_wins += 1
    
    total_tests = len(MESSAGE_COUNTS) * 10  # 10 metrics per message count
    print(f"ZoldyQ wins: {total_zoldyq_wins}/{total_tests}")
    print(f"RabbitMQ wins: {total_rabbitmq_wins}/{total_tests}")
    
    if total_zoldyq_wins > total_rabbitmq_wins:
        print("\n🏆 ZoldyQ is faster overall!")
    elif total_rabbitmq_wins > total_zoldyq_wins:
        print("\n🏆 RabbitMQ is faster overall!")
    else:
        print("\n🤝 It's a tie!")
    
    print("\n💡 Protocol Comparison:")
    print("   ZoldyQ: RESP (Redis protocol) - Simple, fast")
    print("   RabbitMQ: AMQP - Complex, feature-rich")
    
    # Cleanup
    zoldyq_client.close()
    rabbitmq_conn.close()

if __name__ == '__main__':
    main()
