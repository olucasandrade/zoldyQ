use std::sync::Arc;
use zoldyq::{QueueManager, ConnectionState};
use zoldyq::resp::handler::handle_command;
use redis_protocol::resp2::types::OwnedFrame as RespFrame;

fn make_cmd(args: &[&str]) -> RespFrame {
    RespFrame::Array(
        args.iter()
            .map(|s| RespFrame::BulkString(s.as_bytes().to_vec()))
            .collect()
    )
}

#[tokio::test]
async fn test_commands_rejected_without_auth() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: false };
    let password = Some("secret");

    let response = handle_command(
        make_cmd(&["LPUSH", "queue", "value"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    match response {
        RespFrame::Error(msg) => {
            assert!(msg.contains("NOAUTH"), "Expected NOAUTH error, got: {}", msg);
        }
        _ => panic!("Expected error response"),
    }
}

#[tokio::test]
async fn test_auth_with_correct_password() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: false };
    let password = Some("secret");

    let response = handle_command(
        make_cmd(&["AUTH", "secret"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    match response {
        RespFrame::SimpleString(msg) => {
            assert_eq!(msg, b"OK");
        }
        _ => panic!("Expected OK response"),
    }
    
    assert!(conn_state.authenticated);
}

#[tokio::test]
async fn test_auth_with_wrong_password() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: false };
    let password = Some("secret");

    let response = handle_command(
        make_cmd(&["AUTH", "wrong"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    match response {
        RespFrame::Error(msg) => {
            assert!(msg.contains("WRONGPASS"), "Expected WRONGPASS error, got: {}", msg);
        }
        _ => panic!("Expected error response"),
    }
    
    assert!(!conn_state.authenticated);
}

#[tokio::test]
async fn test_commands_work_after_auth() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: false };
    let password = Some("secret");

    handle_command(
        make_cmd(&["AUTH", "secret"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    let response = handle_command(
        make_cmd(&["LPUSH", "queue", "value"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    match response {
        RespFrame::Integer(n) => assert_eq!(n, 1),
        _ => panic!("Expected integer response"),
    }
}

#[tokio::test]
async fn test_ping_allowed_without_auth() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: false };
    let password = Some("secret");

    let response = handle_command(
        make_cmd(&["PING"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    match response {
        RespFrame::SimpleString(msg) => {
            assert_eq!(msg, b"PONG");
        }
        _ => panic!("Expected PONG response"),
    }
}

#[tokio::test]
async fn test_no_auth_required_when_password_not_set() {
    let manager = Arc::new(QueueManager::new(100, 10));
    let mut conn_state = ConnectionState { authenticated: true };
    let password: Option<&str> = None;

    let response = handle_command(
        make_cmd(&["LPUSH", "queue", "value"]),
        manager.clone(),
        &mut conn_state,
        password,
    ).await;

    match response {
        RespFrame::Integer(n) => assert_eq!(n, 1),
        _ => panic!("Expected integer response"),
    }
}
