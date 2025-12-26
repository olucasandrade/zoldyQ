use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use bytes::{Buf, BytesMut};
use redis_protocol::resp2::decode::decode;
use redis_protocol::resp2::encode::encode;
use redis_protocol::resp2::types::OwnedFrame as RespFrame;

use super::handler::handle_command;
use crate::QueueManager;

#[derive(Debug, Clone)]
pub struct RespConfig {
    pub host: String,
    pub port: u16,
}

impl Default for RespConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 6379,
        }
    }
}

/// RESP Server
pub struct RespServer {
    config: RespConfig,
    queue_manager: Arc<QueueManager>,
}

impl RespServer {
    pub fn new(config: RespConfig, queue_manager: Arc<QueueManager>) -> Self {
        Self {
            config,
            queue_manager,
        }
    }

    pub async fn run(self) -> std::io::Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("ZoldyQ RESP server listening on {}", addr);

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            tracing::info!("📥 New connection from {}", peer_addr);

            let queue_manager = self.queue_manager.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, queue_manager).await {
                    tracing::error!("Connection error: {} (kind: {:?})", e, e.kind());
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    queue_manager: Arc<QueueManager>,
) -> std::io::Result<()> {
    let mut buffer = BytesMut::with_capacity(4096);

    loop {
        let n = socket.read_buf(&mut buffer).await?;

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

                    let response = handle_command(frame, queue_manager.clone()).await;

                    tracing::debug!("Sending response: {:?}", response);

                    // Encode and send response
                    // Allocate a large enough buffer
                    let mut response_bytes = vec![0u8; 65536]; // 64KB buffer
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
                        // Need more data, break and read more
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
}
