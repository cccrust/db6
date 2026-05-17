//! Async Queue Implementation using tokio channels

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::sync::Notify;
use std::collections::BTreeSet;
use std::sync::Mutex;
use tokio::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncQueueMessage {
    pub id: String,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    pub enqueued_at: u64,
    pub visibility_timeout: u64,
    pub visible_after: u64,
    pub delivery_count: u32,
    pub priority: u8,
}

impl AsyncQueueMessage {
    pub fn new(payload: Vec<u8>, visibility_timeout: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let id = format!("{}:{:08x}", now, fastrand::u32(..));

        Self {
            id,
            payload,
            enqueued_at: now,
            visibility_timeout,
            visible_after: 0,
            delivery_count: 0,
            priority: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        now >= self.visible_after
    }

    pub fn payload_str(&self) -> Option<String> {
        String::from_utf8(self.payload.clone()).ok()
    }
}

#[derive(Debug, Clone)]
pub struct AsyncQueueConfig {
    pub max_delivery_count: u32,
    pub dlq_name: Option<String>,
    pub message_ttl_secs: Option<u64>,
    pub priority_enabled: bool,
}

impl Default for AsyncQueueConfig {
    fn default() -> Self {
        Self {
            max_delivery_count: 3,
            dlq_name: None,
            message_ttl_secs: None,
            priority_enabled: false,
        }
    }
}

pub struct AsyncQueue {
    name: String,
    messages: Arc<RwLock<Vec<AsyncQueueMessage>>>,
    inflight: Arc<RwLock<Vec<String>>>,
    priority_messages: Arc<RwLock<Vec<AsyncQueueMessage>>>,
    config: AsyncQueueConfig,
    dlq: Arc<RwLock<Vec<AsyncQueueMessage>>>,
}

impl AsyncQueue {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            messages: Arc::new(RwLock::new(Vec::new())),
            inflight: Arc::new(RwLock::new(Vec::new())),
            priority_messages: Arc::new(RwLock::new(Vec::new())),
            config: AsyncQueueConfig::default(),
            dlq: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_config(name: &str, config: AsyncQueueConfig) -> Self {
        Self {
            name: name.to_string(),
            messages: Arc::new(RwLock::new(Vec::new())),
            inflight: Arc::new(RwLock::new(Vec::new())),
            priority_messages: Arc::new(RwLock::new(Vec::new())),
            config,
            dlq: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn config(&self) -> &AsyncQueueConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: AsyncQueueConfig) {
        self.config = config;
    }

    /// Enqueue - add message to queue
    pub async fn enqueue(&mut self, payload: Vec<u8>, _visibility_timeout: u64) -> Result<String, String> {
        let msg = AsyncQueueMessage::new(payload, 30);
        let msg_id = msg.id.clone();

        let mut messages = self.messages.write().await;
        messages.push(msg);

        Ok(msg_id)
    }

    pub async fn enqueue_at(&mut self, payload: Vec<u8>, visibility_timeout: u64, deliver_at: u64) -> Result<String, String> {
        let mut msg = AsyncQueueMessage::new(payload, visibility_timeout);
        msg.visible_after = deliver_at;
        let msg_id = msg.id.clone();

        let mut messages = self.messages.write().await;
        messages.push(msg);

        Ok(msg_id)
    }

    pub async fn enqueue_delay(&mut self, payload: Vec<u8>, visibility_timeout: u64, delay_secs: u64) -> Result<String, String> {
        let deliver_at = Self::now_millis() + delay_secs * 1000;
        self.enqueue_at(payload, visibility_timeout, deliver_at).await
    }

    pub async fn enqueue_priority(&mut self, payload: Vec<u8>, priority: u8) -> Result<String, String> {
        if !self.config.priority_enabled {
            return Err("priority queue not enabled".to_string());
        }

        let mut msg = AsyncQueueMessage::new(payload, 0);
        msg.priority = priority;
        let msg_id = msg.id.clone();

        let mut pmsgs = self.priority_messages.write().await;
        pmsgs.push(msg);
        pmsgs.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(msg_id)
    }

    pub async fn batch_enqueue(&mut self, payloads: Vec<Vec<u8>>, visibility_timeout: u64) -> Result<Vec<String>, String> {
        let mut ids = Vec::new();
        for payload in payloads {
            let id = self.enqueue(payload, visibility_timeout).await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Dequeue - get next visible message
    pub async fn dequeue(&mut self, wait_secs: u64) -> Result<Option<AsyncQueueMessage>, String> {
        if self.config.priority_enabled {
            if let Some(msg) = self.dequeue_priority().await? {
                return Ok(Some(msg));
            }
        }

        let now = Self::now_millis();

        loop {
            let mut messages = self.messages.write().await;
            let inflight = self.inflight.read().await;

            let mut to_move_to_dlq: Option<(usize, AsyncQueueMessage)> = None;
            let mut result_msg: Option<AsyncQueueMessage> = None;

            for (i, msg) in messages.iter_mut().enumerate() {
                if msg.is_visible() && !inflight.contains(&msg.id) {
                    if self.config.dlq_name.is_some() && msg.delivery_count >= self.config.max_delivery_count {
                        to_move_to_dlq = Some((i, msg.clone()));
                        continue;
                    }

                    msg.delivery_count += 1;
                    msg.visible_after = now + msg.visibility_timeout * 1000;
                    result_msg = Some(msg.clone());
                    break;
                }
            }

            if let Some((i, dlq_msg)) = to_move_to_dlq {
                let dlq_msg_id = dlq_msg.id.clone();
                {
                    let mut dlq = self.dlq.write().await;
                    dlq.push(dlq_msg);
                }
                messages.remove(i);
                drop(inflight);
                let mut inflight = self.inflight.write().await;
                inflight.push(dlq_msg_id.clone());
                inflight.retain(|id| *id != dlq_msg_id);
                continue;
            }

            if let Some(msg) = result_msg {
                drop(inflight);
                let mut inflight = self.inflight.write().await;
                inflight.push(msg.id.clone());
                return Ok(Some(msg));
            }

            drop(messages);

            if wait_secs == 0 {
                return Ok(None);
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    async fn dequeue_priority(&mut self) -> Result<Option<AsyncQueueMessage>, String> {
        let now = Self::now_millis();
        let mut pmsgs = self.priority_messages.write().await;
        let inflight = self.inflight.read().await;

        for msg in pmsgs.iter_mut() {
            if msg.is_visible() && !inflight.contains(&msg.id) {
                let mut inflight = self.inflight.write().await;
                inflight.push(msg.id.clone());

                msg.visible_after = now + msg.visibility_timeout * 1000;

                return Ok(Some(msg.clone()));
            }
        }

        Ok(None)
    }

    /// Ack - confirm message processed
    pub async fn ack(&mut self, msg_id: &str) -> Result<(), String> {
        let mut messages = self.messages.write().await;
        messages.retain(|m| m.id != msg_id);

        let mut pmsgs = self.priority_messages.write().await;
        pmsgs.retain(|m| m.id != msg_id);

        let mut inflight = self.inflight.write().await;
        inflight.retain(|id| id != msg_id);

        Ok(())
    }

    /// Nack - requeue message
    pub async fn nack(&mut self, msg_id: &str) -> Result<(), String> {
        let now = Self::now_millis();

        let mut messages = self.messages.write().await;
        if let Some(msg) = messages.iter_mut().find(|m| m.id == msg_id) {
            msg.visible_after = now;
        }

        let mut inflight = self.inflight.write().await;
        inflight.retain(|id| id != msg_id);

        Ok(())
    }

    /// Peek - view first message without removing
    pub async fn peek(&self) -> Result<Option<AsyncQueueMessage>, String> {
        let messages = self.messages.read().await;
        for msg in messages.iter() {
            if msg.is_visible() {
                return Ok(Some(msg.clone()));
            }
        }
        Ok(None)
    }

    /// Length - queue size
    pub async fn length(&self) -> Result<usize, String> {
        let messages = self.messages.read().await;
        Ok(messages.len())
    }

    pub async fn priority_length(&self) -> Result<usize, String> {
        let pmsgs = self.priority_messages.read().await;
        Ok(pmsgs.len())
    }

    pub async fn dlq_length(&self) -> Result<usize, String> {
        let dlq = self.dlq.read().await;
        Ok(dlq.len())
    }

    pub async fn cleanup_expired(&mut self) -> Result<usize, String> {
        let ttl = match self.config.message_ttl_secs {
            Some(ttl) => ttl * 1000,
            None => return Ok(0),
        };

        let now = Self::now_millis();
        let mut removed = 0;

        let mut messages = self.messages.write().await;
        let original_len = messages.len();
        messages.retain(|msg| now - msg.enqueued_at <= ttl);
        removed = original_len - messages.len();

        Ok(removed)
    }

    pub async fn purge_dlq(&mut self) -> Result<usize, String> {
        let mut dlq = self.dlq.write().await;
        let len = dlq.len();
        dlq.clear();
        Ok(len)
    }

    /// Purge - clear all messages
    pub async fn purge(&mut self) -> Result<(), String> {
        let mut messages = self.messages.write().await;
        messages.clear();

        let mut pmsgs = self.priority_messages.write().await;
        pmsgs.clear();

        let mut inflight = self.inflight.write().await;
        inflight.clear();

        Ok(())
    }
}

/// Async Queue Manager
pub struct AsyncMsgq {
    queues: Arc<RwLock<std::collections::HashMap<String, AsyncQueue>>>,
}

impl AsyncMsgq {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn queue(&self, name: &str) -> AsyncQueue {
        let mut queues = self.queues.write().await;
        if let Some(q) = queues.get(name) {
            return q.clone();
        }
        let q = AsyncQueue::new(name);
        let q_clone = q.clone();
        queues.insert(name.to_string(), q);
        q_clone
    }

    pub async fn delete_queue(&mut self, name: &str) -> Result<(), String> {
        let mut queues = self.queues.write().await;
        queues.remove(name);
        Ok(())
    }

    pub async fn list_queues(&self) -> Vec<String> {
        let queues = self.queues.read().await;
        queues.keys().cloned().collect()
    }
}

impl Default for AsyncMsgq {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AsyncQueue {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            messages: self.messages.clone(),
            inflight: self.inflight.clone(),
            priority_messages: self.priority_messages.clone(),
            config: self.config.clone(),
            dlq: self.dlq.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_queue_basic() {
        let mut q = AsyncQueue::new("test");

        let id = q.enqueue(b"hello".to_vec(), 30).await.unwrap();
        assert!(!id.is_empty());

        assert_eq!(q.length().await.unwrap(), 1);

        let msg = q.dequeue(0).await.unwrap().unwrap();
        assert_eq!(msg.payload, b"hello");

        q.ack(&msg.id).await.unwrap();
        assert_eq!(q.length().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_async_queue_nack() {
        let mut q = AsyncQueue::new("test");

        q.enqueue(b"msg".to_vec(), 30).await.unwrap();

        let msg = q.dequeue(0).await.unwrap().unwrap();
        assert_eq!(msg.delivery_count, 1);

        q.nack(&msg.id).await.unwrap();

        let msg2 = q.dequeue(0).await.unwrap().unwrap();
        assert_eq!(msg2.delivery_count, 2);
    }

    #[tokio::test]
    async fn test_async_queue_purge() {
        let mut q = AsyncQueue::new("test");

        q.enqueue(b"msg1".to_vec(), 30).await.unwrap();
        q.enqueue(b"msg2".to_vec(), 30).await.unwrap();

        q.purge().await.unwrap();

        assert_eq!(q.length().await.unwrap(), 0);
    }
}