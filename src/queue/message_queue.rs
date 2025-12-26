use std::sync::atomic::{AtomicU64, Ordering};
use std::io::Error;
use std::time::Duration;
use std::sync::Arc;
use crossbeam::queue::ArrayQueue;
use tokio::sync::Notify;

use super::message::Message;

pub struct MessageQueue {
    name: String,
    queue: Arc<ArrayQueue<Message>>,
    capacity: usize,
    stats: Arc<QueueStats>,
    notify: Arc<Notify>,
}

pub struct QueueStats {
    enqueued_total: AtomicU64,
    dequeued_total: AtomicU64,
    queue_full_count: AtomicU64,
}

impl QueueStats {
    pub fn new() -> Self {
        Self {
            enqueued_total: AtomicU64::new(0),
            dequeued_total: AtomicU64::new(0),
            queue_full_count: AtomicU64::new(0),
        }
    }

    pub fn enqueued_total(&self) -> u64 {
        self.enqueued_total.load(Ordering::SeqCst)
    }

    pub fn dequeued_total(&self) -> u64 {
        self.dequeued_total.load(Ordering::SeqCst)
    }

    pub fn queue_full_count(&self) -> u64 {
        self.queue_full_count.load(Ordering::SeqCst)
    }

    pub fn increment_queue_full(&self) {
        self.queue_full_count.fetch_add(1, Ordering::SeqCst);
    }
}

impl MessageQueue {
    pub fn new(name: String, capacity: usize) -> Self {
        Self {
            name,
            queue: Arc::new(ArrayQueue::new(capacity)),
            capacity,
            stats: Arc::new(QueueStats::new()),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn enqueue(&self, payload: serde_json::Value) -> Result<Message, Error> {
        let message = Message::new(payload);
        match self.queue.push(message.clone()) {
            Ok(()) => {
                self.stats.enqueued_total.fetch_add(1, Ordering::SeqCst);
                // Use notify_one() instead of notify_waiters() to avoid blocking
                // notify_one() wakes a single waiting task and is non-blocking
                self.notify.notify_one();
                Ok(message)
            }
            Err(_) => {
                self.stats.increment_queue_full();
                Err(Error::new(std::io::ErrorKind::Other, "Queue is full"))
            }
        }
    }

    pub async fn dequeue(&self, timeout: Duration) -> Option<Message> {
        // Try immediate dequeue first
        if let Some(message) = self.queue.pop() {
            self.stats.dequeued_total.fetch_add(1, Ordering::SeqCst);
            return Some(message);
        }

        // If timeout is zero and queue is empty, return None immediately
        if timeout.is_zero() {
            return None;
        }

        // Clone Arc references for use in async block
        let queue = self.queue.clone();
        let stats = self.stats.clone();
        let notify = self.notify.clone();

        // Use tokio::time::timeout to wrap the async wait
        // This ensures proper async behavior and prevents blocking
        let result = tokio::time::timeout(timeout, async move {
            // Wait in a loop until we successfully pop a message
            // This handles spurious wakeups correctly
            loop {
                // Wait for notification that a message might be available
                notify.notified().await;
                
                // Try to pop a message after being notified
                if let Some(message) = queue.pop() {
                    stats.dequeued_total.fetch_add(1, Ordering::SeqCst);
                    return Some(message);
                }
                // Spurious wakeup - another consumer took the message
                // Continue waiting for the next notification
            }
        }).await;

        match result {
            Ok(Some(message)) => Some(message),
            Ok(None) => {
                // This shouldn't happen as the loop always returns Some(message) or times out
                // But handle it gracefully by doing a final check
                self.queue.pop().map(|message| {
                    self.stats.dequeued_total.fetch_add(1, Ordering::SeqCst);
                    message
                })
            }
            Err(_) => {
                // Timeout expired, do final check before returning None
                self.queue.pop().map(|message| {
                    self.stats.dequeued_total.fetch_add(1, Ordering::SeqCst);
                    message
                })
            }
        }
    }

    pub fn size(&self) -> usize {
        self.queue.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn name(&self) -> &str {
        &self.name.as_str()
    }

    pub fn stats(&self) -> &QueueStats {
        &self.stats
    }
}
