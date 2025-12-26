use std::sync::Arc;
use std::time::Duration;
use zoldyq::QueueManager;

#[tokio::test]
async fn test_end_to_end_single_message() {
    let manager = Arc::new(QueueManager::new(100, 10));
    
    let payload = serde_json::json!({
        "email": "user@example.com",
        "subject": "Welcome",
        "body": "Thanks for signing up!"
    });
    
    manager.enqueue("emails", payload.clone()).unwrap();
    
    let message = manager.dequeue("emails", Duration::from_secs(0)).await.unwrap().unwrap();
    
    assert_eq!(message.payload, payload);
}

#[tokio::test]
async fn test_producer_consumer_pattern() {
    let manager = Arc::new(QueueManager::new(1000, 10));
    
    let producer = {
        let manager = manager.clone();
        tokio::spawn(async move {
            for i in 0..100 {
                let payload = serde_json::json!({"id": i});
                manager.enqueue("tasks", payload).unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
    };
    
    let consumer = {
        let manager = manager.clone();
        tokio::spawn(async move {
            let mut count = 0;
            for _ in 0..100 {
                let msg = manager.dequeue("tasks", Duration::from_secs(2)).await.unwrap();
                if msg.is_some() {
                    count += 1;
                }
            }
            count
        })
    };
    
    producer.await.unwrap();
    let consumed = consumer.await.unwrap();
    
    assert_eq!(consumed, 100);
}

#[tokio::test]
async fn test_multiple_producers_single_consumer() {
    let manager = Arc::new(QueueManager::new(1000, 10));
    let mut producers = vec![];
    
    for i in 0..5 {
        let manager = manager.clone();
        let producer = tokio::spawn(async move {
            for j in 0..20 {
                let payload = serde_json::json!({"producer": i, "item": j});
                manager.enqueue("shared", payload).unwrap();
            }
        });
        producers.push(producer);
    }
    
    for producer in producers {
        producer.await.unwrap();
    }
    
    let queue = manager.get_queue("shared").unwrap();
    assert_eq!(queue.size(), 100);
    
    for _ in 0..100 {
        let msg = manager.dequeue("shared", Duration::from_secs(0)).await.unwrap();
        assert!(msg.is_some());
    }
    
    assert_eq!(queue.size(), 0);
}

#[tokio::test]
async fn test_single_producer_multiple_consumers() {
    let manager = Arc::new(QueueManager::new(1000, 10));
    
    for i in 0..100 {
        let payload = serde_json::json!({"id": i});
        manager.enqueue("work", payload).unwrap();
    }
    
    let mut consumers = vec![];
    for _ in 0..5 {
        let manager = manager.clone();
        let consumer = tokio::spawn(async move {
            let mut count = 0;
            loop {
                let msg = manager.dequeue("work", Duration::from_millis(100)).await.unwrap();
                match msg {
                    Some(_) => count += 1,
                    None => break,
                }
            }
            count
        });
        consumers.push(consumer);
    }
    
    let mut total = 0;
    for consumer in consumers {
        total += consumer.await.unwrap();
    }
    
    assert_eq!(total, 100);
}

#[tokio::test]
async fn test_multiple_queues_isolated_operations() {
    let manager = Arc::new(QueueManager::new(100, 10));
    
    let emails_producer = {
        let manager = manager.clone();
        tokio::spawn(async move {
            for i in 0..50 {
                manager.enqueue("emails", serde_json::json!({"email_id": i})).unwrap();
            }
        })
    };
    
    let webhooks_producer = {
        let manager = manager.clone();
        tokio::spawn(async move {
            for i in 0..30 {
                manager.enqueue("webhooks", serde_json::json!({"webhook_id": i})).unwrap();
            }
        })
    };
    
    emails_producer.await.unwrap();
    webhooks_producer.await.unwrap();
    
    let emails_queue = manager.get_queue("emails").unwrap();
    let webhooks_queue = manager.get_queue("webhooks").unwrap();
    
    assert_eq!(emails_queue.size(), 50);
    assert_eq!(webhooks_queue.size(), 30);
}

#[tokio::test]
async fn test_backpressure_handling() {
    let manager = Arc::new(QueueManager::new(10, 5));
    
    for i in 0..10 {
        let result = manager.enqueue("limited", serde_json::json!({"id": i}));
        assert!(result.is_ok());
    }
    
    let result = manager.enqueue("limited", serde_json::json!({"overflow": true}));
    assert!(result.is_err());
    
    manager.dequeue("limited", Duration::from_secs(0)).await.unwrap().unwrap();
    
    let result = manager.enqueue("limited", serde_json::json!({"after_dequeue": true}));
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_consumer_blocks_until_message_arrives() {
    let manager = Arc::new(QueueManager::new(100, 10));
    
    let consumer = {
        let manager = manager.clone();
        tokio::spawn(async move {
            let start = std::time::Instant::now();
            let msg = manager.dequeue("delayed", Duration::from_secs(2)).await.unwrap();
            (msg, start.elapsed())
        })
    };
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    manager.enqueue("delayed", serde_json::json!({"arrived": true})).unwrap();
    
    let (msg, elapsed) = consumer.await.unwrap();
    
    assert!(msg.is_some());
    assert!(elapsed < Duration::from_secs(1));
    assert_eq!(msg.unwrap().payload["arrived"], true);
}

#[tokio::test]
async fn test_fifo_order_under_load() {
    let manager = Arc::new(QueueManager::new(1000, 10));
    
    for i in 0..500 {
        let payload = serde_json::json!({"sequence": i});
        manager.enqueue("ordered", payload).unwrap();
    }
    
    for i in 0..500 {
        let msg = manager.dequeue("ordered", Duration::from_secs(0)).await.unwrap().unwrap();
        assert_eq!(msg.payload["sequence"], i);
    }
}

#[tokio::test]
async fn test_stats_accuracy_under_concurrent_operations() {
    let manager = Arc::new(QueueManager::new(1000, 10));
    let mut handles = vec![];
    
    for i in 0..50 {
        let manager = manager.clone();
        let handle = tokio::spawn(async move {
            manager.enqueue("stats_test", serde_json::json!({"id": i})).unwrap();
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.await.unwrap();
    }
    
    let queue = manager.get_queue("stats_test").unwrap();
    assert_eq!(queue.stats().enqueued_total(), 50);
    
    let mut dequeue_handles = vec![];
    for _ in 0..25 {
        let manager = manager.clone();
        let handle = tokio::spawn(async move {
            manager.dequeue("stats_test", Duration::from_secs(0)).await.unwrap()
        });
        dequeue_handles.push(handle);
    }
    
    for handle in dequeue_handles {
        handle.await.unwrap();
    }
    
    assert_eq!(queue.stats().dequeued_total(), 25);
    assert_eq!(queue.size(), 25);
}

#[tokio::test]
async fn test_high_throughput_scenario() {
    let manager = Arc::new(QueueManager::new(10000, 10));
    
    let start = std::time::Instant::now();
    
    for i in 0..1000 {
        let payload = serde_json::json!({"id": i});
        manager.enqueue("throughput", payload).unwrap();
    }
    
    let enqueue_duration = start.elapsed();
    
    let start = std::time::Instant::now();
    
    for _ in 0..1000 {
        manager.dequeue("throughput", Duration::from_secs(0)).await.unwrap().unwrap();
    }
    
    let dequeue_duration = start.elapsed();
    
    println!("Enqueued 1000 messages in {:?}", enqueue_duration);
    println!("Dequeued 1000 messages in {:?}", dequeue_duration);
    
    assert!(enqueue_duration < Duration::from_secs(1));
    assert!(dequeue_duration < Duration::from_secs(1));
}

#[tokio::test]
async fn test_queue_full_count_tracking() {
    let manager = Arc::new(QueueManager::new(5, 10));
    
    for i in 0..5 {
        manager.enqueue("full_test", serde_json::json!({"id": i})).unwrap();
    }
    
    for _ in 0..10 {
        let _ = manager.enqueue("full_test", serde_json::json!({"overflow": true}));
    }
    
    let queue = manager.get_queue("full_test").unwrap();
    assert_eq!(queue.stats().queue_full_count(), 10);
}

#[tokio::test]
async fn test_message_timestamps_are_sequential() {
    let manager = Arc::new(QueueManager::new(100, 10));
    
    let mut timestamps = vec![];
    
    for i in 0..10 {
        let msg = manager.enqueue("timestamps", serde_json::json!({"id": i})).unwrap();
        timestamps.push(msg.enqueued_at);
        tokio::time::sleep(Duration::from_micros(100)).await;
    }
    
    for i in 1..timestamps.len() {
        assert!(timestamps[i] >= timestamps[i-1]);
    }
}

