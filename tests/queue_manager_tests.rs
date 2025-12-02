use std::sync::Arc;
use std::time::Duration;
use zoldyq::{MessageQueue, QueueManager};

#[tokio::test]
async fn test_queue_manager_creation() {
    let manager = QueueManager::new(100000, 100);
    
    assert_eq!(manager.queue_count(), 0);
    assert_eq!(manager.default_capacity(), 100000);
    assert_eq!(manager.max_queues(), 100);
}

#[tokio::test]
async fn test_get_or_create_queue() {
    let manager = QueueManager::new(100, 10);
    
    let queue = manager.get_or_create_queue("emails").await;
    
    assert!(queue.is_ok());
    assert_eq!(queue.unwrap().name(), "emails");
    assert_eq!(manager.queue_count(), 1);
}

#[tokio::test]
async fn test_get_or_create_returns_existing_queue() {
    let manager = QueueManager::new(100, 10);
    
    let queue1 = manager.get_or_create_queue("emails").await.unwrap();
    queue1.enqueue(serde_json::json!({"test": "data"})).await.unwrap();
    
    let queue2 = manager.get_or_create_queue("emails").await.unwrap();
    
    assert_eq!(queue2.size(), 1);
    assert_eq!(manager.queue_count(), 1);
}

#[tokio::test]
async fn test_get_queue_existing() {
    let manager = QueueManager::new(100, 10);
    
    manager.get_or_create_queue("webhooks").await.unwrap();
    
    let queue = manager.get_queue("webhooks");
    
    assert!(queue.is_some());
    assert_eq!(queue.unwrap().name(), "webhooks");
}

#[tokio::test]
async fn test_get_queue_nonexistent() {
    let manager = QueueManager::new(100, 10);
    
    let queue = manager.get_queue("nonexistent");
    
    assert!(queue.is_none());
}

#[tokio::test]
async fn test_create_multiple_queues() {
    let manager = QueueManager::new(100, 10);
    
    manager.get_or_create_queue("emails").await.unwrap();
    manager.get_or_create_queue("webhooks").await.unwrap();
    manager.get_or_create_queue("images").await.unwrap();
    
    assert_eq!(manager.queue_count(), 3);
    assert!(manager.get_queue("emails").is_some());
    assert!(manager.get_queue("webhooks").is_some());
    assert!(manager.get_queue("images").is_some());
}

#[tokio::test]
async fn test_queue_capacity_from_manager() {
    let manager = QueueManager::new(500, 10);
    
    let queue = manager.get_or_create_queue("test").await.unwrap();
    
    assert_eq!(queue.capacity(), 500);
}

#[tokio::test]
async fn test_max_queues_limit_enforced() {
    let manager = QueueManager::new(100, 3);
    
    manager.get_or_create_queue("queue1").await.unwrap();
    manager.get_or_create_queue("queue2").await.unwrap();
    manager.get_or_create_queue("queue3").await.unwrap();
    
    let result = manager.get_or_create_queue("queue4").await;
    
    assert!(result.is_err());
    assert_eq!(manager.queue_count(), 3);
}

#[tokio::test]
async fn test_list_all_queues() {
    let manager = QueueManager::new(100, 10);
    
    manager.get_or_create_queue("emails").await.unwrap();
    manager.get_or_create_queue("webhooks").await.unwrap();
    
    let queues = manager.list_queues();
    
    assert_eq!(queues.len(), 2);
    assert!(queues.contains(&"emails".to_string()));
    assert!(queues.contains(&"webhooks".to_string()));
}

#[tokio::test]
async fn test_get_all_queue_stats() {
    let manager = QueueManager::new(100, 10);
    
    let queue1 = manager.get_or_create_queue("emails").await.unwrap();
    let queue2 = manager.get_or_create_queue("webhooks").await.unwrap();
    
    queue1.enqueue(serde_json::json!({"a": 1})).await.unwrap();
    queue1.enqueue(serde_json::json!({"b": 2})).await.unwrap();
    queue2.enqueue(serde_json::json!({"c": 3})).await.unwrap();
    
    let all_stats = manager.get_all_stats();
    
    assert_eq!(all_stats.len(), 2);
    assert!(all_stats.contains_key("emails"));
    assert!(all_stats.contains_key("webhooks"));
}

#[tokio::test]
async fn test_concurrent_queue_creation() {
    let manager = Arc::new(QueueManager::new(100, 50));
    let mut handles = vec![];
    
    for i in 0..20 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            manager_clone.get_or_create_queue(&format!("queue-{}", i)).await
        });
        handles.push(handle);
    }
    
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    
    assert_eq!(manager.queue_count(), 20);
}

#[tokio::test]
async fn test_concurrent_access_to_same_queue() {
    let manager = Arc::new(QueueManager::new(1000, 10));
    let mut handles = vec![];
    
    for i in 0..50 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let queue = manager_clone.get_or_create_queue("shared").await.unwrap();
            queue.enqueue(serde_json::json!({"id": i})).await
        });
        handles.push(handle);
    }
    
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    
    assert_eq!(manager.queue_count(), 1);
    let queue = manager.get_queue("shared").unwrap();
    assert_eq!(queue.size(), 50);
}

#[tokio::test]
async fn test_queue_isolation() {
    let manager = QueueManager::new(100, 10);
    
    let queue1 = manager.get_or_create_queue("queue1").await.unwrap();
    let queue2 = manager.get_or_create_queue("queue2").await.unwrap();
    
    queue1.enqueue(serde_json::json!({"queue": 1})).await.unwrap();
    queue1.enqueue(serde_json::json!({"queue": 1})).await.unwrap();
    queue2.enqueue(serde_json::json!({"queue": 2})).await.unwrap();
    
    assert_eq!(queue1.size(), 2);
    assert_eq!(queue2.size(), 1);
    
    let msg = queue2.dequeue(Duration::from_secs(0)).await.unwrap();
    assert_eq!(msg.payload["queue"], 2);
    assert_eq!(queue1.size(), 2);
}

#[tokio::test]
async fn test_enqueue_to_named_queue() {
    let manager = QueueManager::new(100, 10);
    
    let result = manager.enqueue("emails", serde_json::json!({"test": "data"})).await;
    
    assert!(result.is_ok());
    
    let queue = manager.get_queue("emails").unwrap();
    assert_eq!(queue.size(), 1);
}

#[tokio::test]
async fn test_dequeue_from_named_queue() {
    let manager = QueueManager::new(100, 10);
    
    manager.enqueue("emails", serde_json::json!({"order": 1})).await.unwrap();
    manager.enqueue("emails", serde_json::json!({"order": 2})).await.unwrap();
    
    let msg1 = manager.dequeue("emails", Duration::from_secs(0)).await.unwrap();
    let msg2 = manager.dequeue("emails", Duration::from_secs(0)).await.unwrap();
    
    assert_eq!(msg1.payload["order"], 1);
    assert_eq!(msg2.payload["order"], 2);
}

#[tokio::test]
async fn test_dequeue_from_nonexistent_queue() {
    let manager = QueueManager::new(100, 10);
    
    let result = manager.dequeue("nonexistent", Duration::from_secs(0)).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_queue_names_case_sensitive() {
    let manager = QueueManager::new(100, 10);
    
    manager.get_or_create_queue("Emails").await.unwrap();
    manager.get_or_create_queue("emails").await.unwrap();
    
    assert_eq!(manager.queue_count(), 2);
    assert!(manager.get_queue("Emails").is_some());
    assert!(manager.get_queue("emails").is_some());
}

#[tokio::test]
async fn test_queue_names_with_special_characters() {
    let manager = QueueManager::new(100, 10);
    
    let result1 = manager.get_or_create_queue("email-queue").await;
    let result2 = manager.get_or_create_queue("email_queue").await;
    let result3 = manager.get_or_create_queue("email.queue").await;
    
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
    assert_eq!(manager.queue_count(), 3);
}

#[tokio::test]
async fn test_manager_stats_summary() {
    let manager = QueueManager::new(100, 10);
    
    let queue1 = manager.get_or_create_queue("emails").await.unwrap();
    let queue2 = manager.get_or_create_queue("webhooks").await.unwrap();
    
    queue1.enqueue(serde_json::json!({"a": 1})).await.unwrap();
    queue1.enqueue(serde_json::json!({"b": 2})).await.unwrap();
    queue1.dequeue(Duration::from_secs(0)).await.unwrap();
    
    queue2.enqueue(serde_json::json!({"c": 3})).await.unwrap();
    
    let summary = manager.get_stats_summary();
    
    assert_eq!(summary.total_queues, 2);
    assert!(summary.queues.contains_key("emails"));
    assert!(summary.queues.contains_key("webhooks"));
    
    let emails_stats = &summary.queues["emails"];
    assert_eq!(emails_stats.size, 1);
    assert_eq!(emails_stats.enqueued_total, 2);
    assert_eq!(emails_stats.dequeued_total, 1);
}

#[tokio::test]
async fn test_delete_empty_queue() {
    let manager = QueueManager::new(100, 10);
    
    manager.get_or_create_queue("temp").await.unwrap();
    assert_eq!(manager.queue_count(), 1);
    
    let result = manager.delete_queue("temp").await;
    
    assert!(result.is_ok());
    assert_eq!(manager.queue_count(), 0);
    assert!(manager.get_queue("temp").is_none());
}

#[tokio::test]
async fn test_delete_nonempty_queue_fails() {
    let manager = QueueManager::new(100, 10);
    
    let queue = manager.get_or_create_queue("temp").await.unwrap();
    queue.enqueue(serde_json::json!({"data": "test"})).await.unwrap();
    
    let result = manager.delete_queue("temp").await;
    
    assert!(result.is_err());
    assert_eq!(manager.queue_count(), 1);
}

#[tokio::test]
async fn test_delete_nonexistent_queue() {
    let manager = QueueManager::new(100, 10);
    
    let result = manager.delete_queue("nonexistent").await;
    
    assert!(result.is_err());
}

