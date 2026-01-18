#!/usr/bin/env python3
"""
Quick ZoldyQ vs Redis vs RabbitMQ benchmark
Runs in ~30 seconds with meaningful results
"""

import time
import json
import sys

# Configuration
ZOLDYQ_PORT = 6379
REDIS_PORT = 6381
RABBITMQ_PORT = 5673
MESSAGE_COUNTS = [1000, 10000, 50000]
QUEUE_NAME = 'benchmark_queue'

results = {
    'zoldyq': {'enqueue': [], 'dequeue': []},
    'redis': {'enqueue': [], 'dequeue': []},
    'rabbitmq': {'enqueue': [], 'dequeue': []},
}

def test_zoldyq():
    try:
        import redis
        client = redis.Redis(host='localhost', port=ZOLDYQ_PORT, socket_connect_timeout=2)
        client.ping()
        return client
    except Exception as e:
        print(f"  ZoldyQ not available: {e}")
        return None

def test_redis():
    try:
        import redis
        client = redis.Redis(host='localhost', port=REDIS_PORT, socket_connect_timeout=2)
        client.ping()
        return client
    except Exception as e:
        print(f"  Redis not available: {e}")
        return None

def test_rabbitmq():
    try:
        import pika
        conn = pika.BlockingConnection(pika.ConnectionParameters(
            host='localhost', port=RABBITMQ_PORT, connection_attempts=1))
        ch = conn.channel()
        ch.queue_declare(queue=QUEUE_NAME)
        return (conn, ch)
    except Exception as e:
        print(f"  RabbitMQ not available: {e}")
        return None

def bench_redis_like(client, name, count):
    """Benchmark Redis or ZoldyQ (both use RESP)"""
    msg = json.dumps({"id": 0, "data": "test"}).encode()
    
    # Clear
    client.delete(QUEUE_NAME)
    
    # Enqueue
    start = time.perf_counter()
    for i in range(count):
        client.lpush(QUEUE_NAME, msg)
    enqueue_time = time.perf_counter() - start
    
    # Dequeue
    start = time.perf_counter()
    for i in range(count):
        client.rpop(QUEUE_NAME)
    dequeue_time = time.perf_counter() - start
    
    return {
        'enqueue_throughput': count / enqueue_time,
        'dequeue_throughput': count / dequeue_time,
        'enqueue_latency_us': (enqueue_time / count) * 1_000_000,
        'dequeue_latency_us': (dequeue_time / count) * 1_000_000,
    }

def bench_rabbitmq(conn_ch, count):
    """Benchmark RabbitMQ"""
    conn, ch = conn_ch
    msg = json.dumps({"id": 0, "data": "test"})
    
    # Clear
    ch.queue_delete(queue=QUEUE_NAME)
    ch.queue_declare(queue=QUEUE_NAME)
    
    # Enqueue
    start = time.perf_counter()
    for i in range(count):
        ch.basic_publish(exchange='', routing_key=QUEUE_NAME, body=msg)
    enqueue_time = time.perf_counter() - start
    
    # Dequeue
    start = time.perf_counter()
    for i in range(count):
        method, props, body = ch.basic_get(queue=QUEUE_NAME)
        if method:
            ch.basic_ack(method.delivery_tag)
    dequeue_time = time.perf_counter() - start
    
    return {
        'enqueue_throughput': count / enqueue_time,
        'dequeue_throughput': count / dequeue_time,
        'enqueue_latency_us': (enqueue_time / count) * 1_000_000,
        'dequeue_latency_us': (dequeue_time / count) * 1_000_000,
    }

def main():
    print("=" * 70)
    print("ZoldyQ vs Redis vs RabbitMQ - Quick Benchmark")
    print("=" * 70)
    
    # Setup connections
    print("\nConnecting to services...")
    zoldyq = test_zoldyq()
    redis_client = test_redis()
    rabbitmq = test_rabbitmq()
    
    if not zoldyq:
        print("\n❌ ZoldyQ must be running on port 6379")
        print("   Run: ./target/release/zoldyq")
        sys.exit(1)
    
    all_results = []
    
    for count in MESSAGE_COUNTS:
        print(f"\n{'='*70}")
        print(f"Benchmarking with {count:,} messages")
        print("=" * 70)
        
        result = {'count': count}
        
        # ZoldyQ
        print(f"\n  ZoldyQ (RESP on :{ZOLDYQ_PORT})...")
        r = bench_redis_like(zoldyq, 'ZoldyQ', count)
        result['zoldyq'] = r
        print(f"    Enqueue: {r['enqueue_throughput']:,.0f} msg/s ({r['enqueue_latency_us']:.1f} µs)")
        print(f"    Dequeue: {r['dequeue_throughput']:,.0f} msg/s ({r['dequeue_latency_us']:.1f} µs)")
        
        # Redis
        if redis_client:
            print(f"\n  Redis (:{REDIS_PORT})...")
            r = bench_redis_like(redis_client, 'Redis', count)
            result['redis'] = r
            print(f"    Enqueue: {r['enqueue_throughput']:,.0f} msg/s ({r['enqueue_latency_us']:.1f} µs)")
            print(f"    Dequeue: {r['dequeue_throughput']:,.0f} msg/s ({r['dequeue_latency_us']:.1f} µs)")
        
        # RabbitMQ
        if rabbitmq:
            print(f"\n  RabbitMQ (AMQP on :{RABBITMQ_PORT})...")
            r = bench_rabbitmq(rabbitmq, count)
            result['rabbitmq'] = r
            print(f"    Enqueue: {r['enqueue_throughput']:,.0f} msg/s ({r['enqueue_latency_us']:.1f} µs)")
            print(f"    Dequeue: {r['dequeue_throughput']:,.0f} msg/s ({r['dequeue_latency_us']:.1f} µs)")
        
        all_results.append(result)
    
    # Summary
    print(f"\n{'='*70}")
    print("SUMMARY - Average Throughput (msg/sec)")
    print("=" * 70)
    
    services = ['zoldyq']
    if redis_client:
        services.append('redis')
    if rabbitmq:
        services.append('rabbitmq')
    
    header = f"{'Messages':<12}"
    for s in services:
        header += f"{s.upper():>20}"
    print(header)
    print("-" * 70)
    
    for result in all_results:
        row = f"{result['count']:<12,}"
        for s in services:
            if s in result:
                avg = (result[s]['enqueue_throughput'] + result[s]['dequeue_throughput']) / 2
                row += f"{avg:>20,.0f}"
            else:
                row += f"{'N/A':>20}"
        print(row)
    
    # Winner
    print(f"\n{'='*70}")
    print("CONCLUSION")
    print("=" * 70)
    
    if len(all_results) > 0:
        last = all_results[-1]
        zoldyq_avg = (last['zoldyq']['enqueue_throughput'] + last['zoldyq']['dequeue_throughput']) / 2
        
        if 'redis' in last:
            redis_avg = (last['redis']['enqueue_throughput'] + last['redis']['dequeue_throughput']) / 2
            ratio = zoldyq_avg / redis_avg
            print(f"ZoldyQ vs Redis: {ratio:.2f}x {'faster' if ratio > 1 else 'slower'}")
        
        if 'rabbitmq' in last:
            rmq_avg = (last['rabbitmq']['enqueue_throughput'] + last['rabbitmq']['dequeue_throughput']) / 2
            ratio = zoldyq_avg / rmq_avg
            print(f"ZoldyQ vs RabbitMQ: {ratio:.2f}x {'faster' if ratio > 1 else 'slower'}")
    
    # Cleanup
    if zoldyq:
        zoldyq.close()
    if redis_client:
        redis_client.close()
    if rabbitmq:
        rabbitmq[0].close()
    
    print("\n✓ Benchmark complete")

if __name__ == '__main__':
    main()
