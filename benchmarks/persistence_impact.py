#!/usr/bin/env python3
"""
Persistence Impact Benchmark

Measures throughput impact of snapshotting operations.

Usage:
    # Start ZoldyQ with persistence enabled:
    ZOLDYQ_SNAPSHOT_INTERVAL=0 ZOLDYQ_SNAPSHOT_ON_SHUTDOWN=true ./target/release/zoldyq
    python benchmarks/persistence_impact.py
"""

import redis
import time
import numpy as np
from tabulate import tabulate

QUEUE_NAME = 'persistence_benchmark'

def setup_client():
    client = redis.Redis(host='localhost', port=6379, decode_responses=False)
    client.ping()
    return client

def fill_queue(client, num_messages):
    """Fill queue with test messages"""
    start = time.perf_counter()
    for i in range(num_messages):
        client.lpush(QUEUE_NAME.encode(), f'{{"id": {i}}}'.encode())
    end = time.perf_counter()
    return (end - start) * 1000

def trigger_bgsave(client):
    """Trigger BGSAVE and measure time"""
    start = time.perf_counter()
    try:
        client.execute_command('BGSAVE')
    except Exception as e:
        if 'persistence not enabled' in str(e):
            return None
        raise
    end = time.perf_counter()
    return (end - start) * 1000

def benchmark_with_concurrent_operations(client, num_messages, num_operations):
    """Benchmark operations while BGSAVE might be running"""
    import threading
    import time as time_module
    
    latencies = []
    bgsave_time = None
    
    def bgsave_thread():
        nonlocal bgsave_time
        time_module.sleep(0.01)
        bgsave_time = trigger_bgsave(client)
    
    t = threading.Thread(target=bgsave_thread)
    t.start()
    
    for i in range(num_operations):
        start = time.perf_counter()
        client.lpush(QUEUE_NAME.encode(), f'{{"concurrent": {i}}}'.encode())
        end = time.perf_counter()
        latencies.append((end - start) * 1000)
    
    t.join()
    
    return latencies, bgsave_time

def main():
    client = setup_client()
    
    print("Persistence Impact Benchmark")
    print("=" * 60)
    print("This benchmark measures the impact of snapshotting on throughput.")
    print()
    
    message_counts = [1000, 10000, 50000]
    
    for num_messages in message_counts:
        print(f"\n--- {num_messages:,} messages ---")
        
        client.delete(QUEUE_NAME.encode())
        
        fill_time = fill_queue(client, num_messages)
        fill_throughput = num_messages / (fill_time / 1000)
        print(f"Fill time: {fill_time:.2f}ms ({fill_throughput:,.0f} msg/s)")
        
        bgsave_time = trigger_bgsave(client)
        if bgsave_time is not None:
            print(f"BGSAVE time: {bgsave_time:.2f}ms")
        else:
            print("BGSAVE: persistence not enabled (run with ZOLDYQ_SNAPSHOT_INTERVAL=0)")
        
        client.delete(QUEUE_NAME.encode())
        fill_queue(client, num_messages // 2)
        
        latencies, concurrent_bgsave = benchmark_with_concurrent_operations(
            client, num_messages // 2, 1000
        )
        
        if latencies:
            stats = {
                'mean': np.mean(latencies),
                'p95': np.percentile(latencies, 95),
                'p99': np.percentile(latencies, 99),
            }
            print(f"Operations during BGSAVE:")
            print(f"  Mean latency: {stats['mean']:.3f}ms")
            print(f"  P95 latency: {stats['p95']:.3f}ms")
            print(f"  P99 latency: {stats['p99']:.3f}ms")
        
        client.delete(QUEUE_NAME.encode())
    
    print("\n" + "=" * 60)
    print("Note: BGSAVE is synchronous in ZoldyQ (saves happen in the command handler).")
    print("For production, consider triggering BGSAVE during low-traffic periods.")

if __name__ == '__main__':
    main()
