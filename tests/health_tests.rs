use std::sync::Arc;
use zoldyq::{QueueManager, ConnectionState, handle_command};
use redis_protocol::resp2::types::OwnedFrame as RespFrame;

fn make_cmd(args: &[&str]) -> RespFrame {
    RespFrame::Array(
        args.iter()
            .map(|s| RespFrame::BulkString(s.as_bytes().to_vec()))
            .collect()
    )
}

#[tokio::test]
async fn test_health_command_returns_ok() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };

    let response = handle_command(
        make_cmd(&["HEALTH"]),
        manager.clone(),
        &mut conn_state,
        None,
    ).await;

    match response {
        RespFrame::SimpleString(msg) => {
            assert_eq!(msg, b"OK");
        }
        _ => panic!("Expected OK response, got: {:?}", response),
    }
}

#[tokio::test]
async fn test_info_command_returns_server_section() {
    zoldyq::init_start_time();
    
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };

    let response = handle_command(
        make_cmd(&["INFO", "server"]),
        manager.clone(),
        &mut conn_state,
        None,
    ).await;

    match response {
        RespFrame::BulkString(data) => {
            let info = String::from_utf8_lossy(&data);
            assert!(info.contains("# Server"), "Missing Server section");
            assert!(info.contains("zoldyq_version:"), "Missing version");
            assert!(info.contains("uptime_in_seconds:"), "Missing uptime");
            assert!(info.contains("process_id:"), "Missing process_id");
        }
        _ => panic!("Expected bulk string response, got: {:?}", response),
    }
}

#[tokio::test]
async fn test_info_command_returns_queues_section() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };
    
    manager.enqueue("test_queue", serde_json::json!({"test": 1})).unwrap();

    let response = handle_command(
        make_cmd(&["INFO", "queues"]),
        manager.clone(),
        &mut conn_state,
        None,
    ).await;

    match response {
        RespFrame::BulkString(data) => {
            let info = String::from_utf8_lossy(&data);
            assert!(info.contains("# Queues"), "Missing Queues section");
            assert!(info.contains("queue_count:1"), "Wrong queue count");
            assert!(info.contains("queue_test_queue:"), "Missing queue info");
        }
        _ => panic!("Expected bulk string response, got: {:?}", response),
    }
}

#[tokio::test]
async fn test_info_command_all_sections() {
    zoldyq::init_start_time();
    
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };

    let response = handle_command(
        make_cmd(&["INFO"]),
        manager.clone(),
        &mut conn_state,
        None,
    ).await;

    match response {
        RespFrame::BulkString(data) => {
            let info = String::from_utf8_lossy(&data);
            assert!(info.contains("# Server"), "Missing Server section");
            assert!(info.contains("# Queues"), "Missing Queues section");
            assert!(info.contains("# Memory"), "Missing Memory section");
        }
        _ => panic!("Expected bulk string response, got: {:?}", response),
    }
}

#[tokio::test]
async fn test_ping_command() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };

    let response = handle_command(
        make_cmd(&["PING"]),
        manager.clone(),
        &mut conn_state,
        None,
    ).await;

    match response {
        RespFrame::SimpleString(msg) => {
            assert_eq!(msg, b"PONG");
        }
        _ => panic!("Expected PONG response"),
    }
}

#[tokio::test]
async fn test_ping_command_with_message() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };

    let response = handle_command(
        make_cmd(&["PING", "hello"]),
        manager.clone(),
        &mut conn_state,
        None,
    ).await;

    match response {
        RespFrame::BulkString(msg) => {
            assert_eq!(msg, b"hello");
        }
        _ => panic!("Expected echo response, got: {:?}", response),
    }
}
