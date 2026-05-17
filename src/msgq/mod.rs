//! Message Queue - 基於 KV 的訊息佇列系統

mod error;
mod message;
mod queue;

pub use error::{MsgqError, Result};
pub use message::Message;
pub use queue::Queue;

use crate::kv::{KvEngine, KvStore};
use std::path::Path;
use std::sync::{Arc, RwLock};

pub struct Msgq {
    engine: Arc<RwLock<KvEngine>>,
}

impl Msgq {
    pub fn new(engine_type: &str) -> Result<Self> {
        let engine = KvEngine::new(engine_type)
            .map_err(|e| MsgqError::InvalidEngine(e.to_string()))?;
        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
        })
    }

    pub fn open(engine_type: &str, path: &Path) -> Result<Self> {
        let engine = KvEngine::open(engine_type, path)
            .map_err(|e| MsgqError::InvalidEngine(e.to_string()))?;
        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
        })
    }

    pub fn queue(&self, name: &str) -> Queue {
        Queue::new(name, self.engine.clone())
    }

    pub fn list_queues(&self) -> Result<Vec<String>> {
        let start = b"queue:";
        let end = b"queue;";

        let results = self.engine.read().unwrap().scan(1, start, end)?;
        let mut queues = std::collections::HashSet::new();

        for (key, _) in results {
            let key_str = String::from_utf8_lossy(&key);
            if let Some(name) = key_str.strip_prefix("queue:") {
                if let Some(queue_name) = name.split(':').next() {
                    if !queue_name.is_empty() {
                        queues.insert(queue_name.to_string());
                    }
                }
            }
        }

        Ok(queues.into_iter().collect())
    }

    pub fn delete_queue(&mut self, name: &str) -> Result<()> {
        let mut queue = self.queue(name);
        queue.purge()?;
        Ok(())
    }

    pub fn stats(&self, name: &str) -> Result<QueueStats> {
        let queue = self.queue(name);
        let meta_key = format!("queue:{}:meta", name);

        if let Some(data) = self.engine.read().unwrap().get(1, meta_key.as_bytes())? {
            #[derive(serde::Deserialize)]
            struct QueueMeta {
                total_enqueued: u64,
                completed: u64,
                nacked: u64,
            }
            let meta: QueueMeta = serde_json::from_slice(&data)?;

            Ok(QueueStats {
                name: name.to_string(),
                length: queue.length()?,
                total_enqueued: meta.total_enqueued,
                completed: meta.completed,
                nacked: meta.nacked,
            })
        } else {
            Ok(QueueStats {
                name: name.to_string(),
                length: 0,
                total_enqueued: 0,
                completed: 0,
                nacked: 0,
            })
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct QueueStats {
    pub name: String,
    pub length: usize,
    pub total_enqueued: u64,
    pub completed: u64,
    pub nacked: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msgq_basic() {
        let msgq = Msgq::new("memory").unwrap();
        let mut queue = msgq.queue("test");

        let id1 = queue.enqueue(b"msg1".to_vec(), 30).unwrap();
        let id2 = queue.enqueue(b"msg2".to_vec(), 30).unwrap();

        assert!(!id1.is_empty());
        assert!(!id2.is_empty());
        assert_eq!(queue.length().unwrap(), 2);

        let msg = queue.dequeue(0).unwrap().unwrap();
        assert!(msg.payload == b"msg1" || msg.payload == b"msg2");

        queue.ack(&msg.id).unwrap();
        assert_eq!(queue.length().unwrap(), 1);
    }

    #[test]
    fn test_msgq_visibility_timeout() {
        let msgq = Msgq::new("memory").unwrap();
        let mut queue = msgq.queue("test");

        queue.enqueue(b"msg".to_vec(), 1).unwrap();

        let msg = queue.dequeue(0).unwrap().unwrap();

        let msg2 = queue.dequeue(0).unwrap();
        assert!(msg2.is_none() || msg2.unwrap().id != msg.id);

        std::thread::sleep(std::time::Duration::from_secs(2));

        let msg3 = queue.dequeue(0).unwrap().unwrap();
        assert_eq!(msg3.id, msg.id);
    }

    #[test]
    fn test_msgq_nack() {
        let msgq = Msgq::new("memory").unwrap();
        let mut queue = msgq.queue("test");

        let _id = queue.enqueue(b"msg".to_vec(), 30).unwrap();

        let msg = queue.dequeue(0).unwrap().unwrap();
        assert_eq!(msg.delivery_count, 1);

        queue.nack(&msg.id).unwrap();

        let msg2 = queue.dequeue(0).unwrap().unwrap();
        assert_eq!(msg2.delivery_count, 2);
    }

    #[test]
    fn test_msgq_stats() {
        let msgq = Msgq::new("memory").unwrap();
        let mut queue = msgq.queue("test");

        queue.enqueue(b"msg1".to_vec(), 30).unwrap();
        queue.enqueue(b"msg2".to_vec(), 30).unwrap();

        let stats = msgq.stats("test").unwrap();
        assert_eq!(stats.total_enqueued, 2);
        assert_eq!(stats.length, 2);
    }

    #[test]
    fn test_msgq_purge() {
        let msgq = Msgq::new("memory").unwrap();
        let mut queue = msgq.queue("test");

        queue.enqueue(b"msg1".to_vec(), 30).unwrap();
        queue.enqueue(b"msg2".to_vec(), 30).unwrap();

        assert_eq!(queue.length().unwrap(), 2);

        queue.purge().unwrap();

        assert_eq!(queue.length().unwrap(), 0);
    }

    #[test]
    fn test_msgq_peek() {
        let msgq = Msgq::new("memory").unwrap();
        let mut queue = msgq.queue("test");

        queue.enqueue(b"msg1".to_vec(), 30).unwrap();
        queue.enqueue(b"msg2".to_vec(), 30).unwrap();

        let peeked = queue.peek().unwrap().unwrap();
        assert_eq!(peeked.payload, b"msg1");

        assert_eq!(queue.length().unwrap(), 2);
    }
}