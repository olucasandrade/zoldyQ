use std::sync::Arc;
use zoldyq::{RespServer, RespConfig, ZqpServer, ZqpConfig, QueueManager, AckManager, SnapshotManager, SnapshotConfig, init_start_time};

mod http;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    init_start_time();

    let host = std::env::var("ZOLDYQ_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("ZOLDYQ_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6379);

    let http_port = std::env::var("ZOLDYQ_HTTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let zqp_port = std::env::var("ZOLDYQ_ZQP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6380);

    let queue_capacity = std::env::var("ZOLDYQ_QUEUE_CAPACITY")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(100_000);

    let max_queues = std::env::var("ZOLDYQ_MAX_QUEUES")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or(1000);

    let password = std::env::var("ZOLDYQ_PASSWORD").ok();

    let visibility_timeout = std::env::var("ZOLDYQ_VISIBILITY_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let max_retries = std::env::var("ZOLDYQ_MAX_RETRIES")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or(3);

    let ack_enabled = std::env::var("ZOLDYQ_ACK_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let snapshot_path = std::env::var("ZOLDYQ_SNAPSHOT_PATH")
        .unwrap_or_else(|_| "./data/snapshot.rdb".to_string());
    
    let snapshot_interval = std::env::var("ZOLDYQ_SNAPSHOT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    
    let snapshot_on_shutdown = std::env::var("ZOLDYQ_SNAPSHOT_ON_SHUTDOWN")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    let queue_manager = Arc::new(QueueManager::new(queue_capacity, max_queues));

    let config = RespConfig {
        host: host.clone(),
        port,
        password: password.clone(),
    };

    let mut server = RespServer::new(config, queue_manager.clone());
    let shutdown_tx = server.shutdown_signal();
    let stats = server.stats();

    // Initialize persistence if enabled
    if snapshot_interval > 0 || snapshot_on_shutdown {
        let snapshot_config = SnapshotConfig {
            path: snapshot_path,
            interval_secs: snapshot_interval,
            on_shutdown: snapshot_on_shutdown,
        };
        
        let snapshot_manager = Arc::new(SnapshotManager::new(snapshot_config.clone(), queue_manager.clone()));
        
        if let Err(e) = snapshot_manager.load() {
            tracing::error!("Failed to load snapshot: {}", e);
        }
        
        let scheduler_sm = snapshot_manager.clone();
        let scheduler_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            scheduler_sm.start_scheduler(scheduler_shutdown).await;
        });

        server = server.with_snapshot_manager(snapshot_manager);
        tracing::info!("Persistence enabled (interval={}s, on_shutdown={})", snapshot_interval, snapshot_on_shutdown);
    }

    // Initialize ACK manager if enabled
    let ack_manager = if ack_enabled {
        let ack_manager = Arc::new(AckManager::new(
            queue_manager.clone(),
            visibility_timeout,
            max_retries,
        ));
        
        let reaper_ack = ack_manager.clone();
        let reaper_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            reaper_ack.start_reaper(reaper_shutdown).await;
        });

        server = server.with_ack_manager(ack_manager.clone());
        tracing::info!("ACK mode enabled (visibility_timeout={}s, max_retries={})", visibility_timeout, max_retries);
        Some(ack_manager)
    } else {
        None
    };

    // Spawn ZQP server (native ZoldyQ protocol)
    let zqp_config = ZqpConfig {
        host: host.clone(),
        port: zqp_port,
        password: password.clone(),
    };
    let zqp_shutdown = shutdown_tx.subscribe();
    let zqp_qm = queue_manager.clone();
    let zqp_am = ack_manager.clone();
    tokio::spawn(async move {
        let zqp_server = ZqpServer::new(zqp_config, zqp_qm, zqp_am, zqp_shutdown);
        if let Err(e) = zqp_server.run().await {
            tracing::error!("ZQP server error: {}", e);
        }
    });

    // Spawn HTTP health server
    let http_shutdown_tx = shutdown_tx.clone();
    let http_stats = stats.clone();
    let http_qm = queue_manager.clone();
    let http_host = host.clone();
    tokio::spawn(async move {
        if let Err(e) = http::run_http_server(
            &http_host,
            http_port,
            http_qm,
            http_stats,
            http_shutdown_tx,
        ).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // Spawn signal handler
    let signal_shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("Initiating graceful shutdown...");
        let _ = signal_shutdown_tx.send(());
    });

    server.run().await?;
    tracing::info!("Server shutdown complete");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
