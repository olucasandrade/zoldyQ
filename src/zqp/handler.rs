use std::sync::Arc;
use std::time::Duration;

use crate::{AckManager, QueueManager};
use super::protocol::{Command, Request, Response};

pub struct ConnectionState {
    pub authenticated: bool,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self { authenticated: false }
    }
}

pub async fn handle_request(
    request: Request,
    queue_manager: Arc<QueueManager>,
    conn_state: &mut ConnectionState,
    password: Option<&str>,
    ack_manager: Option<Arc<AckManager>>,
) -> Response {
    if !conn_state.authenticated && !matches!(request.cmd, Command::Auth | Command::Ping) {
        if password.is_some() {
            return Response::error("NOAUTH Authentication required");
        } else {
            conn_state.authenticated = true;
        }
    }

    match request.cmd {
        Command::Auth => handle_auth(&request, conn_state, password),
        Command::Ping => handle_ping(&request),
        Command::Push => handle_push(&request, queue_manager).await,
        Command::Pop => handle_pop(&request, queue_manager, ack_manager.as_ref()).await,
        Command::Ack => handle_ack(&request, ack_manager.as_ref()),
        Command::Nack => handle_nack(&request, ack_manager.as_ref()),
        Command::Len => handle_len(&request, queue_manager).await,
        Command::Del => handle_del(&request, queue_manager).await,
        Command::Subscribe | Command::Unsubscribe => {
            Response::error("Use streaming API for subscribe/unsubscribe")
        }
    }
}

fn handle_auth(request: &Request, conn_state: &mut ConnectionState, password: Option<&str>) -> Response {
    let provided = request.password.as_deref().unwrap_or("");
    
    match password {
        Some(expected) if expected == provided => {
            conn_state.authenticated = true;
            Response::ok()
        }
        Some(_) => Response::error("WRONGPASS invalid password"),
        None => {
            conn_state.authenticated = true;
            Response::ok()
        }
    }
}

fn handle_ping(request: &Request) -> Response {
    let pong = request.payload
        .as_ref()
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "PONG".to_string());
    
    Response::ok().with_pong(pong)
}

async fn handle_push(request: &Request, queue_manager: Arc<QueueManager>) -> Response {
    let queue = match &request.queue {
        Some(q) => q,
        None => return Response::error("Missing 'queue' field"),
    };
    
    let payload = match &request.payload {
        Some(p) => p.clone(),
        None => return Response::error("Missing 'payload' field"),
    };
    
    match queue_manager.enqueue(queue, payload) {
        Ok(message) => {
            let len = queue_manager
                .get_queue(queue)
                .map(|q| q.size() as u64)
                .unwrap_or(0);
            Response::ok().with_id(message.id).with_length(len)
        }
        Err(e) => Response::error(e.to_string()),
    }
}

async fn handle_pop(
    request: &Request,
    queue_manager: Arc<QueueManager>,
    ack_manager: Option<&Arc<AckManager>>,
) -> Response {
    let queue = match &request.queue {
        Some(q) => q,
        None => return Response::error("Missing 'queue' field"),
    };
    
    let timeout = Duration::from_secs(request.timeout.unwrap_or(0));
    
    match queue_manager.dequeue(queue, timeout).await {
        Ok(Some(mut message)) => {
            if let Some(ack) = ack_manager {
                message = ack.track_message(queue, message);
            }
            Response::ok()
                .with_id(message.id)
                .with_queue(queue.clone())
                .with_payload(message.payload)
        }
        Ok(None) => Response::ok(),
        Err(e) => Response::error(e.to_string()),
    }
}

fn handle_ack(request: &Request, ack_manager: Option<&Arc<AckManager>>) -> Response {
    let ack = match ack_manager {
        Some(a) => a,
        None => return Response::error("ACK mode not enabled"),
    };
    
    let id = match &request.id {
        Some(i) => i,
        None => return Response::error("Missing 'id' field"),
    };
    
    match ack.ack(id) {
        Ok(()) => Response::ok(),
        Err(e) => Response::error(e),
    }
}

fn handle_nack(request: &Request, ack_manager: Option<&Arc<AckManager>>) -> Response {
    let ack = match ack_manager {
        Some(a) => a,
        None => return Response::error("ACK mode not enabled"),
    };
    
    let id = match &request.id {
        Some(i) => i,
        None => return Response::error("Missing 'id' field"),
    };
    
    match ack.nack(id) {
        Ok(()) => Response::ok(),
        Err(e) => Response::error(e),
    }
}

async fn handle_len(request: &Request, queue_manager: Arc<QueueManager>) -> Response {
    let queue = match &request.queue {
        Some(q) => q,
        None => return Response::error("Missing 'queue' field"),
    };
    
    match queue_manager.get_queue(queue) {
        Some(q) => Response::ok().with_length(q.size() as u64),
        None => Response::ok().with_length(0),
    }
}

async fn handle_del(request: &Request, queue_manager: Arc<QueueManager>) -> Response {
    let queue = match &request.queue {
        Some(q) => q,
        None => return Response::error("Missing 'queue' field"),
    };
    
    let deleted = queue_manager.delete_queue(queue);
    Response::ok().with_length(if deleted { 1 } else { 0 })
}
