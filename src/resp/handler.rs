use std::{sync::Arc, u64};
use std::time::Duration;
use redis_protocol::resp2::types::OwnedFrame as RespFrame;
use crate::{QueueManager, AckManager, SnapshotManager};

use super::server::ConnectionState;
use super::utils::{extract_string, extract_bytes, extract_integer};

const VERSION: &str = env!("CARGO_PKG_VERSION");
static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub fn init_start_time() {
    START_TIME.get_or_init(std::time::Instant::now);
}

pub async fn handle_command(
    frame: RespFrame,
    queue_manager: Arc<QueueManager>,
    conn_state: &mut ConnectionState,
    password: Option<&str>,
) -> RespFrame {
    handle_command_full(frame, queue_manager, conn_state, password, None, None).await
}

pub async fn handle_command_with_ack(
    frame: RespFrame,
    queue_manager: Arc<QueueManager>,
    conn_state: &mut ConnectionState,
    password: Option<&str>,
    ack_manager: Option<Arc<AckManager>>,
) -> RespFrame {
    handle_command_full(frame, queue_manager, conn_state, password, ack_manager, None).await
}

pub async fn handle_command_full(
    frame: RespFrame,
    queue_manager: Arc<QueueManager>,
    conn_state: &mut ConnectionState,
    password: Option<&str>,
    ack_manager: Option<Arc<AckManager>>,
    snapshot_manager: Option<Arc<SnapshotManager>>,
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

    let no_auth_commands = ["AUTH", "PING", "COMMAND", "QUIT"];
    
    if !conn_state.authenticated && !no_auth_commands.contains(&command_name.as_str()) {
        return RespFrame::Error("NOAUTH Authentication required.".to_string());
    }

    match command_name.as_str() {
        "PING" => handle_ping(&cmd_array),
        "AUTH" => handle_auth(&cmd_array, conn_state, password),
        "INFO" => handle_info(&cmd_array, queue_manager, ack_manager.as_ref(), snapshot_manager.as_ref()),
        "HEALTH" => handle_health(),
        "LPUSH" => handle_lpush(&cmd_array, queue_manager).await,
        "RPOP" => handle_rpop(&cmd_array, queue_manager, ack_manager.as_ref()).await,
        "BRPOP" => handle_brpop(&cmd_array, queue_manager, ack_manager.as_ref()).await,
        "ACK" => handle_ack(&cmd_array, ack_manager.as_ref()),
        "NACK" => handle_nack(&cmd_array, ack_manager.as_ref()),
        "BGSAVE" => handle_bgsave(snapshot_manager.as_ref()),
        "LASTSAVE" => handle_lastsave(snapshot_manager.as_ref()),
        "LLEN" => handle_llen(&cmd_array, queue_manager).await,
        "DEL" => handle_del(&cmd_array, queue_manager).await,
        "COMMAND" => handle_command_docs(),
        "QUIT" => RespFrame::SimpleString(b"OK".to_vec()),
        _ => RespFrame::Error(format!("ERR unknown command '{}'", command_name)),
    }
}

fn handle_auth(cmd: &[RespFrame], conn_state: &mut ConnectionState, password: Option<&str>) -> RespFrame {
    if cmd.len() != 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'auth' command".to_string());
    }

    let provided_password = match extract_string(&cmd[1]) {
        Ok(p) => p,
        Err(e) => return e,
    };

    match password {
        Some(expected) if expected == provided_password => {
            conn_state.authenticated = true;
            RespFrame::SimpleString(b"OK".to_vec())
        }
        Some(_) => {
            RespFrame::Error("WRONGPASS invalid password".to_string())
        }
        None => {
            conn_state.authenticated = true;
            RespFrame::SimpleString(b"OK".to_vec())
        }
    }
}

fn handle_info(
    cmd: &[RespFrame],
    queue_manager: Arc<QueueManager>,
    ack_manager: Option<&Arc<AckManager>>,
    snapshot_manager: Option<&Arc<SnapshotManager>>,
) -> RespFrame {
    let section = if cmd.len() > 1 {
        extract_string(&cmd[1]).ok()
    } else {
        None
    };

    let mut info = String::new();
    let uptime = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);

    let show_all = section.is_none();
    let section_str = section.as_deref().unwrap_or("");

    if show_all || section_str.eq_ignore_ascii_case("server") {
        info.push_str("# Server\r\n");
        info.push_str(&format!("zoldyq_version:{}\r\n", VERSION));
        info.push_str(&format!("uptime_in_seconds:{}\r\n", uptime));
        info.push_str(&format!("process_id:{}\r\n", std::process::id()));
        info.push_str(&format!("ack_enabled:{}\r\n", ack_manager.is_some()));
        if let Some(am) = ack_manager {
            info.push_str(&format!("in_flight_messages:{}\r\n", am.in_flight_count()));
        }
        info.push_str("\r\n");
    }

    if show_all || section_str.eq_ignore_ascii_case("queues") {
        info.push_str("# Queues\r\n");
        info.push_str(&format!("queue_count:{}\r\n", queue_manager.queue_count()));
        info.push_str(&format!("max_queues:{}\r\n", queue_manager.max_queues()));
        info.push_str(&format!("default_capacity:{}\r\n", queue_manager.default_capacity()));
        
        let queues = queue_manager.list_queues();
        for entry in queues.iter() {
            let name = entry.key();
            let queue = entry.value();
            info.push_str(&format!(
                "queue_{}:size={},enqueued={},dequeued={}\r\n",
                name,
                queue.size(),
                queue.stats().enqueued_total(),
                queue.stats().dequeued_total()
            ));
        }
        info.push_str("\r\n");
    }

    if show_all || section_str.eq_ignore_ascii_case("persistence") {
        info.push_str("# Persistence\r\n");
        info.push_str(&format!("persistence_enabled:{}\r\n", snapshot_manager.is_some()));
        if let Some(sm) = snapshot_manager {
            info.push_str(&format!("last_save:{}\r\n", sm.last_save_timestamp()));
        }
        info.push_str("\r\n");
    }

    if show_all || section_str.eq_ignore_ascii_case("memory") {
        info.push_str("# Memory\r\n");
        info.push_str("used_memory:0\r\n");
        info.push_str("\r\n");
    }

    RespFrame::BulkString(info.into_bytes())
}

fn handle_bgsave(snapshot_manager: Option<&Arc<SnapshotManager>>) -> RespFrame {
    let Some(sm) = snapshot_manager else {
        return RespFrame::Error("ERR persistence not enabled".to_string());
    };

    match sm.save() {
        Ok(()) => RespFrame::SimpleString(b"Background saving started".to_vec()),
        Err(e) => RespFrame::Error(format!("ERR {}", e)),
    }
}

fn handle_lastsave(snapshot_manager: Option<&Arc<SnapshotManager>>) -> RespFrame {
    let Some(sm) = snapshot_manager else {
        return RespFrame::Error("ERR persistence not enabled".to_string());
    };

    RespFrame::Integer(sm.last_save_timestamp() as i64)
}

fn handle_health() -> RespFrame {
    RespFrame::SimpleString(b"OK".to_vec())
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

async fn handle_rpop(cmd: &[RespFrame], queue_manager: Arc<QueueManager>, ack_manager: Option<&Arc<AckManager>>) -> RespFrame {
    if cmd.len() != 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'rpop' command".to_string());
    }

    let queue_name = match extract_string(&cmd[1]) {
        Ok(name) => name,
        Err(e) => return e,
    };

    match queue_manager.dequeue(&queue_name, Duration::from_secs(0)).await {
        Ok(Some(message)) => {
            if let Some(am) = ack_manager {
                let tracked = am.track_message(&queue_name, message);
                RespFrame::Array(vec![
                    RespFrame::BulkString(tracked.id.clone().into_bytes()),
                    RespFrame::BulkString(tracked.payload.to_string().into_bytes()),
                ])
            } else {
                RespFrame::BulkString(message.payload.to_string().into_bytes())
            }
        }
        Ok(None) => RespFrame::Null,
        Err(e) => RespFrame::Error(format!("ERR {}", e).into()),
    }
}

async fn handle_brpop(cmd: &[RespFrame], queue_manager: Arc<QueueManager>, ack_manager: Option<&Arc<AckManager>>) -> RespFrame {
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
            if let Some(am) = ack_manager {
                let tracked = am.track_message(&queue_name, message);
                RespFrame::Array(vec![
                    RespFrame::BulkString(queue_name.into_bytes()),
                    RespFrame::BulkString(tracked.id.clone().into_bytes()),
                    RespFrame::BulkString(tracked.payload.to_string().into_bytes()),
                ])
            } else {
                RespFrame::Array(vec![
                    RespFrame::BulkString(queue_name.into_bytes()),
                    RespFrame::BulkString(message.payload.to_string().into_bytes()),
                ])
            }
        }
        Ok(None) => RespFrame::Null,
        Err(e) => RespFrame::Error(format!("ERR {}", e).into()),
    }
}

fn handle_ack(cmd: &[RespFrame], ack_manager: Option<&Arc<AckManager>>) -> RespFrame {
    let Some(am) = ack_manager else {
        return RespFrame::Error("ERR ACK not enabled".to_string());
    };

    if cmd.len() != 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'ack' command".to_string());
    }

    let message_id = match extract_string(&cmd[1]) {
        Ok(id) => id,
        Err(e) => return e,
    };

    match am.ack(&message_id) {
        Ok(()) => RespFrame::SimpleString(b"OK".to_vec()),
        Err(e) => RespFrame::Error(format!("ERR {}", e)),
    }
}

fn handle_nack(cmd: &[RespFrame], ack_manager: Option<&Arc<AckManager>>) -> RespFrame {
    let Some(am) = ack_manager else {
        return RespFrame::Error("ERR NACK not enabled".to_string());
    };

    if cmd.len() != 2 {
        return RespFrame::Error("ERR wrong number of arguments for 'nack' command".to_string());
    }

    let message_id = match extract_string(&cmd[1]) {
        Ok(id) => id,
        Err(e) => return e,
    };

    match am.nack(&message_id) {
        Ok(()) => RespFrame::SimpleString(b"OK".to_vec()),
        Err(e) => RespFrame::Error(format!("ERR {}", e)),
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

fn handle_command_docs() -> RespFrame {
    RespFrame::Array(vec![
        RespFrame::BulkString(b"PING".to_vec()),
        RespFrame::BulkString(b"AUTH".to_vec()),
        RespFrame::BulkString(b"INFO".to_vec()),
        RespFrame::BulkString(b"HEALTH".to_vec()),
        RespFrame::BulkString(b"LPUSH".to_vec()),
        RespFrame::BulkString(b"RPOP".to_vec()),
        RespFrame::BulkString(b"BRPOP".to_vec()),
        RespFrame::BulkString(b"ACK".to_vec()),
        RespFrame::BulkString(b"NACK".to_vec()),
        RespFrame::BulkString(b"BGSAVE".to_vec()),
        RespFrame::BulkString(b"LASTSAVE".to_vec()),
        RespFrame::BulkString(b"LLEN".to_vec()),
        RespFrame::BulkString(b"DEL".to_vec()),
    ])
}
