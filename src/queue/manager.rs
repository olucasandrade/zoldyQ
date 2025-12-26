use std::collections::HashMap;
use std::sync::Arc;
use std::io::Error;
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

impl QueueManager {
    pub fn new(default_capacity: usize, max_queues: usize) -> Self {
        Self {
            queues: Arc::new(DashMap::new()),
            default_capacity,
            max_queues
        }
    }

    pub fn get_or_create_queue(&self, queue_name: &str) -> Result<Arc<MessageQueue>, Error> {
        // First, try to get existing queue without holding any locks
        if let Some(queue) = self.queues.get(queue_name) {
            return Ok(Arc::clone(&queue));
        }

        // Check max queues limit before creating
        if self.queues.len() >= self.max_queues {
            return Err(Error::new(std::io::ErrorKind::Other, "Maximum number of queues reached"));
        }

        // Create the queue outside of any lock
        let new_queue = Arc::new(MessageQueue::new(queue_name.to_string(), self.default_capacity));

        // Try to insert, but if another thread beat us to it, use theirs
        match self.queues.insert(queue_name.to_string(), new_queue.clone()) {
            None => Ok(new_queue), // We successfully inserted
            Some(existing) => Ok(existing), // Someone else inserted first, use theirs
        }
    }


    pub fn get_queue(&self, queue_name: &str) -> Option<Arc<MessageQueue>> {
        self.queues.get(queue_name).map(|arc_queue| Arc::clone(&arc_queue))
    }

    pub fn enqueue(&self, queue_name: &str, payload: serde_json::Value) -> Result<Message, Error> {
        let queue = self.get_or_create_queue(queue_name)?;
        queue.enqueue(payload)
    }

    pub async fn dequeue(&self, queue_name: &str, timeout: Duration) -> Result<Option<Message>, Error> {
        let queue_arc = self.get_queue(queue_name)
            .ok_or_else(|| Error::new(std::io::ErrorKind::NotFound, "Queue not found"))?;
        
        // Clone the Arc to ensure we own it before awaiting
        // This prevents any potential borrow checker issues in async context
        let queue = queue_arc.clone();
        // Arc implements Deref, so we can call methods directly
        Ok(queue.dequeue(timeout).await)
    }

    pub fn list_queues(&self) -> Arc<DashMap<String, Arc<MessageQueue>>> {
        self.queues.clone()
    }

    pub fn queue_count(&self) -> usize {
        self.queues.len()
    }

    pub fn default_capacity(&self) -> usize {
        self.default_capacity
    }

    pub fn max_queues(&self) -> usize {
        self.max_queues
    }

    pub fn delete_queue(&self, queue_name: &str) -> bool {
        self.queues.remove(queue_name).is_some()
    }
}