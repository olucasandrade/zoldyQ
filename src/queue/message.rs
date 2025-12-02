// TODO: Implement Message struct
//
// Required fields:
// - payload: serde_json::Value
// - enqueued_at: u64 (microseconds since epoch)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub payload: serde_json::Value,
    pub enqueued_at: u64,
}

// TODO: Implement Message methods if needed

