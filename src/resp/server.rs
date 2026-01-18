use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use bytes::{Buf, BytesMut};
use redis_protocol::resp2::decode::decode;
use redis_protocol::resp2::encode::encode;
use redis_protocol::resp2::types::OwnedFrame as RespFrame;

use super::handler::handle_command_full;
use crate::{QueueManager, AckManager, SnapshotManager};

#[derive(Debug, Clone)]
pub struct RespConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

impl Default for RespConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 6379,
            password: None,
        }
    }
}

pub struct ServerStats {
    pub connected_clients: AtomicU64,
    pub total_connections: AtomicU64,
    pub total_commands: AtomicU64,
}

impl ServerStats {
    pub fn new() -> Self {
        Self {
            connected_clients: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
            total_commands: AtomicU64::new(0),
        }
    }
}

impl Default for ServerStats {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RespServer {
    config: RespConfig,
    queue_manager: Arc<QueueManager>,
    ack_manager: Option<Arc<AckManager>>,
    snapshot_manager: Option<Arc<SnapshotManager>>,
    stats: Arc<ServerStats>,
    shutdown_tx: broadcast::Sender<()>,
}

impl RespServer {
    pub fn new(config: RespConfig, queue_manager: Arc<QueueManager>) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config,
            queue_manager,
            ack_manager: None,
            snapshot_manager: None,
            stats: Arc::new(ServerStats::new()),
            shutdown_tx,
        }
    }

    pub fn with_ack_manager(mut self, ack_manager: Arc<AckManager>) -> Self {
        self.ack_manager = Some(ack_manager);
        self
    }

    pub fn with_snapshot_manager(mut self, snapshot_manager: Arc<SnapshotManager>) -> Self {
        self.snapshot_manager = Some(snapshot_manager);
        self
    }

    pub fn stats(&self) -> Arc<ServerStats> {
        self.stats.clone()
    }

    pub fn shutdown_signal(&self) -> broadcast::Sender<()> {
        self.shutdown_tx.clone()
    }

    pub async fn run(self) -> std::io::Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tracing::info!("ZoldyQ RESP server listening on {}", addr);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (socket, peer_addr) = result?;
                    tracing::debug!("New connection from {}", peer_addr);

                    self.stats.connected_clients.fetch_add(1, Ordering::SeqCst);
                    self.stats.total_connections.fetch_add(1, Ordering::SeqCst);

                    let queue_manager = self.queue_manager.clone();
                    let ack_manager = self.ack_manager.clone();
                    let snapshot_manager = self.snapshot_manager.clone();
                    let stats = self.stats.clone();
                    let password = self.config.password.clone();
                    let mut conn_shutdown_rx = self.shutdown_tx.subscribe();

                    tokio::spawn(async move {
                        let result = handle_connection(
                            socket,
                            queue_manager,
                            ack_manager,
                            snapshot_manager,
                            stats.clone(),
                            password,
                            &mut conn_shutdown_rx,
                        ).await;
                        
                        stats.connected_clients.fetch_sub(1, Ordering::SeqCst);
                        
                        if let Err(e) = result {
                            if e.kind() != std::io::ErrorKind::ConnectionReset {
                                tracing::error!("Connection error: {} (kind: {:?})", e, e.kind());
                            }
                        }
                    });
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("Shutdown signal received, stopping server");
                    break;
                }
            }
        }

        Ok(())
    }
}

pub struct ConnectionState {
    pub authenticated: bool,
}

async fn handle_connection(
    mut socket: TcpStream,
    queue_manager: Arc<QueueManager>,
    ack_manager: Option<Arc<AckManager>>,
    snapshot_manager: Option<Arc<SnapshotManager>>,
    stats: Arc<ServerStats>,
    password: Option<String>,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> std::io::Result<()> {
    let mut buffer = BytesMut::with_capacity(4096);
    let mut conn_state = ConnectionState {
        authenticated: password.is_none(),
    };

    loop {
        tokio::select! {
            result = socket.read_buf(&mut buffer) => {
                let n = result?;

                tracing::debug!("Read {} bytes, buffer len: {}", n, buffer.len());

                if n == 0 {
                    if buffer.is_empty() {
                        tracing::debug!("Connection closed by client");
                        return Ok(());
                    }
                }

                loop {
                    match decode(&buffer) {
                        Ok(Some((frame, consumed))) => {
                            tracing::debug!("Received frame: {:?}", frame);

                            buffer.advance(consumed);
                            stats.total_commands.fetch_add(1, Ordering::SeqCst);

                            let response = handle_command_full(
                                frame,
                                queue_manager.clone(),
                                &mut conn_state,
                                password.as_deref(),
                                ack_manager.clone(),
                                snapshot_manager.clone(),
                            ).await;

                            tracing::debug!("Sending response: {:?}", response);

                            let mut response_bytes = vec![0u8; 65536];
                            let written = encode(&mut response_bytes, &response)
                                .map_err(|e| {
                                    tracing::error!("Encode error: {:?}", e);
                                    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{:?}", e))
                                })?;
                            socket.write_all(&response_bytes[..written]).await?;
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(e) => {
                            let err_str = format!("{:?}", e);
                            if err_str.contains("Buffer too small") || err_str.contains("Incomplete") {
                                tracing::debug!("Need more data: {:?}", e);
                                break;
                            }
                            
                            tracing::error!("Parse error: {:?}", e);
                            let error_response = RespFrame::Error(format!("ERR {}", e));
                            let mut response_bytes = vec![0u8; 4096];
                            let written = encode(&mut response_bytes, &error_response)
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{:?}", e)))?;
                            socket.write_all(&response_bytes[..written]).await?;
                            
                            buffer.clear();
                            break;
                        }
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                tracing::debug!("Connection shutdown signal received");
                return Ok(());
            }
        }
    }
}
