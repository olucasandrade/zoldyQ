// ZoldyQ - Ultra-fast in-memory FIFO message queue
//
// This library provides the core message queue functionality.
// Binary entry point is in src/main.rs

pub mod queue;

// Re-export main types for convenience
pub use queue::{Message, MessageQueue, QueueManager, QueueStats};

