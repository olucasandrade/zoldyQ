// TODO: Implement MessageQueue struct
//
// Required methods:
// - new(name: String, capacity: usize) -> Self
// - enqueue(payload: serde_json::Value) -> Result<u64, Error>
// - dequeue(timeout: Duration) -> Option<Message>
// - size() -> usize
// - capacity() -> usize
// - name() -> &str
// - stats() -> &QueueStats

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
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
}

// TODO: Implement all MessageQueue methods to make tests pass

