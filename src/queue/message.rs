// Message struct
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

impl Message {
    pub fn new(payload: serde_json::Value) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let enqueued_at = now.as_secs() * 1_000_000 + u64::from(now.subsec_micros());
        
        Self {
            payload,
            enqueued_at,
        }
    }
}
