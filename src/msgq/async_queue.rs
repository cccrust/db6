//! Async Queue Implementation using tokio channels

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncQueueMessage {
    pub id: String,
    pub payload: Vec<u8>,
    pub enqueued_at: u64,
    pub visibility_timeout: u64,
    pub visible_after: u64,
    pub delivery_count: u32,
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

pub struct AsyncQueue {
    name: String,
    messages: Arc<RwLock<Vec<AsyncQueueMessage>>>,
    inflight: Arc<RwLock<Vec<String>>>,
}

impl AsyncQueue {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            messages: Arc::new(RwLock::new(Vec::new())),
            inflight: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Enqueue - add message to queue
    pub async fn enqueue(&mut self, payload: Vec<u8>, _visibility_timeout: u64) -> Result<String, String> {
        let msg = AsyncQueueMessage::new(payload, 30);
        let msg_id = msg.id.clone();

        let mut messages = self.messages.write().await;
        messages.push(msg);

        Ok(msg_id)
    }

    /// Dequeue - get next visible message
    pub async fn dequeue(&mut self, wait_secs: u64) -> Result<Option<AsyncQueueMessage>, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        loop {
            let mut messages = self.messages.write().await;
            let inflight = self.inflight.read().await;

            for (i, msg) in messages.iter_mut().enumerate() {
                if msg.is_visible() && !inflight.contains(&msg.id) {
                    // Mark as inflight
                    drop(inflight);
                    let mut inflight = self.inflight.write().await;
                    inflight.push(msg.id.clone());
                    drop(inflight);

                    // Update delivery count
                    msg.delivery_count += 1;
                    msg.visible_after = now + msg.visibility_timeout * 1000;

                    return Ok(Some(msg.clone()));
                }
            }
            drop(messages);

            if wait_secs == 0 {
                return Ok(None);
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    /// Ack - confirm message processed
    pub async fn ack(&mut self, msg_id: &str) -> Result<(), String> {
        let mut messages = self.messages.write().await;
        if let Some(pos) = messages.iter().position(|m| m.id == msg_id) {
            messages.remove(pos);
        }

        let mut inflight = self.inflight.write().await;
        inflight.retain(|id| id != msg_id);

        Ok(())
    }

    /// Nack - requeue message
    pub async fn nack(&mut self, msg_id: &str) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

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

    /// Purge - clear all messages
    pub async fn purge(&mut self) -> Result<(), String> {
        let mut messages = self.messages.write().await;
        messages.clear();

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