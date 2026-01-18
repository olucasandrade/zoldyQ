use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::queue::{Message, QueueManager};

pub struct InFlightMessage {
    pub message: Message,
    pub queue_name: String,
    pub visible_at: Instant,
}

pub struct AckManager {
    in_flight: Arc<DashMap<String, InFlightMessage>>,
    visibility_timeout: Duration,
    max_retries: u32,
    queue_manager: Arc<QueueManager>,
}

impl AckManager {
    pub fn new(
        queue_manager: Arc<QueueManager>,
        visibility_timeout_secs: u64,
        max_retries: u32,
    ) -> Self {
        Self {
            in_flight: Arc::new(DashMap::new()),
            visibility_timeout: Duration::from_secs(visibility_timeout_secs),
            max_retries,
            queue_manager,
        }
    }

    pub fn track_message(&self, queue_name: &str, mut message: Message) -> Message {
        message.increment_delivery();
        let msg_id = message.id.clone();
        
        self.in_flight.insert(msg_id, InFlightMessage {
            message: message.clone(),
            queue_name: queue_name.to_string(),
            visible_at: Instant::now() + self.visibility_timeout,
        });
        
        message
    }

    pub fn ack(&self, message_id: &str) -> Result<(), String> {
        match self.in_flight.remove(message_id) {
            Some(_) => Ok(()),
            None => Err(format!("Message {} not found in flight", message_id)),
        }
    }

    pub fn nack(&self, message_id: &str) -> Result<(), String> {
        match self.in_flight.remove(message_id) {
            Some((_, in_flight)) => {
                self.return_or_dlq(in_flight);
                Ok(())
            }
            None => Err(format!("Message {} not found in flight", message_id)),
        }
    }

    fn return_or_dlq(&self, in_flight: InFlightMessage) {
        let message = in_flight.message;
        let queue_name = in_flight.queue_name;

        if message.delivery_count >= self.max_retries {
            let dlq_name = format!("{}:dlq", queue_name);
            if let Err(e) = self.queue_manager.enqueue_message(&dlq_name, message.clone()) {
                tracing::error!("Failed to enqueue to DLQ {}: {}", dlq_name, e);
            } else {
                tracing::debug!(
                    "Message {} moved to DLQ after {} deliveries",
                    message.id,
                    message.delivery_count
                );
            }
        } else {
            if let Err(e) = self.queue_manager.enqueue_message(&queue_name, message.clone()) {
                tracing::error!("Failed to return message {} to queue {}: {}", message.id, queue_name, e);
            } else {
                tracing::debug!(
                    "Message {} returned to queue {} (delivery {})",
                    message.id,
                    queue_name,
                    message.delivery_count
                );
            }
        }
    }

    pub fn process_timeouts(&self) {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for entry in self.in_flight.iter() {
            if now >= entry.visible_at {
                timed_out.push(entry.key().clone());
            }
        }

        for msg_id in timed_out {
            if let Some((_, in_flight)) = self.in_flight.remove(&msg_id) {
                tracing::debug!("Message {} timed out, requeuing", msg_id);
                self.return_or_dlq(in_flight);
            }
        }
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn get_in_flight(&self, message_id: &str) -> Option<InFlightMessage> {
        self.in_flight.get(message_id).map(|entry| InFlightMessage {
            message: entry.message.clone(),
            queue_name: entry.queue_name.clone(),
            visible_at: entry.visible_at,
        })
    }

    pub async fn start_reaper(self: Arc<Self>, mut shutdown_rx: broadcast::Receiver<()>) {
        let interval = Duration::from_secs(1);
        
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    self.process_timeouts();
                }
                _ = shutdown_rx.recv() => {
                    tracing::info!("ACK reaper shutting down");
                    break;
                }
            }
        }
    }
}
