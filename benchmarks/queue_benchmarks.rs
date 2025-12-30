use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use zoldyq::{QueueManager, MessageQueue};

/// Benchmark: Single-threaded enqueue operations
fn bench_enqueue_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("enqueue_single_thread");
    
    for capacity in [1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            capacity,
            |b, &capacity| {
                let queue = MessageQueue::new("bench".to_string(), capacity);
                let payload = serde_json::json!({"test": "data", "id": 123});
                
                b.iter(|| {
                    for _ in 0..1000 {
                        let _ = queue.enqueue(black_box(payload.clone()));
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Single-threaded dequeue operations
fn bench_dequeue_single_thread(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dequeue_single_thread");
    
    for capacity in [1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            capacity,
            |b, &capacity| {
                b.iter(|| {
                    rt.block_on(async {
                        let queue = MessageQueue::new("bench".to_string(), capacity);
                        let payload = serde_json::json!({"test": "data"});
                        
                        // Fill queue
                        for _ in 0..1000 {
                            let _ = queue.enqueue(payload.clone());
                        }
                        
                        // Dequeue all
                        for _ in 0..1000 {
                            let _ = black_box(queue.dequeue(Duration::from_secs(0)).await);
                        }
                    });
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Enqueue/Dequeue round-trip latency
fn bench_roundtrip_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("roundtrip_latency");
    group.sample_size(1000);
    
    group.bench_function("enqueue_then_dequeue", |b| {
        b.iter(|| {
            rt.block_on(async {
                let queue = MessageQueue::new("bench".to_string(), 10000);
                let payload = serde_json::json!({"test": "data"});
                
                queue.enqueue(black_box(payload)).unwrap();
                let _ = black_box(queue.dequeue(Duration::from_secs(0)).await);
            });
        });
    });
    
    group.finish();
}

/// Benchmark: Concurrent producers (multiple threads enqueuing)
fn bench_concurrent_producers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_producers");
    
    for num_producers in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*num_producers * 1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_producers),
            num_producers,
            |b, &num_producers| {
                b.iter(|| {
                    rt.block_on(async {
                        let manager = Arc::new(QueueManager::new(100000, 10));
                        let payload = serde_json::json!({"test": "data"});
                        
                        let mut handles = vec![];
                        for _ in 0..num_producers {
                            let manager = manager.clone();
                            let payload = payload.clone();
                            
                            let handle = tokio::spawn(async move {
                                for _ in 0..1000 {
                                    let _ = manager.enqueue("bench", black_box(payload.clone()));
                                }
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Concurrent consumers (multiple threads dequeuing)
fn bench_concurrent_consumers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_consumers");
    
    for num_consumers in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*num_consumers * 1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_consumers),
            num_consumers,
            |b, &num_consumers| {
                b.iter(|| {
                    rt.block_on(async {
                        let manager = Arc::new(QueueManager::new(100000, 10));
                        let payload = serde_json::json!({"test": "data"});
                        
                        // Fill queue
                        for _ in 0..(num_consumers * 1000) {
                            let _ = manager.enqueue("bench", payload.clone());
                        }
                        
                        let mut handles = vec![];
                        for _ in 0..num_consumers {
                            let manager = manager.clone();
                            
                            let handle = tokio::spawn(async move {
                                for _ in 0..1000 {
                                    let _ = black_box(
                                        manager.dequeue("bench", Duration::from_secs(0)).await
                                    );
                                }
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            handle.await.unwrap();
                        }
                    });
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Producer-Consumer pattern
fn bench_producer_consumer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("producer_consumer");
    group.sample_size(50);
    
    group.bench_function("1_producer_1_consumer", |b| {
        b.iter(|| {
            rt.block_on(async {
                let manager = Arc::new(QueueManager::new(100000, 10));
                let payload = serde_json::json!({"test": "data"});
                
                let producer = {
                    let manager = manager.clone();
                    let payload = payload.clone();
                    tokio::spawn(async move {
                        for _ in 0..10000 {
                            let _ = manager.enqueue("bench", payload.clone());
                        }
                    })
                };
                
                let consumer = {
                    let manager = manager.clone();
                    tokio::spawn(async move {
                        let mut count = 0;
                        while count < 10000 {
                            if let Ok(Some(_)) = manager.dequeue("bench", Duration::from_millis(100)).await {
                                count += 1;
                            }
                        }
                    })
                };
                
                producer.await.unwrap();
                consumer.await.unwrap();
            });
        });
    });
    
    group.finish();
}

/// Benchmark: Message size impact
fn bench_message_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_sizes");
    
    for size in [100, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let queue = MessageQueue::new("bench".to_string(), 10000);
                let data = "x".repeat(size);
                let payload = serde_json::json!({"data": data});
                
                b.iter(|| {
                    let _ = queue.enqueue(black_box(payload.clone()));
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_enqueue_single_thread,
    bench_dequeue_single_thread,
    bench_roundtrip_latency,
    bench_concurrent_producers,
    bench_concurrent_consumers,
    bench_producer_consumer,
    bench_message_sizes,
);

criterion_main!(benches);



