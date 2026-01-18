use std::sync::Arc;
use std::time::Duration;
use zoldyq::{QueueManager, RespServer, RespConfig};

#[tokio::test]
async fn test_server_stops_on_shutdown_signal() {
    let queue_manager = Arc::new(QueueManager::new(100, 10));
    let config = RespConfig {
        host: "127.0.0.1".to_string(),
        port: 16379,
        password: None,
    };

    let server = RespServer::new(config, queue_manager);
    let shutdown_tx = server.shutdown_signal();

    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let _ = shutdown_tx.send(());

    let result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
    
    assert!(result.is_ok(), "Server should stop within 2 seconds");
    assert!(result.unwrap().is_ok(), "Server should stop cleanly");
}

#[tokio::test]
async fn test_server_stats_tracking() {
    let queue_manager = Arc::new(QueueManager::new(100, 10));
    let config = RespConfig {
        host: "127.0.0.1".to_string(),
        port: 16380,
        password: None,
    };

    let server = RespServer::new(config, queue_manager);
    let stats = server.stats();
    let shutdown_tx = server.shutdown_signal();

    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(stats.connected_clients.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(stats.total_connections.load(std::sync::atomic::Ordering::SeqCst), 0);

    let _ = shutdown_tx.send(());
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_multiple_shutdown_signals_are_safe() {
    let queue_manager = Arc::new(QueueManager::new(100, 10));
    let config = RespConfig {
        host: "127.0.0.1".to_string(),
        port: 16381,
        password: None,
    };

    let server = RespServer::new(config, queue_manager);
    let shutdown_tx = server.shutdown_signal();

    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let _ = shutdown_tx.send(());
    let _ = shutdown_tx.send(());
    let _ = shutdown_tx.send(());

    let result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
    assert!(result.is_ok(), "Server should handle multiple shutdown signals");
}
