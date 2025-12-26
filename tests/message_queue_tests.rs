use std::sync::Arc;
use std::time::Duration;
use zoldyq::{MessageQueue};

#[tokio::test]
async fn test_message_queue_creation() {
    let queue = MessageQueue::new("test-queue".to_string(), 100);
    
    assert_eq!(queue.name(), "test-queue");
    assert_eq!(queue.capacity(), 100);
    assert_eq!(queue.size(), 0);
}

#[tokio::test]
async fn test_enqueue_single_message() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    let payload = serde_json::json!({"test": "data"});
    
    let result = queue.enqueue(payload.clone());
    
    assert!(result.is_ok());
    assert_eq!(queue.size(), 1);
    assert_eq!(queue.stats().enqueued_total(), 1);
}

#[tokio::test]
async fn test_enqueue_returns_timestamp() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    let payload = serde_json::json!({"test": "data"});
    
    let timestamp = queue.enqueue(payload).unwrap().enqueued_at;
    
    assert!(timestamp > 0);
}

#[tokio::test]
async fn test_dequeue_single_message() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    let payload = serde_json::json!({"test": "data", "value": 42});
    
    queue.enqueue(payload.clone()).unwrap();
    
    let message = queue.dequeue(Duration::from_secs(0)).await.unwrap();
    
    assert_eq!(message.payload, payload);
    assert_eq!(queue.size(), 0);
    assert_eq!(queue.stats().dequeued_total(), 1);
}

#[tokio::test]
async fn test_dequeue_preserves_fifo_order() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    
    queue.enqueue(serde_json::json!({"order": 1})).unwrap();
    queue.enqueue(serde_json::json!({"order": 2})).unwrap();
    queue.enqueue(serde_json::json!({"order": 3})).unwrap();
    
    let msg1 = queue.dequeue(Duration::from_secs(0)).await.unwrap();
    let msg2 = queue.dequeue(Duration::from_secs(0)).await.unwrap();
    let msg3 = queue.dequeue(Duration::from_secs(0)).await.unwrap();
    
    assert_eq!(msg1.payload["order"], 1);
    assert_eq!(msg2.payload["order"], 2);
    assert_eq!(msg3.payload["order"], 3);
}

#[tokio::test]
async fn test_dequeue_empty_queue_returns_none() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    
    let result = queue.dequeue(Duration::from_secs(0)).await;
    
    assert!(result.is_none());
}

#[tokio::test]
async fn test_dequeue_with_timeout_waits_for_message() {
    let queue = Arc::new(MessageQueue::new("test-queue".to_string(), 10));
    let queue_clone = queue.clone();
    
    let dequeue_task = tokio::spawn(async move {
        let start = std::time::Instant::now();
        let result = queue_clone.dequeue(Duration::from_millis(500)).await;
        (result, start.elapsed())
    });
    
    queue.enqueue(serde_json::json!({"data": "test"})).unwrap();
    
    let (result, elapsed) = dequeue_task.await.unwrap();
    
    assert!(result.is_some());
    assert!(elapsed < Duration::from_millis(200));
}

#[tokio::test]
async fn test_dequeue_timeout_expires() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    
    let start = std::time::Instant::now();
    let result = queue.dequeue(Duration::from_millis(100)).await;
    let elapsed = start.elapsed();
    
    assert!(result.is_none());
    assert!(elapsed >= Duration::from_millis(100));
    assert!(elapsed < Duration::from_millis(150));
}

#[tokio::test]
async fn test_enqueue_full_queue_returns_error() {
    let queue = MessageQueue::new("test-queue".to_string(), 2);
    
    queue.enqueue(serde_json::json!({"msg": 1})).unwrap();
    queue.enqueue(serde_json::json!({"msg": 2})).unwrap();
    
    let result = queue.enqueue(serde_json::json!({"msg": 3}));
    
    assert!(result.is_err());
    assert_eq!(queue.size(), 2);
}

#[tokio::test]
async fn test_enqueue_notifies_waiting_consumers() {
    let queue = Arc::new(MessageQueue::new("test-queue".to_string(), 10));
    let queue_clone = queue.clone();
    
    let consumer = tokio::spawn(async move {
        queue_clone.dequeue(Duration::from_secs(2)).await
    });
    
    queue.enqueue(serde_json::json!({"notified": true})).unwrap();
    
    let result = consumer.await.unwrap();
    
    assert!(result.is_some());
    assert_eq!(result.unwrap().payload["notified"], true);
}

#[tokio::test]
async fn test_multiple_concurrent_enqueues() {
    let queue = Arc::new(MessageQueue::new("test-queue".to_string(), 1000));
    let mut handles = vec![];
    
    for i in 0..100 {
        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            queue_clone.enqueue(serde_json::json!({"id": i}))
        });
        handles.push(handle);
    }
    
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    
    assert_eq!(queue.size(), 100);
    assert_eq!(queue.stats().enqueued_total(), 100);
}

#[tokio::test]
async fn test_multiple_concurrent_dequeues() {
    let queue = Arc::new(MessageQueue::new("test-queue".to_string(), 100));
    
    for i in 0..50 {
        queue.enqueue(serde_json::json!({"id": i})).unwrap();
    }
    
    let mut handles = vec![];
    for _ in 0..50 {
        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            queue_clone.dequeue(Duration::from_secs(0)).await
        });
        handles.push(handle);
    }
    
    let mut count = 0;
    for handle in handles {
        if handle.await.unwrap().is_some() {
            count += 1;
        }
    }
    
    assert_eq!(count, 50);
    assert_eq!(queue.size(), 0);
    assert_eq!(queue.stats().dequeued_total(), 50);
}

#[tokio::test]
async fn test_queue_stats_track_all_operations() {
    let queue = MessageQueue::new("test-queue".to_string(), 5);
    
    queue.enqueue(serde_json::json!({"a": 1})).unwrap();
    queue.enqueue(serde_json::json!({"b": 2})).unwrap();
    queue.enqueue(serde_json::json!({"c": 3})).unwrap();
    
    queue.dequeue(Duration::from_secs(0)).await.unwrap();
    
    queue.enqueue(serde_json::json!({"d": 4})).unwrap();
    queue.enqueue(serde_json::json!({"e": 5})).unwrap();
    queue.enqueue(serde_json::json!({"f": 6})).unwrap();
    
    let full_result = queue.enqueue(serde_json::json!({"g": 7}));
    assert!(full_result.is_err());
    
    let stats = queue.stats();
    assert_eq!(stats.enqueued_total(), 6);
    assert_eq!(stats.dequeued_total(), 1);
}

#[tokio::test]
async fn test_message_contains_enqueued_timestamp() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    let payload = serde_json::json!({"test": "timestamp"});
    
    let enqueue_time = queue.enqueue(payload).unwrap().enqueued_at;
    let message = queue.dequeue(Duration::from_secs(0)).await.unwrap();
    
    assert_eq!(message.enqueued_at, enqueue_time);
    assert!(message.enqueued_at > 0);
}

#[tokio::test]
async fn test_queue_size_updates_correctly() {
    let queue = MessageQueue::new("test-queue".to_string(), 10);
    
    assert_eq!(queue.size(), 0);
    
    queue.enqueue(serde_json::json!({"a": 1})).unwrap();
    assert_eq!(queue.size(), 1);
    
    queue.enqueue(serde_json::json!({"b": 2})).unwrap();
    assert_eq!(queue.size(), 2);
    
    queue.dequeue(Duration::from_secs(0)).await.unwrap();
    assert_eq!(queue.size(), 1);
    
    queue.dequeue(Duration::from_secs(0)).await.unwrap();
    assert_eq!(queue.size(), 0);
}

#[tokio::test]
async fn test_multiple_consumers_waiting() {
    let queue = Arc::new(MessageQueue::new("test-queue".to_string(), 10));
    
    let mut consumers = vec![];
    for _ in 0..5 {
        let queue_clone = queue.clone();
        let consumer = tokio::spawn(async move {
            queue_clone.dequeue(Duration::from_secs(2)).await
        });
        consumers.push(consumer);
    }
    
    
    for i in 0..5 {
        queue.enqueue(serde_json::json!({"id": i})).unwrap();
    }
    
    for consumer in consumers {
        let result = consumer.await.unwrap();
        assert!(result.is_some());
    }
}

#[tokio::test]
async fn test_queue_capacity_enforced() {
    let capacity = 3;
    let queue = MessageQueue::new("test-queue".to_string(), capacity);
    
    for i in 0..capacity {
        let result = queue.enqueue(serde_json::json!({"id": i}));
        assert!(result.is_ok());
    }
    
    let overflow_result = queue.enqueue(serde_json::json!({"overflow": true}));
    assert!(overflow_result.is_err());
}

#[tokio::test]
async fn test_queue_name_is_preserved() {
    let queue = MessageQueue::new("my-special-queue".to_string(), 10);
    
    assert_eq!(queue.name(), "my-special-queue");
}

