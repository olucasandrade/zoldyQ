use std::sync::Arc;
use std::time::Duration;
use zoldyq::{QueueManager, AckManager};

#[tokio::test]
async fn test_ack_removes_message_from_inflight() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 3));

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();

    let msg = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked = am.track_message("tasks", msg);

    assert_eq!(am.in_flight_count(), 1);

    let result = am.ack(&tracked.id);
    assert!(result.is_ok());
    assert_eq!(am.in_flight_count(), 0);
}

#[tokio::test]
async fn test_nack_returns_message_to_queue() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 3));

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();

    let msg = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked = am.track_message("tasks", msg);

    let result = am.nack(&tracked.id);
    assert!(result.is_ok());
    assert_eq!(am.in_flight_count(), 0);

    let returned = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap();
    assert!(returned.is_some());
}

#[tokio::test]
async fn test_ack_unknown_message_fails() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 3));

    let result = am.ack("nonexistent-id");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_visibility_timeout_returns_message() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 1, 3)); // 1 second timeout

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();

    let msg = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let _tracked = am.track_message("tasks", msg);

    assert_eq!(am.in_flight_count(), 1);

    tokio::time::sleep(Duration::from_millis(1100)).await;
    am.process_timeouts();

    assert_eq!(am.in_flight_count(), 0);

    let returned = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap();
    assert!(returned.is_some());
}

#[tokio::test]
async fn test_delivery_count_increments() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let am = Arc::new(AckManager::new(qm.clone(), 30, 3));

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();

    let msg1 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    assert_eq!(msg1.delivery_count, 0);

    let tracked1 = am.track_message("tasks", msg1);
    assert_eq!(tracked1.delivery_count, 1);

    am.nack(&tracked1.id).unwrap();

    let msg2 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let tracked2 = am.track_message("tasks", msg2);
    assert_eq!(tracked2.delivery_count, 2);
}

#[tokio::test]
async fn test_message_has_unique_id() {
    let qm = Arc::new(QueueManager::new(100, 10));

    qm.enqueue("tasks", serde_json::json!({"task": 1})).unwrap();
    qm.enqueue("tasks", serde_json::json!({"task": 2})).unwrap();

    let msg1 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    let msg2 = qm.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();

    assert_ne!(msg1.id, msg2.id);
    assert!(!msg1.id.is_empty());
    assert!(!msg2.id.is_empty());
}
