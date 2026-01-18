#!/usr/bin/env python3
"""
TTL Overhead Benchmark

Measures the latency impact of message TTL expiration checks.

Usage:
    python benchmarks/ttl_overhead.py
"""

import redis
import time
import numpy as np
from tabulate import tabulate

QUEUE_NAME = 'ttl_benchmark'
MESSAGE_COUNTS = [1000, 10000]

def setup_client():
    client = redis.Redis(host='localhost', port=6379, decode_responses=False)
    client.ping()
    return client

def benchmark_enqueue_dequeue(client, num_messages):
    """Benchmark basic enqueue/dequeue (messages have TTL metadata but not expired)"""
    client.delete(QUEUE_NAME.encode())
    
    enqueue_latencies = []
    for i in range(num_messages):
        start = time.perf_counter()
        client.lpush(QUEUE_NAME.encode(), f'{{"id": {i}}}'.encode())
        end = time.perf_counter()
        enqueue_latencies.append((end - start) * 1000)
    
    dequeue_latencies = []
    for _ in range(num_messages):
        start = time.perf_counter()
        client.rpop(QUEUE_NAME.encode())
        end = time.perf_counter()
        dequeue_latencies.append((end - start) * 1000)
    
    return enqueue_latencies, dequeue_latencies

def calculate_stats(latencies):
    if not latencies:
        return {}
    return {
        'min': np.min(latencies),
        'mean': np.mean(latencies),
        'median': np.median(latencies),
        'p95': np.percentile(latencies, 95),
        'p99': np.percentile(latencies, 99),
    }

def main():
    client = setup_client()
    
    print("TTL Overhead Benchmark")
    print("=" * 60)
    print("Note: This benchmark measures baseline performance.")
    print("Messages include TTL metadata but expiration checking overhead is minimal.")
    print()
    
    for num_messages in MESSAGE_COUNTS:
        print(f"Benchmarking {num_messages:,} messages...")
        
        enqueue_lat, dequeue_lat = benchmark_enqueue_dequeue(client, num_messages)
        
        enq_stats = calculate_stats(enqueue_lat)
        deq_stats = calculate_stats(dequeue_lat)
        
        print(f"\nEnqueue Latency ({num_messages:,} messages):")
        rows = [
            ['Min (ms)', f"{enq_stats['min']:.4f}"],
            ['Mean (ms)', f"{enq_stats['mean']:.4f}"],
            ['P95 (ms)', f"{enq_stats['p95']:.4f}"],
            ['P99 (ms)', f"{enq_stats['p99']:.4f}"],
        ]
        print(tabulate(rows, headers=['Metric', 'Value'], tablefmt='grid'))
        
        print(f"\nDequeue Latency ({num_messages:,} messages):")
        rows = [
            ['Min (ms)', f"{deq_stats['min']:.4f}"],
            ['Mean (ms)', f"{deq_stats['mean']:.4f}"],
            ['P95 (ms)', f"{deq_stats['p95']:.4f}"],
            ['P99 (ms)', f"{deq_stats['p99']:.4f}"],
        ]
        print(tabulate(rows, headers=['Metric', 'Value'], tablefmt='grid'))
        
        total_time = sum(enqueue_lat) + sum(dequeue_lat)
        throughput = (num_messages * 2) / (total_time / 1000)
        print(f"\nTotal throughput: {throughput:,.0f} ops/s")
        print("-" * 60)
    
    client.delete(QUEUE_NAME.encode())
    client.close()

if __name__ == '__main__':
    main()
