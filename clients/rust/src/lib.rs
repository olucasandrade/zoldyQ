use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream as TokioTcpStream;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub queue: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Request {
    cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Response {
    ok: bool,
    error: Option<String>,
    id: Option<String>,
    payload: Option<serde_json::Value>,
    queue: Option<String>,
    length: Option<u64>,
    pong: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
}

#[derive(Debug)]
pub struct ZoldyQError {
    pub message: String,
}

impl std::fmt::Display for ZoldyQError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ZoldyQError {}

impl From<io::Error> for ZoldyQError {
    fn from(err: io::Error) -> Self {
        ZoldyQError {
            message: err.to_string(),
        }
    }
}

impl From<rmp_serde::encode::Error> for ZoldyQError {
    fn from(err: rmp_serde::encode::Error) -> Self {
        ZoldyQError {
            message: err.to_string(),
        }
    }
}

impl From<rmp_serde::decode::Error> for ZoldyQError {
    fn from(err: rmp_serde::decode::Error) -> Self {
        ZoldyQError {
            message: err.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, ZoldyQError>;

pub struct ZoldyQ {
    stream: TcpStream,
}

impl ZoldyQ {
    pub fn connect(addr: &str, password: Option<&str>) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        let mut client = Self { stream };

        if let Some(pw) = password {
            client.call(Request {
                cmd: "auth".to_string(),
                queue: None,
                payload: None,
                timeout: None,
                id: None,
                password: Some(pw.to_string()),
            })?;
        }

        Ok(client)
    }

    fn send(&mut self, req: &Request) -> Result<()> {
        let data = rmp_serde::to_vec(req)?;
        let len = data.len() as u32;
        self.stream.write_all(&len.to_le_bytes())?;
        self.stream.write_all(&data)?;
        Ok(())
    }

    fn recv(&mut self) -> Result<Response> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header)?;
        let len = u32::from_le_bytes(header) as usize;

        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data)?;

        let resp: Response = rmp_serde::from_slice(&data)?;
        Ok(resp)
    }

    fn call(&mut self, req: Request) -> Result<Response> {
        self.send(&req)?;
        let resp = self.recv()?;
        if !resp.ok {
            return Err(ZoldyQError {
                message: resp.error.unwrap_or_else(|| "Unknown error".to_string()),
            });
        }
        Ok(resp)
    }

    pub fn ping(&mut self, message: Option<&str>) -> Result<String> {
        let resp = self.call(Request {
            cmd: "ping".to_string(),
            queue: None,
            payload: message.map(|m| serde_json::Value::String(m.to_string())),
            timeout: None,
            id: None,
            password: None,
        })?;
        Ok(resp.pong.unwrap_or_else(|| "PONG".to_string()))
    }

    pub fn push(&mut self, queue: &str, payload: serde_json::Value) -> Result<String> {
        let resp = self.call(Request {
            cmd: "push".to_string(),
            queue: Some(queue.to_string()),
            payload: Some(payload),
            timeout: None,
            id: None,
            password: None,
        })?;
        Ok(resp.id.ok_or_else(|| ZoldyQError {
            message: "No message ID returned".to_string(),
        })?)
    }

    pub fn pop(&mut self, queue: &str, timeout: u64) -> Result<Option<Message>> {
        let resp = self.call(Request {
            cmd: "pop".to_string(),
            queue: Some(queue.to_string()),
            payload: None,
            timeout: Some(timeout),
            id: None,
            password: None,
        })?;

        if let Some(id) = resp.id {
            Ok(Some(Message {
                id,
                queue: resp.queue.unwrap_or_else(|| queue.to_string()),
                payload: resp.payload.unwrap_or(serde_json::Value::Null),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn ack(&mut self, message_id: &str) -> Result<()> {
        self.call(Request {
            cmd: "ack".to_string(),
            queue: None,
            payload: None,
            timeout: None,
            id: Some(message_id.to_string()),
            password: None,
        })?;
        Ok(())
    }

    pub fn nack(&mut self, message_id: &str) -> Result<()> {
        self.call(Request {
            cmd: "nack".to_string(),
            queue: None,
            payload: None,
            timeout: None,
            id: Some(message_id.to_string()),
            password: None,
        })?;
        Ok(())
    }

    pub fn length(&mut self, queue: &str) -> Result<u64> {
        let resp = self.call(Request {
            cmd: "len".to_string(),
            queue: Some(queue.to_string()),
            payload: None,
            timeout: None,
            id: None,
            password: None,
        })?;
        Ok(resp.length.unwrap_or(0))
    }

    pub fn delete(&mut self, queue: &str) -> Result<bool> {
        let resp = self.call(Request {
            cmd: "del".to_string(),
            queue: Some(queue.to_string()),
            payload: None,
            timeout: None,
            id: None,
            password: None,
        })?;
        Ok(resp.length.unwrap_or(0) > 0)
    }
}

pub struct ZoldyQAsync {
    stream: TokioTcpStream,
}

impl ZoldyQAsync {
    pub async fn connect(addr: &str, password: Option<&str>) -> Result<Self> {
        let stream = TokioTcpStream::connect(addr).await?;
        let mut client = Self { stream };

        if let Some(pw) = password {
            client
                .call(Request {
                    cmd: "auth".to_string(),
                    queue: None,
                    payload: None,
                    timeout: None,
                    id: None,
                    password: Some(pw.to_string()),
                })
                .await?;
        }

        Ok(client)
    }

    async fn send(&mut self, req: &Request) -> Result<()> {
        let data = rmp_serde::to_vec(req)?;
        let len = data.len() as u32;
        self.stream.write_all(&len.to_le_bytes()).await?;
        self.stream.write_all(&data).await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Response> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header).await?;
        let len = u32::from_le_bytes(header) as usize;

        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data).await?;

        let resp: Response = rmp_serde::from_slice(&data)?;
        Ok(resp)
    }

    async fn call(&mut self, req: Request) -> Result<Response> {
        self.send(&req).await?;
        let resp = self.recv().await?;
        if !resp.ok {
            return Err(ZoldyQError {
                message: resp.error.unwrap_or_else(|| "Unknown error".to_string()),
            });
        }
        Ok(resp)
    }

    pub async fn ping(&mut self, message: Option<&str>) -> Result<String> {
        let resp = self
            .call(Request {
                cmd: "ping".to_string(),
                queue: None,
                payload: message.map(|m| serde_json::Value::String(m.to_string())),
                timeout: None,
                id: None,
                password: None,
            })
            .await?;
        Ok(resp.pong.unwrap_or_else(|| "PONG".to_string()))
    }

    pub async fn push(&mut self, queue: &str, payload: serde_json::Value) -> Result<String> {
        let resp = self
            .call(Request {
                cmd: "push".to_string(),
                queue: Some(queue.to_string()),
                payload: Some(payload),
                timeout: None,
                id: None,
                password: None,
            })
            .await?;
        Ok(resp.id.ok_or_else(|| ZoldyQError {
            message: "No message ID returned".to_string(),
        })?)
    }

    pub async fn pop(&mut self, queue: &str, timeout: u64) -> Result<Option<Message>> {
        let resp = self
            .call(Request {
                cmd: "pop".to_string(),
                queue: Some(queue.to_string()),
                payload: None,
                timeout: Some(timeout),
                id: None,
                password: None,
            })
            .await?;

        if let Some(id) = resp.id {
            Ok(Some(Message {
                id,
                queue: resp.queue.unwrap_or_else(|| queue.to_string()),
                payload: resp.payload.unwrap_or(serde_json::Value::Null),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn ack(&mut self, message_id: &str) -> Result<()> {
        self.call(Request {
            cmd: "ack".to_string(),
            queue: None,
            payload: None,
            timeout: None,
            id: Some(message_id.to_string()),
            password: None,
        })
        .await?;
        Ok(())
    }

    pub async fn nack(&mut self, message_id: &str) -> Result<()> {
        self.call(Request {
            cmd: "nack".to_string(),
            queue: None,
            payload: None,
            timeout: None,
            id: Some(message_id.to_string()),
            password: None,
        })
        .await?;
        Ok(())
    }

    pub async fn length(&mut self, queue: &str) -> Result<u64> {
        let resp = self
            .call(Request {
                cmd: "len".to_string(),
                queue: Some(queue.to_string()),
                payload: None,
                timeout: None,
                id: None,
                password: None,
            })
            .await?;
        Ok(resp.length.unwrap_or(0))
    }

    pub async fn delete(&mut self, queue: &str) -> Result<bool> {
        let resp = self
            .call(Request {
                cmd: "del".to_string(),
                queue: Some(queue.to_string()),
                payload: None,
                timeout: None,
                id: None,
                password: None,
            })
            .await?;
        Ok(resp.length.unwrap_or(0) > 0)
    }

    pub async fn subscribe(&mut self, queue: &str) -> Result<()> {
        self.call(Request {
            cmd: "subscribe".to_string(),
            queue: Some(queue.to_string()),
            payload: None,
            timeout: None,
            id: None,
            password: None,
        })
        .await?;
        Ok(())
    }

    pub async fn recv_message(&mut self) -> Result<Message> {
        let resp = self.recv().await?;
        if resp.msg_type.as_deref() == Some("message") {
            Ok(Message {
                id: resp.id.unwrap_or_default(),
                queue: resp.queue.unwrap_or_default(),
                payload: resp.payload.unwrap_or(serde_json::Value::Null),
            })
        } else {
            Err(ZoldyQError {
                message: "Unexpected response type".to_string(),
            })
        }
    }

    pub async fn unsubscribe(&mut self, queue: &str) -> Result<()> {
        self.call(Request {
            cmd: "unsubscribe".to_string(),
            queue: Some(queue.to_string()),
            payload: None,
            timeout: None,
            id: None,
            password: None,
        })
        .await?;
        Ok(())
    }
}
