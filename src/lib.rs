// ZoldyQ - Ultra-fast in-memory FIFO message queue
//
// This library provides the core message queue functionality.
// Binary entry point is in src/main.rs

pub mod queue;
pub mod resp;

pub use queue::{Message, MessageQueue, QueueManager, QueueStats};
pub use resp::{RespServer, RespConfig};

