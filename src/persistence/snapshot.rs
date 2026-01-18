use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::queue::{Message, QueueManager};

const MAGIC: &[u8; 4] = b"ZQSN";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    timestamp: u64,
    queues: Vec<QueueSnapshot>,
}

#[derive(Serialize, Deserialize)]
struct QueueSnapshot {
    name: String,
    messages: Vec<SerializableMessage>,
}

#[derive(Serialize, Deserialize)]
struct SerializableMessage {
    id: String,
    payload: String,
    enqueued_at: u64,
    delivery_count: u32,
    expires_at: Option<u64>,
}

impl From<&Message> for SerializableMessage {
    fn from(msg: &Message) -> Self {
        Self {
            id: msg.id.clone(),
            payload: msg.payload.to_string(),
            enqueued_at: msg.enqueued_at,
            delivery_count: msg.delivery_count,
            expires_at: msg.expires_at,
        }
    }
}

impl SerializableMessage {
    fn to_message(&self) -> Message {
        let payload = serde_json::from_str(&self.payload)
            .unwrap_or_else(|_| serde_json::Value::String(self.payload.clone()));
        Message {
            id: self.id.clone(),
            payload,
            enqueued_at: self.enqueued_at,
            delivery_count: self.delivery_count,
            expires_at: self.expires_at,
        }
    }
}

#[derive(Clone)]
pub struct SnapshotConfig {
    pub path: String,
    pub interval_secs: u64,
    pub on_shutdown: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            path: "./data/snapshot.rdb".to_string(),
            interval_secs: 300,
            on_shutdown: true,
        }
    }
}

pub struct SnapshotManager {
    config: SnapshotConfig,
    queue_manager: Arc<QueueManager>,
    last_save: AtomicU64,
}

impl SnapshotManager {
    pub fn new(config: SnapshotConfig, queue_manager: Arc<QueueManager>) -> Self {
        Self {
            config,
            queue_manager,
            last_save: AtomicU64::new(0),
        }
    }

    pub fn last_save_timestamp(&self) -> u64 {
        self.last_save.load(Ordering::SeqCst)
    }

    pub fn save(&self) -> std::io::Result<()> {
        let temp_path = format!("{}.tmp", self.config.path);
        
        if let Some(parent) = Path::new(&self.config.path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let mut queues = Vec::new();
        let queue_map = self.queue_manager.list_queues();
        
        for entry in queue_map.iter() {
            let name = entry.key().clone();
            let queue = entry.value();
            let messages: Vec<SerializableMessage> = queue
                .snapshot_messages()
                .iter()
                .map(SerializableMessage::from)
                .collect();
            queues.push(QueueSnapshot { name, messages });
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let snapshot = Snapshot {
            version: VERSION,
            timestamp,
            queues,
        };

        let file = File::create(&temp_path)?;
        let mut writer = BufWriter::new(file);

        writer.write_all(MAGIC)?;
        writer.write_all(&VERSION.to_le_bytes())?;

        let data = bincode::serialize(&snapshot)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        let checksum = crc32fast::hash(&data);
        writer.write_all(&checksum.to_le_bytes())?;
        writer.write_all(&data)?;

        writer.flush()?;
        drop(writer);

        fs::rename(&temp_path, &self.config.path)?;

        self.last_save.store(timestamp, Ordering::SeqCst);
        
        let total_messages: usize = snapshot.queues.iter().map(|q| q.messages.len()).sum();
        tracing::info!(
            "Snapshot saved: {} queues, {} messages to {}",
            snapshot.queues.len(),
            total_messages,
            self.config.path
        );

        Ok(())
    }

    pub fn load(&self) -> std::io::Result<()> {
        let path = Path::new(&self.config.path);
        if !path.exists() {
            tracing::info!("No snapshot file found at {}", self.config.path);
            return Ok(());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid snapshot file: wrong magic bytes"
            ));
        }

        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_le_bytes(version_bytes);
        
        if version > VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Snapshot version {} is newer than supported version {}", version, VERSION)
            ));
        }

        let mut checksum_bytes = [0u8; 4];
        reader.read_exact(&mut checksum_bytes)?;
        let expected_checksum = u32::from_le_bytes(checksum_bytes);

        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        let actual_checksum = crc32fast::hash(&data);
        if actual_checksum != expected_checksum {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Snapshot checksum mismatch - file may be corrupted"
            ));
        }

        let snapshot: Snapshot = bincode::deserialize(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut total_messages = 0;
        for queue_snap in snapshot.queues {
            for ser_msg in queue_snap.messages {
                let message = ser_msg.to_message();
                if !message.is_expired() {
                    self.queue_manager.enqueue_message(&queue_snap.name, message)?;
                    total_messages += 1;
                }
            }
        }

        self.last_save.store(snapshot.timestamp, Ordering::SeqCst);
        
        tracing::info!(
            "Snapshot loaded: {} messages from {} (saved at {})",
            total_messages,
            self.config.path,
            snapshot.timestamp
        );

        Ok(())
    }

    pub async fn start_scheduler(
        self: Arc<Self>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        if self.config.interval_secs == 0 {
            tracing::info!("Snapshot scheduling disabled (interval=0)");
            return;
        }

        let interval = Duration::from_secs(self.config.interval_secs);
        tracing::info!("Snapshot scheduler started (interval={}s)", self.config.interval_secs);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = self.save() {
                        tracing::error!("Scheduled snapshot failed: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    if self.config.on_shutdown {
                        tracing::info!("Saving snapshot on shutdown...");
                        if let Err(e) = self.save() {
                            tracing::error!("Shutdown snapshot failed: {}", e);
                        }
                    }
                    break;
                }
            }
        }

        tracing::info!("Snapshot scheduler stopped");
    }
}
