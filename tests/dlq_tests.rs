use std::sync::Arc;
use std::time::Duration;
use zoldyq::{QueueManager, AckManager};

#[tokio::test]
async fn test_message_moves_to_dlq_after_max_retries() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 2)); // max_retries = 2

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();

    // First delivery
    let msg1 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked1 = am.track_message("tasks", msg1);
    am.nack(&tracked1.id).unwrap();

    // Second delivery
    let msg2 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked2 = am.track_message("tasks", msg2);
    am.nack(&tracked2.id).unwrap();

    // Third delivery (exceeds max_retries=2) - should go to DLQ
    let main_queue = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap();
    assert!(main_queue.is_none(), "Main queue should be empty");

    let dlq = qm.dequeue("tasks:dlq", Duration::from_secs(0)).await.unwrap();
    assert!(dlq.is_some(), "Message should be in DLQ");
}

#[tokio::test]
async fn test_dlq_preserves_payload() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 1)); // max_retries = 1

    let original_payload = serde_json::json!({"important": "data", "value": 42});
    qm.enqueue("tasks", original_payload.clone()).unwrap();

    // First delivery
    let msg = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked = am.track_message("tasks", msg);
    am.nack(&tracked.id).unwrap(); // This exceeds max_retries=1

    let dlq_msg = qm.dequeue("tasks:dlq", Duration::from_secs(0)).await.unwrap().unwrap();
    assert_eq!(dlq_msg.payload, original_payload);
}

#[tokio::test]
async fn test_timeout_also_counts_as_retry() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 1, 2)); // 1s timeout, max_retries = 2

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();

    // First delivery - let it timeout (delivery_count becomes 1)
    let msg = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let _tracked = am.track_message("tasks", msg);
    
    tokio::time::sleep(Duration::from_millis(1100)).await;
    am.process_timeouts();

    // Message should be back in queue after first timeout
    let msg2 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    assert_eq!(msg2.delivery_count, 1);
    let _tracked2 = am.track_message("tasks", msg2);
    
    tokio::time::sleep(Duration::from_millis(1100)).await;
    am.process_timeouts();

    // After second timeout (delivery_count=2 >= max_retries=2), should be in DLQ
    let main = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap();
    assert!(main.is_none());

    let dlq = qm.dequeue("tasks:dlq", Duration::from_secs(0)).await.unwrap();
    assert!(dlq.is_some());
}

#[tokio::test]
async fn test_dlq_queue_name_format() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 0)); // max_retries = 0

    qm.enqueue("my-queue", serde_json::json!({"x": 1})).unwrap();

    let msg = qm.dequeue("my-queue", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked = am.track_message("my-queue", msg);
    am.nack(&tracked.id).unwrap();

    let dlq = qm.dequeue("my-queue:dlq", Duration::from_secs(0)).await.unwrap();
    assert!(dlq.is_some());
}

#[tokio::test]
async fn test_multiple_messages_dlq() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 0)); // max_retries = 0

    for i in 0..5 {
        qm.enqueue("tasks", serde_json::json!({"task": i})).unwrap();
    }

    for _ in 0..5 {
        let msg = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
        let tracked = am.track_message("tasks", msg);
        am.nack(&tracked.id).unwrap();
    }

    let mut dlq_count = 0;
    while qm.dequeue("tasks:dlq", Duration::from_secs(0)).await.unwrap().is_some() {
        dlq_count += 1;
    }

    assert_eq!(dlq_count, 5);
}
