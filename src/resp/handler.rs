use std::{sync::Arc, u64};
use std::time::Duration;
use redis_protocol::resp2::types::OwnedFrame as RespFrame;
use crate::QueueManager;

use super::utils::{extract_string, extract_bytes, extract_integer};

pub async fn handle_command(
    frame: RespFrame,
    queue_manager: Arc<QueueManager>,
) -> RespFrame {
    let cmd_array = match frame {
        RespFrame::Array(arr) => arr,
        _ => {
            return RespFrame::Error("ERR expected array".into());
        }
    };

    if cmd_array.is_empty() {
        return RespFrame::Error("ERR empty command".to_string());
    }

    let command_name = match &cmd_array[0] {
        RespFrame::BulkString(data) | RespFrame::SimpleString(data) => {
            String::from_utf8_lossy(data).to_uppercase()
        }
        _ => {
            return RespFrame::Error("ERR invalid command format".to_string());
        }
    };

    match command_name.as_str() {
        "PING" => handle_ping(&cmd_array),
        "LPUSH" => handle_lpush(&cmd_array, queue_manager).await,
        "RPOP" => handle_rpop(&cmd_array, queue_manager).await,
        "BRPOP" => handle_brpop(&cmd_array, queue_manager).await,
        "LLEN" => handle_llen(&cmd_array, queue_manager).await,
        "DEL" => handle_del(&cmd_array, queue_manager).await,
        "COMMAND" => handle_command_docs(),
        _ => RespFrame::Error(format!("ERR unknown command '{}'", command_name)),
    }
}

/// PING - Test connection
fn handle_ping(cmd: &[RespFrame]) -> RespFrame {
    if cmd.len() == 1 {
        RespFrame::SimpleString(b"PONG".to_vec())
    } else if cmd.len() == 2 {
        cmd[1].clone()
    } else {
        RespFrame::Error("ERR wrong number of arguments for 'ping' command".to_string())
    }
}

/// LPUSH queue value [value ...] - Push to queue (enqueue)
async fn handle_lpush(cmd: &[RespFrame], queue_manager: Arc<QueueManager>) -> RespFrame {
    if cmd.len() < 3 {
        return RespFrame::Error("ERR wrong number of arguments for 'lpush' command".to_string());
    }

    let queue_name = match extract_string(&cmd[1]) {
        Ok(name) => name,
        Err(e) => return e,
    };

    let mut count = 0;
    for i in 2..cmd.len() {
        let value = match extract_bytes(&cmd[i]) {
            Ok(data) => data,
            Err(e) => return e,
        };

        let json_value = match serde_json::from_slice::<serde_json::Value>(&value) {
            Ok(v) => v,
            Err(_) => {
                serde_json::Value::String(String::from_utf8_lossy(&value).to_string())
            }
        };

        match queue_manager.enqueue(&queue_name, json_value) {
            Ok(_) => count += 1,
            Err(e) => {
                return RespFrame::Error(format!("ERR {}", e));
            }
        }
    }

    RespFrame::Integer(count)
}

/// RPOP queue - Pop from queue (dequeue, non-blocking)
async fn handle_rpop(cmd: &[RespFrame], queue_manager: Arc<QueueManager>) -> RespFrame {
    if cmd.len() != 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'rpop' command".to_string());
    }

    let queue_name = match extract_string(&cmd[1]) {
        Ok(name) => name,
        Err(e) => return e,
    };

    match queue_manager.dequeue(&queue_name, Duration::from_secs(0)).await {
        Ok(Some(message)) => {
            let json_str = message.payload.to_string();
            RespFrame::BulkString(json_str.into_bytes())
        }
        Ok(None) => RespFrame::Null,
        Err(e) => RespFrame::Error(format!("ERR {}", e).into()),
    }
}

/// BRPOP queue timeout - Pop from queue (blocking)
async fn handle_brpop(cmd: &[RespFrame], queue_manager: Arc<QueueManager>) -> RespFrame {
    if cmd.len() < 3 {
        return RespFrame::Error("ERR wrong number of arguments for 'brpop' command".to_string());
    }

    let queue_name = match extract_string(&cmd[1]) {
        Ok(name) => name,
        Err(e) => return e,
    };

    let timeout_secs = match extract_integer(&cmd[2]) {
        Ok(n) => n,
        Err(e) => return e,
    };

    if timeout_secs < 0 {
        return RespFrame::Error("ERR timeout must be non-negative".to_string());
    }

    let timeout = if timeout_secs == 0 {
        Duration::from_secs(u64::MAX)
    } else {
        Duration::from_secs(timeout_secs as u64)
    };

    match queue_manager.dequeue(&queue_name, timeout).await {
        Ok(Some(message)) => {
            let json_str = message.payload.to_string();
            RespFrame::Array(vec![
                RespFrame::BulkString(queue_name.into_bytes()),
                RespFrame::BulkString(json_str.into_bytes()),
            ])
        }
        Ok(None) => RespFrame::Null,
        Err(e) => RespFrame::Error(format!("ERR {}", e).into()),
    }
}

/// LLEN queue - Get queue length
async fn handle_llen(cmd: &[RespFrame], queue_manager: Arc<QueueManager>) -> RespFrame {
    if cmd.len() != 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'llen' command".to_string());
    }

    let queue_name = match extract_string(&cmd[1]) {
        Ok(name) => name,
        Err(e) => return e,
    };

    match queue_manager.get_or_create_queue(&queue_name) {
        Ok(queue) => RespFrame::Integer(queue.size() as i64),
        Err(e) => RespFrame::Error(format!("ERR {}", e).into()),
    }
}

/// DEL queue [queue ...] - Delete queues
async fn handle_del(cmd: &[RespFrame], queue_manager: Arc<QueueManager>) -> RespFrame {
    if cmd.len() < 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'del' command".to_string());
    }

    let mut count = 0;
    for i in 1..cmd.len() {
        let queue_name = match extract_string(&cmd[i]) {
            Ok(name) => name,
            Err(e) => return e,
        };

        if queue_manager.delete_queue(&queue_name) {
            count += 1;
        }
    }

    RespFrame::Integer(count)
}

/// COMMAND - Return supported commands
fn handle_command_docs() -> RespFrame {
    RespFrame::Array(vec![
        RespFrame::BulkString(b"PING".to_vec()),
        RespFrame::BulkString(b"LPUSH".to_vec()),
        RespFrame::BulkString(b"RPOP".to_vec()),
        RespFrame::BulkString(b"BRPOP".to_vec()),
        RespFrame::BulkString(b"LLEN".to_vec()),
        RespFrame::BulkString(b"DEL".to_vec()),
    ])
}
