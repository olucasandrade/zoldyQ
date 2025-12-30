#!/usr/bin/env python3
"""
ZoldyQ vs Redis Performance Comparison

Both use RESP protocol for fair comparison.

Requirements:
    pip install redis numpy tabulate

Usage:
    python benchmarks/compare_redis.py
"""

import redis
import time
import json
import numpy as np
from tabulate import tabulate
import sys

# Configuration
ZOLDYQ_HOST = 'localhost'
ZOLDYQ_PORT = 6379
REDIS_HOST = 'localhost'
REDIS_PORT = 6380  # Redis on different port
MESSAGE_COUNTS = [1000, 10000, 100000, 1000000]
QUEUE_NAME = 'benchmark_queue'

def setup_zoldyq():
    """Setup ZoldyQ connection"""
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

def setup_redis():
    """Setup Redis connection"""
    try:
        client = redis.Redis(
            host=REDIS_HOST,
            port=REDIS_PORT,
            decode_responses=False,
            socket_connect_timeout=3
        )
        client.ping()
        return client
    except Exception as e:
        print(f"❌ Failed to connect to Redis: {e}")
        print(f"   Make sure Redis is running on port {REDIS_PORT}")
        print(f"   Run: docker run -d --name redis -p {REDIS_PORT}:6379 redis:7")
        sys.exit(1)

def benchmark_enqueue(client, num_messages):
    """Benchmark LPUSH performance"""
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
        
        latencies.append((end - start) * 1000)  # Convert to ms
    
    return latencies

def benchmark_dequeue(client, num_messages):
    """Benchmark RPOP performance"""
    latencies = []
    
    for i in range(num_messages):
        start = time.perf_counter()
        message = client.rpop(QUEUE_NAME.encode('utf-8'))
        end = time.perf_counter()
        
        if message:
            latencies.append((end - start) * 1000)  # Convert to ms
        else:
            print(f"⚠️  No message available at iteration {i}")
            break
    
    return latencies

def calculate_stats(latencies):
    """Calculate statistics from latency measurements"""
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

def print_comparison_table(title, zoldyq_stats, redis_stats):
    """Print comparison table"""
    print(f"\n{title}:")
    
    rows = []
    for metric in ['min', 'mean', 'median', 'p95', 'p99']:
        zoldyq_val = zoldyq_stats[metric]
        redis_val = redis_stats[metric]
        
        # Determine winner (lower is better)
        if zoldyq_val < redis_val:
            winner = "ZoldyQ"
        elif redis_val < zoldyq_val:
            winner = "Redis"
        else:
            winner = "Tie"
        
        rows.append([
            metric.capitalize(),
            f"{zoldyq_val:.3f}",
            f"{redis_val:.3f}",
            winner
        ])
    
    print(tabulate(rows, headers=['Metric', 'ZoldyQ', 'Redis', 'Winner'], tablefmt='grid'))

def main():
    print("=" * 80)
    print("ZoldyQ vs Redis Performance Comparison (Both using RESP)")
    print("=" * 80)
    
    # Setup connections
    print("\n🔌 Connecting to services...")
    zoldyq_client = setup_zoldyq()
    redis_client = setup_redis()
    print("✓ Connected to both services")
    
    # Warmup
    print("\n🔥 Warming up...")
    try:
        # Warmup with 100 messages
        for _ in range(100):
            zoldyq_client.lpush(QUEUE_NAME.encode('utf-8'), b'warmup')
            redis_client.lpush(QUEUE_NAME.encode('utf-8'), b'warmup')
        
        # Clear warmup messages
        zoldyq_client.delete(QUEUE_NAME.encode('utf-8'))
        redis_client.delete(QUEUE_NAME.encode('utf-8'))
        
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
        print(f"\n📤 Benchmarking enqueue latency...")
        
        # Clear queues before enqueue benchmark
        zoldyq_client.delete(QUEUE_NAME.encode('utf-8'))
        redis_client.delete(QUEUE_NAME.encode('utf-8'))

        print("   Testing ZoldyQ...")
        zoldyq_enqueue_latencies = benchmark_enqueue(zoldyq_client, num_messages)
        zoldyq_enqueue_stats = calculate_stats(zoldyq_enqueue_latencies)
        
        print("   Testing Redis...")
        redis_enqueue_latencies = benchmark_enqueue(redis_client, num_messages)
        redis_enqueue_stats = calculate_stats(redis_enqueue_latencies)
        
        print_comparison_table("Enqueue Latency (milliseconds)", zoldyq_enqueue_stats, redis_enqueue_stats)
        
        # Benchmark Dequeue
        print(f"\n📥 Benchmarking dequeue latency...")
        
        print("   Testing ZoldyQ...")
        zoldyq_dequeue_latencies = benchmark_dequeue(zoldyq_client, num_messages)
        zoldyq_dequeue_stats = calculate_stats(zoldyq_dequeue_latencies)
        
        print("   Testing Redis...")
        redis_dequeue_latencies = benchmark_dequeue(redis_client, num_messages)
        redis_dequeue_stats = calculate_stats(redis_dequeue_latencies)
        
        print_comparison_table("Dequeue Latency (milliseconds)", zoldyq_dequeue_stats, redis_dequeue_stats)
        
        # Calculate throughput
        zoldyq_enqueue_throughput = num_messages / (sum(zoldyq_enqueue_latencies) / 1000)
        redis_enqueue_throughput = num_messages / (sum(redis_enqueue_latencies) / 1000)
        zoldyq_dequeue_throughput = num_messages / (sum(zoldyq_dequeue_latencies) / 1000)
        redis_dequeue_throughput = num_messages / (sum(redis_dequeue_latencies) / 1000)
        
        print(f"\n📈 Throughput:")
        print(f"   Enqueue: ZoldyQ {zoldyq_enqueue_throughput:,.0f} msg/s vs Redis {redis_enqueue_throughput:,.0f} msg/s")
        print(f"   Dequeue: ZoldyQ {zoldyq_dequeue_throughput:,.0f} msg/s vs Redis {redis_dequeue_throughput:,.0f} msg/s")
        
        # Store results
        all_results.append({
            'count': num_messages,
            'zoldyq_enqueue': zoldyq_enqueue_stats,
            'redis_enqueue': redis_enqueue_stats,
            'zoldyq_dequeue': zoldyq_dequeue_stats,
            'redis_dequeue': redis_dequeue_stats,
            'zoldyq_enqueue_throughput': zoldyq_enqueue_throughput,
            'redis_enqueue_throughput': redis_enqueue_throughput,
            'zoldyq_dequeue_throughput': zoldyq_dequeue_throughput,
            'redis_dequeue_throughput': redis_dequeue_throughput,
        })
    
    # Overall Summary
    print("\n" + "=" * 80)
    print("Overall Summary")
    print("=" * 80)
    
    total_zoldyq_wins = 0
    total_redis_wins = 0
    
    for result in all_results:
        # Count wins for enqueue
        for metric in ['min', 'mean', 'median', 'p95', 'p99']:
            if result['zoldyq_enqueue'][metric] < result['redis_enqueue'][metric]:
                total_zoldyq_wins += 1
            elif result['redis_enqueue'][metric] < result['zoldyq_enqueue'][metric]:
                total_redis_wins += 1
        
        # Count wins for dequeue
        for metric in ['min', 'mean', 'median', 'p95', 'p99']:
            if result['zoldyq_dequeue'][metric] < result['redis_dequeue'][metric]:
                total_zoldyq_wins += 1
            elif result['redis_dequeue'][metric] < result['zoldyq_dequeue'][metric]:
                total_redis_wins += 1
    
    total_tests = len(MESSAGE_COUNTS) * 10  # 10 metrics per message count
    print(f"ZoldyQ wins: {total_zoldyq_wins}/{total_tests}")
    print(f"Redis wins: {total_redis_wins}/{total_tests}")
    
    if total_zoldyq_wins > total_redis_wins:
        print("\n🏆 ZoldyQ is faster overall!")
    elif total_redis_wins > total_zoldyq_wins:
        print("\n🏆 Redis is faster overall!")
    else:
        print("\n🤝 It's a tie!")
    
    print("\n💡 Note: Both use RESP protocol - difference is implementation")
    print("   ZoldyQ: Pure in-memory, optimized for queues")
    print("   Redis: General-purpose with disk persistence")
    
    # Cleanup
    zoldyq_client.close()
    redis_client.close()

if __name__ == '__main__':
    main()
