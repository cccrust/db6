use std::sync::Arc;
use tokio::sync::RwLock;

use super::queue::AsyncQueue;
use crate::kv::{KvEngine, KvStore};

pub struct AsyncMsgq {
    engine: Arc<std::sync::RwLock<KvEngine>>,
    queues: Arc<RwLock<std::collections::HashMap<String, AsyncQueue>>>,
}

impl AsyncMsgq {
    pub fn new(engine: Arc<std::sync::RwLock<KvEngine>>) -> Self {
        Self {
            engine,
            queues: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn queue(&self, name: &str) -> AsyncQueue {
        let mut queues = self.queues.write().await;
        if let Some(q) = queues.get(name) {
            return q.clone();
        }
        let q = AsyncQueue::new(name, self.engine.clone());
        let q_clone = q.clone();
        queues.insert(name.to_string(), q);
        q_clone
    }

    pub async fn delete_queue(&mut self, name: &str) -> Result<(), String> {
        let mut queues = self.queues.write().await;
        if let Some(mut q) = queues.remove(name) {
            q.purge().await?;
        }
        Ok(())
    }

    pub async fn list_queues(&self) -> Vec<String> {
        let start = b"queue:";
        let end = b"queue;";

        let results = {
            let guard = self.engine.read().unwrap();
            guard.scan(1, start, end).unwrap_or_default()
        };
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

        queues.into_iter().collect()
    }
}
