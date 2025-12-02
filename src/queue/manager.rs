// TODO: Implement QueueManager struct
//
// Required methods:
// - new(default_capacity: usize, max_queues: usize) -> Self
// - get_or_create_queue(name: &str) -> Result<Arc<MessageQueue>, Error>
// - get_queue(name: &str) -> Option<Arc<MessageQueue>>
// - enqueue(queue_name: &str, payload: Value) -> Result<u64, Error>
// - dequeue(queue_name: &str, timeout: Duration) -> Result<Option<Message>, Error>
// - list_queues() -> Vec<String>
// - queue_count() -> usize
// - get_all_stats() -> HashMap<String, QueueStats>
// - get_stats_summary() -> StatsSummary
// - delete_queue(name: &str) -> Result<(), Error>
// - default_capacity() -> usize
// - max_queues() -> usize

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;

use super::message::Message;
use super::message_queue::MessageQueue;

pub struct QueueManager {
    queues: Arc<DashMap<String, Arc<MessageQueue>>>,
    default_capacity: usize,
    max_queues: usize,
}

pub struct StatsSummary {
    pub total_queues: usize,
    pub queues: HashMap<String, QueueStatsInfo>,
}

pub struct QueueStatsInfo {
    pub size: usize,
    pub capacity: usize,
    pub enqueued_total: u64,
    pub dequeued_total: u64,
}

// TODO: Implement all QueueManager methods to make tests pass

