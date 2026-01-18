use std::time::Duration;
use zoldyq::Message;

#[tokio::test]
async fn test_message_with_ttl_expires() {
    let msg = Message::with_ttl(serde_json::json!({"data": 1}), Some(0));
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    assert!(msg.is_expired());
}

#[tokio::test]
async fn test_message_without_ttl_never_expires() {
    let msg = Message::new(serde_json::json!({"data": 1}));
    
    assert!(!msg.is_expired());
    assert!(msg.expires_at.is_none());
}

#[tokio::test]
async fn test_message_with_future_ttl_not_expired() {
    let msg = Message::with_ttl(serde_json::json!({"data": 1}), Some(3600)); // 1 hour
    
    assert!(!msg.is_expired());
    assert!(msg.expires_at.is_some());
}

#[tokio::test]
async fn test_message_has_id() {
    let msg = Message::new(serde_json::json!({"data": 1}));
    
    assert!(!msg.id.is_empty());
    assert!(msg.id.len() > 10); // UUID should be reasonably long
}

#[tokio::test]
async fn test_message_delivery_count_starts_at_zero() {
    let msg = Message::new(serde_json::json!({"data": 1}));
    
    assert_eq!(msg.delivery_count, 0);
}

#[tokio::test]
async fn test_message_increment_delivery() {
    let mut msg = Message::new(serde_json::json!({"data": 1}));
    
    assert_eq!(msg.delivery_count, 0);
    msg.increment_delivery();
    assert_eq!(msg.delivery_count, 1);
    msg.increment_delivery();
    assert_eq!(msg.delivery_count, 2);
}

#[tokio::test]
async fn test_message_enqueued_at_set() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() * 1_000_000;
    
    let msg = Message::new(serde_json::json!({"data": 1}));
    
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() * 1_000_000 + 1_000_000;
    
    assert!(msg.enqueued_at >= before);
    assert!(msg.enqueued_at <= after);
}

#[tokio::test]
async fn test_expires_at_calculated_from_ttl() {
    let ttl_seconds = 60u64;
    let msg = Message::with_ttl(serde_json::json!({"data": 1}), Some(ttl_seconds));
    
    let expected_expires = msg.enqueued_at + (ttl_seconds * 1_000_000);
    assert_eq!(msg.expires_at, Some(expected_expires));
}

#[tokio::test]
async fn test_unique_message_ids() {
    let msg1 = Message::new(serde_json::json!({"data": 1}));
    let msg2 = Message::new(serde_json::json!({"data": 1}));
    let msg3 = Message::new(serde_json::json!({"data": 1}));
    
    assert_ne!(msg1.id, msg2.id);
    assert_ne!(msg2.id, msg3.id);
    assert_ne!(msg1.id, msg3.id);
}
