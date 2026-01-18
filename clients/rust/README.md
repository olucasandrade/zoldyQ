# ZoldyQ Rust Client

Rust client library for ZoldyQ message queue using the native ZQP protocol.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
zoldyq-client = "0.1"
```

## Quick Start

### Synchronous API

```rust
use zoldyq_client::ZoldyQ;
use serde_json::json;

fn main() -> zoldyq_client::Result<()> {
    let mut client = ZoldyQ::connect("localhost:6380", None)?;

    // Push a message
    let msg_id = client.push("tasks", json!({
        "job": "process-image",
        "id": 123
    }))?;
    println!("Pushed message: {}", msg_id);

    // Pop a message (with 30 second timeout)
    if let Some(msg) = client.pop("tasks", 30)? {
        println!("Got message: {:?}", msg.payload);
        client.ack(&msg.id)?;
    }

    // Check queue length
    let length = client.length("tasks")?;
    println!("Queue length: {}", length);

    Ok(())
}
```

### Asynchronous API

```rust
use zoldyq_client::ZoldyQAsync;
use serde_json::json;

#[tokio::main]
async fn main() -> zoldyq_client::Result<()> {
    let mut client = ZoldyQAsync::connect("localhost:6380", None).await?;

    // Push messages
    client.push("tasks", json!({"job": "task-1"})).await?;
    client.push("tasks", json!({"job": "task-2"})).await?;

    // Pop a message
    if let Some(msg) = client.pop("tasks", 5).await? {
        println!("Processing: {:?}", msg.payload);
        client.ack(&msg.id).await?;
    }

    Ok(())
}
```

### Subscription (Server Push)

```rust
use zoldyq_client::ZoldyQAsync;

#[tokio::main]
async fn main() -> zoldyq_client::Result<()> {
    let mut client = ZoldyQAsync::connect("localhost:6380", None).await?;

    // Subscribe to queue
    client.subscribe("tasks").await?;
    println!("Subscribed to tasks queue");

    // Receive messages
    loop {
        let msg = client.recv_message().await?;
        println!("Received: {:?}", msg.payload);
        client.ack(&msg.id).await?;
    }
}
```

### With Authentication

```rust
// Sync
let mut client = ZoldyQ::connect("localhost:6380", Some("secret"))?;

// Async
let mut client = ZoldyQAsync::connect("localhost:6380", Some("secret")).await?;
```

## API Reference

### ZoldyQ (Synchronous)

- `connect(addr: &str, password: Option<&str>) -> Result<Self>` - Connect to server
- `ping(message: Option<&str>) -> Result<String>` - Health check
- `push(queue: &str, payload: Value) -> Result<String>` - Push a message
- `pop(queue: &str, timeout: u64) -> Result<Option<Message>>` - Pop a message
- `ack(message_id: &str) -> Result<()>` - Acknowledge a message
- `nack(message_id: &str) -> Result<()>` - Reject a message
- `length(queue: &str) -> Result<u64>` - Get queue length
- `delete(queue: &str) -> Result<bool>` - Delete a queue

### ZoldyQAsync (Asynchronous)

Same methods as ZoldyQ, but async. Plus:

- `subscribe(queue: &str) -> Result<()>` - Subscribe to receive messages
- `recv_message() -> Result<Message>` - Receive next message
- `unsubscribe(queue: &str) -> Result<()>` - Unsubscribe from a queue

### Types

```rust
pub struct Message {
    pub id: String,
    pub queue: String,
    pub payload: serde_json::Value,
}
```

## License

MIT
