use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub payload: serde_json::Value,
    pub enqueued_at: u64,
    pub delivery_count: u32,
    pub expires_at: Option<u64>,
}

impl Message {
    pub fn new(payload: serde_json::Value) -> Self {
        Self::with_ttl(payload, None)
    }

    pub fn with_ttl(payload: serde_json::Value, ttl_seconds: Option<u64>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let enqueued_at = now.as_secs() * 1_000_000 + u64::from(now.subsec_micros());
        
        let expires_at = ttl_seconds.map(|ttl| {
            enqueued_at + (ttl * 1_000_000)
        });

        Self {
            id: Uuid::new_v4().to_string(),
            payload,
            enqueued_at,
            delivery_count: 0,
            expires_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let now_micros = now.as_secs() * 1_000_000 + u64::from(now.subsec_micros());
            now_micros > expires_at
        } else {
            false
        }
    }

    pub fn increment_delivery(&mut self) {
        self.delivery_count += 1;
    }
}
