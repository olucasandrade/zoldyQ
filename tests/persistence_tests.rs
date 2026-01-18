use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use zoldyq::{QueueManager, SnapshotManager, SnapshotConfig};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_snapshot_path() -> String {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("/tmp/zoldyq_test_{}_{}.rdb", std::process::id(), id)
}

#[tokio::test]
async fn test_snapshot_and_restore() {
    let path = temp_snapshot_path();
    
    let qm1 = Arc::new(QueueManager::new(100, 10));
    qm1.enqueue("tasks", serde_json::json!({"id": 1})).unwrap();
    qm1.enqueue("tasks", serde_json::json!({"id": 2})).unwrap();
    qm1.enqueue("events", serde_json::json!({"type": "test"})).unwrap();

    let config = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm1 = SnapshotManager::new(config, qm1.clone());
    sm1.save().unwrap();

    let qm2 = Arc::new(QueueManager::new(100, 10));
    let config2 = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm2 = SnapshotManager::new(config2, qm2.clone());
    sm2.load().unwrap();

    assert_eq!(qm2.queue_count(), 2);

    let msg1 = qm2.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    assert_eq!(msg1.payload, serde_json::json!({"id": 1}));

    let msg2 = qm2.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    assert_eq!(msg2.payload, serde_json::json!({"id": 2}));

    let msg3 = qm2.dequeue("events", Duration::from_secs(0)).await.unwrap().unwrap();
    assert_eq!(msg3.payload, serde_json::json!({"type": "test"}));

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn test_snapshot_updates_lastsave() {
    let path = temp_snapshot_path();
    
    let qm = Arc::new(QueueManager::new(100, 10));
    let config = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm = SnapshotManager::new(config, qm.clone());

    assert_eq!(sm.last_save_timestamp(), 0);

    sm.save().unwrap();

    assert!(sm.last_save_timestamp() > 0);

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn test_load_nonexistent_snapshot_succeeds() {
    let qm = Arc::new(QueueManager::new(100, 10));
    let config = SnapshotConfig {
        path: "/tmp/nonexistent_zoldyq_snapshot.rdb".to_string(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm = SnapshotManager::new(config, qm.clone());

    let result = sm.load();
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_snapshot_preserves_message_metadata() {
    let path = temp_snapshot_path();
    
    let qm1 = Arc::new(QueueManager::new(100, 10));
    qm1.enqueue("tasks", serde_json::json!({"test": true})).unwrap();

    let config = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm1 = SnapshotManager::new(config, qm1.clone());
    sm1.save().unwrap();

    let qm2 = Arc::new(QueueManager::new(100, 10));
    let config2 = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm2 = SnapshotManager::new(config2, qm2.clone());
    sm2.load().unwrap();

    let msg = qm2.dequeue("tasks", Duration::from_secs(0)).await.unwrap().unwrap();
    
    assert!(!msg.id.is_empty());
    assert!(msg.enqueued_at > 0);
    assert_eq!(msg.delivery_count, 0);

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn test_corrupted_snapshot_returns_error() {
    let path = temp_snapshot_path();
    
    std::fs::write(&path, b"corrupted data").unwrap();

    let qm = Arc::new(QueueManager::new(100, 10));
    let config = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm = SnapshotManager::new(config, qm.clone());

    let result = sm.load();
    assert!(result.is_err());

    std::fs::remove_file(&path).ok();
}

#[tokio::test]
async fn test_empty_queues_snapshot() {
    let path = temp_snapshot_path();
    
    let qm = Arc::new(QueueManager::new(100, 10));

    let config = SnapshotConfig {
        path: path.clone(),
        interval_secs: 0,
        on_shutdown: false,
    };
    let sm = SnapshotManager::new(config, qm.clone());
    
    let result = sm.save();
    assert!(result.is_ok());

    std::fs::remove_file(&path).ok();
}
