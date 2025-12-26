use std::sync::Arc;
use zoldyq::{RespServer, RespConfig, QueueManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // Get configuration from environment variables
    let host = std::env::var("ZOLDYQ_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("ZOLDYQ_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6379);

    let queue_capacity = std::env::var("ZOLDYQ_QUEUE_CAPACITY")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(100_000);

    let max_queues = std::env::var("ZOLDYQ_MAX_QUEUES")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or(1000);

    let queue_manager = Arc::new(QueueManager::new(queue_capacity, max_queues));

    // Create RESP server config
    let config = RespConfig {
        host,
        port,
    };

    // Create and run RESP server
    let server = RespServer::new(config, queue_manager);
    server.run().await?;

    Ok(())
}
