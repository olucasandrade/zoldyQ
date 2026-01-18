//! Quick benchmark - runs in ~10 seconds
//! Run with: cargo run --release --bin quick_bench

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use zoldyq::{QueueManager, MessageQueue};

fn main() {
    println!("ZoldyQ Quick Benchmarks");
    println!("=======================\n");

    let rt = Runtime::new().unwrap();

    // 1. Single-threaded enqueue
    bench_enqueue();

    // 2. Single-threaded dequeue
    rt.block_on(bench_dequeue());

    // 3. Roundtrip latency
    rt.block_on(bench_roundtrip());

    // 4. Concurrent throughput
    rt.block_on(bench_concurrent());

    // 5. Message sizes
    bench_message_sizes();

    println!("\n✓ All benchmarks complete");
}

fn bench_enqueue() {
    let queue = MessageQueue::new("bench".to_string(), 1_000_000);
    let payload = serde_json::json!({"test": "data", "id": 123});
    let count = 100_000;

    let start = Instant::now();
    for _ in 0..count {
        let _ = queue.enqueue(payload.clone());
    }
    let elapsed = start.elapsed();

    let throughput = count as f64 / elapsed.as_secs_f64();
    let latency_ns = elapsed.as_nanos() as f64 / count as f64;

    println!("Enqueue (single-thread):");
    println!("  Throughput: {:.0} msg/sec", throughput);
    println!("  Latency:    {:.0} ns/op", latency_ns);
    println!();
}

async fn bench_dequeue() {
    let queue = MessageQueue::new("bench".to_string(), 1_000_000);
    let payload = serde_json::json!({"test": "data"});
    let count = 100_000;

    // Fill queue
    for _ in 0..count {
        let _ = queue.enqueue(payload.clone());
    }

    let start = Instant::now();
    for _ in 0..count {
        let _ = queue.dequeue(Duration::ZERO).await;
    }
    let elapsed = start.elapsed();

    let throughput = count as f64 / elapsed.as_secs_f64();
    let latency_ns = elapsed.as_nanos() as f64 / count as f64;

    println!("Dequeue (single-thread):");
    println!("  Throughput: {:.0} msg/sec", throughput);
    println!("  Latency:    {:.0} ns/op", latency_ns);
    println!();
}

async fn bench_roundtrip() {
    let queue = MessageQueue::new("bench".to_string(), 10_000);
    let payload = serde_json::json!({"test": "data"});
    let count = 10_000;

    let start = Instant::now();
    for _ in 0..count {
        queue.enqueue(payload.clone()).unwrap();
        let _ = queue.dequeue(Duration::ZERO).await;
    }
    let elapsed = start.elapsed();

    let latency_us = elapsed.as_micros() as f64 / count as f64;
    let throughput = count as f64 / elapsed.as_secs_f64();

    println!("Roundtrip (enqueue + dequeue):");
    println!("  Throughput: {:.0} ops/sec", throughput);
    println!("  Latency:    {:.2} µs/op", latency_us);
    println!();
}

async fn bench_concurrent() {
    for num_threads in [2, 4, 8] {
        let manager = Arc::new(QueueManager::new(1_000_000, 10));
        let payload = serde_json::json!({"test": "data"});
        let msgs_per_thread = 10_000;

        // Producers benchmark
        let start = Instant::now();
        let mut handles = vec![];
        for _ in 0..num_threads {
            let m = manager.clone();
            let p = payload.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..msgs_per_thread {
                    let _ = m.enqueue("bench", p.clone());
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let producer_elapsed = start.elapsed();
        let producer_throughput = (num_threads * msgs_per_thread) as f64 / producer_elapsed.as_secs_f64();

        // Consumers benchmark
        let start = Instant::now();
        let mut handles = vec![];
        for _ in 0..num_threads {
            let m = manager.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..msgs_per_thread {
                    let _ = m.dequeue("bench", Duration::ZERO).await;
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let consumer_elapsed = start.elapsed();
        let consumer_throughput = (num_threads * msgs_per_thread) as f64 / consumer_elapsed.as_secs_f64();

        println!("Concurrent ({} threads):", num_threads);
        println!("  Producer throughput: {:.0} msg/sec", producer_throughput);
        println!("  Consumer throughput: {:.0} msg/sec", consumer_throughput);
        println!();
    }
}

fn bench_message_sizes() {
    println!("Message size impact:");
    
    for size in [100, 1_000, 10_000, 100_000] {
        let queue = MessageQueue::new("bench".to_string(), 10_000);
        let data = "x".repeat(size);
        let payload = serde_json::json!({"data": data});
        let count = 1_000;

        let start = Instant::now();
        for _ in 0..count {
            let _ = queue.enqueue(payload.clone());
        }
        let elapsed = start.elapsed();

        let throughput = count as f64 / elapsed.as_secs_f64();
        let throughput_mb = (size * count) as f64 / elapsed.as_secs_f64() / 1_000_000.0;

        println!("  {:>6} bytes: {:>8.0} msg/sec ({:.1} MB/sec)", size, throughput, throughput_mb);
    }
}
