//! Async Pub/Sub Implementation using tokio::sync::broadcast
//!
//! This implementation uses tokio's broadcast channels for efficient
//! real-time message distribution, similar to mini-redis.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;

pub use crate::msgq::sync_pubsub::{PubSubConfig, TopicMatcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncPubSubMessage {
    pub id: String,
    pub channel: String,
    pub payload: Vec<u8>,
    pub timestamp: u64,
}

impl AsyncPubSubMessage {
    pub fn new(channel: &str, payload: Vec<u8>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let id = format!("{}:{:08x}", now, fastrand::u32(..));

        Self {
            id,
            channel: channel.to_string(),
            payload,
            timestamp: now,
        }
    }

    pub fn payload_str(&self) -> Option<String> {
        String::from_utf8(self.payload.clone()).ok()
    }
}

/// Async pattern subscriber
pub struct AsyncPatternSubscriber {
    pub pattern: String,
    pub receiver: broadcast::Receiver<AsyncPubSubMessage>,
}

/// Async PubSub server
pub struct AsyncPubSub {
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<AsyncPubSubMessage>>>>,
    history: Arc<RwLock<HashMap<String, Vec<AsyncPubSubMessage>>>>,
    config: PubSubConfig,
}

impl AsyncPubSub {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            config: PubSubConfig::default(),
        }
    }

    pub fn with_config(config: PubSubConfig) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn config(&self) -> &PubSubConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: PubSubConfig) {
        self.config = config;
    }

    fn now_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Publish message to channel - all subscribers receive it
    pub async fn publish(&self, channel: &str, payload: Vec<u8>) -> Result<String, String> {
        let msg = AsyncPubSubMessage::new(channel, payload);
        let msg_id = msg.id.clone();

        let mut channels = self.channels.write().await;

        if let Some(sender) = channels.get(channel) {
            let _ = sender.send(msg.clone());
        } else {
            let (tx, _rx) = broadcast::channel(1024);
            let _ = tx.send(msg.clone());
            channels.insert(channel.to_string(), tx);
        }

        if self.config.history_enabled {
            let mut history = self.history.write().await;
            let channel_history = history.entry(channel.to_string()).or_insert_with(Vec::new);
            channel_history.push(msg);
            if channel_history.len() > self.config.max_history {
                channel_history.remove(0);
            }
        }

        Ok(msg_id)
    }

    pub async fn publish_to_topic(&self, topic: &str, payload: Vec<u8>) -> Result<String, String> {
        self.publish(topic, payload).await
    }

    /// Subscribe to a channel - returns a receiver
    pub async fn subscribe(&self, channel: &str) -> Result<broadcast::Receiver<AsyncPubSubMessage>, String> {
        let mut channels = self.channels.write().await;

        let sender = if let Some(existing) = channels.get(channel) {
            existing.clone()
        } else {
            let (tx, _rx) = broadcast::channel(1024);
            channels.insert(channel.to_string(), tx.clone());
            tx
        };

        Ok(sender.subscribe())
    }

    pub async fn subscribe_topic(&self, topic_pattern: &str) -> Result<broadcast::Receiver<AsyncPubSubMessage>, String> {
        self.subscribe(topic_pattern).await
    }

    pub async fn subscribe_pattern(&self, pattern: &str) -> Result<AsyncPatternSubscriber, String> {
        if !self.config.pattern_matching {
            return Err("pattern matching not enabled".to_string());
        }

        let receiver = self.subscribe(pattern).await?;

        Ok(AsyncPatternSubscriber {
            pattern: pattern.to_string(),
            receiver,
        })
    }

    pub async fn subscribe_with_history(
        &self,
        channel: &str,
        history_count: usize,
    ) -> Result<(broadcast::Receiver<AsyncPubSubMessage>, Vec<AsyncPubSubMessage>), String> {
        let receiver = self.subscribe(channel).await?;

        let history = if self.config.history_enabled {
            let history = self.history.read().await;
            if let Some(msgs) = history.get(channel) {
                let max_count = history_count.min(self.config.max_history);
                let start = msgs.len().saturating_sub(max_count);
                msgs[start..].to_vec()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok((receiver, history))
    }

    pub async fn get_history(&self, channel: &str, count: usize) -> Result<Vec<AsyncPubSubMessage>, String> {
        if !self.config.history_enabled {
            return Ok(vec![]);
        }

        let history = self.history.read().await;
        if let Some(msgs) = history.get(channel) {
            let max_count = count.min(self.config.max_history);
            let start = msgs.len().saturating_sub(max_count);
            Ok(msgs[start..].to_vec())
        } else {
            Ok(vec![])
        }
    }

    /// Unsubscribe - just drop the receiver
    pub async fn unsubscribe(&self, _channel: &str) -> Result<(), String> {
        Ok(())
    }

    /// List all channels
    pub async fn list_channels(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }

    /// Get subscriber count
    pub async fn subscriber_count(&self, channel: &str) -> usize {
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(channel) {
            sender.receiver_count()
        } else {
            0
        }
    }
}

impl Default for AsyncPubSub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_pubsub_basic() {
        let ps = AsyncPubSub::new();

        // Subscribe to get receivers
        let mut sub1 = ps.subscribe("news").await.unwrap();
        let mut sub2 = ps.subscribe("news").await.unwrap();

        // Publish
        let id = ps.publish("news", b"Hello".to_vec()).await.unwrap();
        assert!(!id.is_empty());

        // Both subscribers should receive
        let msg1 = sub1.recv().await.unwrap();
        let msg2 = sub2.recv().await.unwrap();

        assert_eq!(msg1.payload, b"Hello");
        assert_eq!(msg2.payload, b"Hello");
    }

    #[tokio::test]
    async fn test_async_pubsub_list_channels() {
        let ps = AsyncPubSub::new();

        ps.subscribe("ch1").await.unwrap();
        ps.publish("ch2", b"msg".to_vec()).await.unwrap();

        let channels = ps.list_channels().await;
        assert!(channels.iter().any(|c| c == "ch1"));
        assert!(channels.iter().any(|c| c == "ch2"));
    }
}