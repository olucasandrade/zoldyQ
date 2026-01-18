#!/usr/bin/env python3
"""
ACK Overhead Benchmark

Measures the latency impact of message acknowledgment vs no-ACK mode.

Usage:
    # Start ZoldyQ without ACK mode:
    ./target/release/zoldyq
    python benchmarks/ack_overhead.py --no-ack
    
    # Start ZoldyQ with ACK mode:
    ZOLDYQ_ACK_ENABLED=true ./target/release/zoldyq
    python benchmarks/ack_overhead.py --ack
"""

import redis
import time
import argparse
import numpy as np
from tabulate import tabulate

QUEUE_NAME = 'ack_benchmark'
MESSAGE_COUNTS = [1000, 10000]

def setup_client():
    client = redis.Redis(host='localhost', port=6379, decode_responses=False)
    client.ping()
    return client

def benchmark_no_ack(client, num_messages):
    """Benchmark without ACK mode"""
    client.delete(QUEUE_NAME.encode())
    
    for i in range(num_messages):
        client.lpush(QUEUE_NAME.encode(), f'{{"id": {i}}}'.encode())
    
    latencies = []
    for _ in range(num_messages):
        start = time.perf_counter()
        client.rpop(QUEUE_NAME.encode())
        end = time.perf_counter()
        latencies.append((end - start) * 1000)
    
    return latencies

def benchmark_with_ack(client, num_messages):
    """Benchmark with ACK mode (RPOP + ACK)"""
    client.delete(QUEUE_NAME.encode())
    
    for i in range(num_messages):
        client.lpush(QUEUE_NAME.encode(), f'{{"id": {i}}}'.encode())
    
    latencies = []
    for _ in range(num_messages):
        start = time.perf_counter()
        result = client.rpop(QUEUE_NAME.encode())
        if result:
            if isinstance(result, list) and len(result) >= 2:
                msg_id = result[0].decode() if isinstance(result[0], bytes) else result[0]
                client.execute_command('ACK', msg_id)
        end = time.perf_counter()
        latencies.append((end - start) * 1000)
    
    return latencies

def calculate_stats(latencies):
    if not latencies:
        return {}
    return {
        'min': np.min(latencies),
        'mean': np.mean(latencies),
        'median': np.median(latencies),
        'p95': np.percentile(latencies, 95),
        'p99': np.percentile(latencies, 99),
        'throughput': len(latencies) / (sum(latencies) / 1000)
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--ack', action='store_true', help='Run ACK mode benchmark')
    parser.add_argument('--no-ack', action='store_true', help='Run no-ACK mode benchmark')
    args = parser.parse_args()
    
    if not args.ack and not args.no_ack:
        print("Please specify --ack or --no-ack")
        return
    
    client = setup_client()
    mode = "ACK" if args.ack else "No-ACK"
    
    print(f"ACK Overhead Benchmark ({mode} mode)")
    print("=" * 60)
    
    all_results = []
    
    for num_messages in MESSAGE_COUNTS:
        print(f"\nBenchmarking {num_messages:,} messages...")
        
        if args.ack:
            latencies = benchmark_with_ack(client, num_messages)
        else:
            latencies = benchmark_no_ack(client, num_messages)
        
        stats = calculate_stats(latencies)
        all_results.append({
            'count': num_messages,
            **stats
        })
        
        rows = [
            ['Min (ms)', f"{stats['min']:.3f}"],
            ['Mean (ms)', f"{stats['mean']:.3f}"],
            ['Median (ms)', f"{stats['median']:.3f}"],
            ['P95 (ms)', f"{stats['p95']:.3f}"],
            ['P99 (ms)', f"{stats['p99']:.3f}"],
            ['Throughput (msg/s)', f"{stats['throughput']:,.0f}"],
        ]
        print(tabulate(rows, headers=['Metric', 'Value'], tablefmt='grid'))
    
    client.delete(QUEUE_NAME.encode())
    client.close()
    
    print(f"\nRun this benchmark in both modes to compare overhead:")
    print("  Without ACK: ./target/release/zoldyq && python benchmarks/ack_overhead.py --no-ack")
    print("  With ACK:    ZOLDYQ_ACK_ENABLED=true ./target/release/zoldyq && python benchmarks/ack_overhead.py --ack")

if __name__ == '__main__':
    main()
