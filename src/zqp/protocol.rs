use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Command {
    Auth,
    Ping,
    Push,
    Pop,
    Subscribe,
    Unsubscribe,
    Ack,
    Nack,
    Len,
    Del,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub cmd: Command,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pong: Option<String>,
}

impl Response {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            id: None,
            payload: None,
            queue: None,
            length: None,
            pong: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            id: None,
            payload: None,
            queue: None,
            length: None,
            pong: None,
        }
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn with_queue(mut self, queue: String) -> Self {
        self.queue = Some(queue);
        self
    }

    pub fn with_length(mut self, length: u64) -> Self {
        self.length = Some(length);
        self
    }

    pub fn with_pong(mut self, pong: String) -> Self {
        self.pong = Some(pong);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPush {
    #[serde(rename = "type")]
    pub push_type: String,
    pub id: String,
    pub queue: String,
    pub payload: serde_json::Value,
}

impl ServerPush {
    pub fn message(id: String, queue: String, payload: serde_json::Value) -> Self {
        Self {
            push_type: "message".to_string(),
            id,
            queue,
            payload,
        }
    }
}

pub fn encode_frame(data: &impl Serialize) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    let payload = rmp_serde::to_vec(data)?;
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend(payload);
    Ok(frame)
}

pub fn decode_request(payload: &[u8]) -> Result<Request, rmp_serde::decode::Error> {
    rmp_serde::from_slice(payload)
}
