use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};
use dashmap::DashMap;

use crate::{AckManager, QueueManager};
use super::handler::{handle_request, ConnectionState};
use super::protocol::{decode_request, encode_frame, Command, Response, ServerPush};

#[derive(Debug, Clone)]
pub struct ZqpConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

impl Default for ZqpConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 6380,
            password: None,
        }
    }
}

type SubscriberTx = mpsc::Sender<ServerPush>;
type Subscriptions = Arc<DashMap<String, Vec<SubscriberTx>>>;

pub struct ZqpServer {
    config: ZqpConfig,
    queue_manager: Arc<QueueManager>,
    ack_manager: Option<Arc<AckManager>>,
    subscriptions: Subscriptions,
    shutdown_rx: broadcast::Receiver<()>,
}

impl ZqpServer {
    pub fn new(
        config: ZqpConfig,
        queue_manager: Arc<QueueManager>,
        ack_manager: Option<Arc<AckManager>>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            config,
            queue_manager,
            ack_manager,
            subscriptions: Arc::new(DashMap::new()),
            shutdown_rx,
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;
        
        tracing::info!("ZQP server listening on {}", addr);

        let subscriptions = self.subscriptions.clone();
        let queue_manager = self.queue_manager.clone();
        let poll_shutdown = self.shutdown_rx.resubscribe();
        
        tokio::spawn(subscription_poller(
            queue_manager,
            subscriptions.clone(),
            self.ack_manager.clone(),
            poll_shutdown,
        ));

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((socket, addr)) => {
                            tracing::debug!("ZQP connection from {}", addr);
                            let qm = self.queue_manager.clone();
                            let am = self.ack_manager.clone();
                            let pw = self.config.password.clone();
                            let subs = self.subscriptions.clone();
                            let shutdown = self.shutdown_rx.resubscribe();
                            
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(socket, qm, am, pw, subs, shutdown).await {
                                    tracing::error!("ZQP connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("ZQP accept error: {}", e);
                        }
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("ZQP server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    queue_manager: Arc<QueueManager>,
    ack_manager: Option<Arc<AckManager>>,
    password: Option<String>,
    subscriptions: Subscriptions,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = vec![0u8; 65536];
    let mut read_buf = Vec::new();
    let mut conn_state = ConnectionState::default();
    
    if password.is_none() {
        conn_state.authenticated = true;
    }

    let (push_tx, mut push_rx) = mpsc::channel::<ServerPush>(100);
    let mut my_subscriptions: Vec<String> = Vec::new();

    loop {
        tokio::select! {
            result = socket.read(&mut buffer) => {
                let n = result?;
                if n == 0 {
                    break;
                }
                
                read_buf.extend_from_slice(&buffer[..n]);
                
                while read_buf.len() >= 4 {
                    let len = u32::from_le_bytes([read_buf[0], read_buf[1], read_buf[2], read_buf[3]]) as usize;
                    
                    if read_buf.len() < 4 + len {
                        break;
                    }
                    
                    let payload = &read_buf[4..4 + len];
                    
                    match decode_request(payload) {
                        Ok(request) => {
                            match request.cmd {
                                Command::Subscribe => {
                                    if let Some(queue) = &request.queue {
                                        subscriptions
                                            .entry(queue.clone())
                                            .or_insert_with(Vec::new)
                                            .push(push_tx.clone());
                                        my_subscriptions.push(queue.clone());
                                        
                                        let response = Response::ok();
                                        let frame = encode_frame(&response)?;
                                        socket.write_all(&frame).await?;
                                    } else {
                                        let response = Response::error("Missing 'queue' field");
                                        let frame = encode_frame(&response)?;
                                        socket.write_all(&frame).await?;
                                    }
                                }
                                Command::Unsubscribe => {
                                    if let Some(queue) = &request.queue {
                                        my_subscriptions.retain(|q| q != queue);
                                        let response = Response::ok();
                                        let frame = encode_frame(&response)?;
                                        socket.write_all(&frame).await?;
                                    } else {
                                        let response = Response::error("Missing 'queue' field");
                                        let frame = encode_frame(&response)?;
                                        socket.write_all(&frame).await?;
                                    }
                                }
                                _ => {
                                    let response = handle_request(
                                        request,
                                        queue_manager.clone(),
                                        &mut conn_state,
                                        password.as_deref(),
                                        ack_manager.clone(),
                                    ).await;
                                    
                                    let frame = encode_frame(&response)?;
                                    socket.write_all(&frame).await?;
                                }
                            }
                        }
                        Err(e) => {
                            let response = Response::error(format!("Invalid request: {}", e));
                            let frame = encode_frame(&response)?;
                            socket.write_all(&frame).await?;
                        }
                    }
                    
                    read_buf.drain(..4 + len);
                }
            }
            
            Some(push) = push_rx.recv() => {
                if my_subscriptions.contains(&push.queue) {
                    let frame = encode_frame(&push)?;
                    socket.write_all(&frame).await?;
                }
            }
            
            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    for queue in &my_subscriptions {
        if let Some(mut subs) = subscriptions.get_mut(queue) {
            subs.retain(|tx| !tx.is_closed());
        }
    }

    Ok(())
}

async fn subscription_poller(
    queue_manager: Arc<QueueManager>,
    subscriptions: Subscriptions,
    ack_manager: Option<Arc<AckManager>>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let poll_interval = Duration::from_millis(10);
    
    loop {
        tokio::select! {
            _ = tokio::time::sleep(poll_interval) => {
                let queues: Vec<String> = subscriptions.iter().map(|e| e.key().clone()).collect();
                
                for queue_name in queues {
                    if let Some(queue) = queue_manager.get_queue(&queue_name) {
                        while let Some(mut message) = queue.try_dequeue() {
                            if let Some(ref ack) = ack_manager {
                                message = ack.track_message(&queue_name, message);
                            }
                            
                            let push = ServerPush::message(
                                message.id,
                                queue_name.clone(),
                                message.payload,
                            );
                            
                            if let Some(subs) = subscriptions.get(&queue_name) {
                                for tx in subs.iter() {
                                    let _ = tx.try_send(push.clone());
                                }
                            }
                        }
                    }
                }
            }
            
            _ = shutdown_rx.recv() => {
                tracing::info!("Subscription poller shutting down");
                break;
            }
        }
    }
}
