//! Sync Pub/Sub Implementation

use crate::kv::{KvEngine, KvStore};
use crate::msgq::error::{MsgqError, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPubSubMessage {
    pub id: String,
    pub channel: String,
    pub payload: Vec<u8>,
    pub timestamp: u64,
}

impl SyncPubSubMessage {
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

#[derive(Serialize, Deserialize, Default)]
struct ChannelMessages {
    messages: Vec<SyncPubSubMessage>,
}

pub struct SyncPubSub {
    name: String,
    engine: Arc<RwLock<KvEngine>>,
}

impl SyncPubSub {
    pub fn new(name: &str, engine: Arc<RwLock<KvEngine>>) -> Self {
        Self { name: name.to_string(), engine }
    }

    pub fn publish(&mut self, channel: &str, payload: Vec<u8>) -> Result<String> {
        let msg = SyncPubSubMessage::new(channel, payload);
        let msg_id = msg.id.clone();

        // Get existing messages
        let mut channel_messages = self.get_channel_messages(channel)?;

        // Append new message
        channel_messages.messages.push(msg);

        // Save
        let key = self.channel_messages_key(channel);
        let json = serde_json::to_vec(&channel_messages)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, key.as_bytes(), &json).map_err(MsgqError::Db)?;
        }

        Ok(msg_id)
    }

    pub fn subscribe(&mut self, channel: &str, subscriber_id: &str) -> Result<()> {
        // Get existing subscribers
        let mut subscribers = self.get_subscribers(channel)?;

        // Add if not exists
        if !subscribers.contains(&subscriber_id.to_string()) {
            subscribers.push(subscriber_id.to_string());
        }

        // Save subscribers
        let key = self.channel_subscribers_key(channel);
        let json = serde_json::to_vec(&subscribers)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, key.as_bytes(), &json).map_err(MsgqError::Db)?;
        }

        // Initialize offset to 0
        let offset_key = self.offset_key(channel, subscriber_id);
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, offset_key.as_bytes(), b"0").map_err(MsgqError::Db)?;
        }

        Ok(())
    }

    pub fn unsubscribe(&mut self, channel: &str, subscriber_id: &str) -> Result<()> {
        // Remove from subscribers
        let mut subscribers = self.get_subscribers(channel)?;
        subscribers.retain(|s| s != subscriber_id);

        let key = self.channel_subscribers_key(channel);
        let json = serde_json::to_vec(&subscribers)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, key.as_bytes(), &json).map_err(MsgqError::Db)?;
        }

        // Delete offset
        let offset_key = self.offset_key(channel, subscriber_id);
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.delete(1, offset_key.as_bytes()).map_err(MsgqError::Db)?;
        }

        Ok(())
    }

    pub fn consume(&mut self, channel: &str, subscriber_id: &str) -> Result<Option<SyncPubSubMessage>> {
        // Get offset
        let offset = self.get_offset(channel, subscriber_id)?;

        // Get messages
        let channel_messages = self.get_channel_messages(channel)?;

        if offset < channel_messages.messages.len() {
            let msg = channel_messages.messages[offset].clone();

            // Increment offset
            let new_offset = offset + 1;
            let offset_key = self.offset_key(channel, subscriber_id);
            {
                let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
                guard.put(1, offset_key.as_bytes(), new_offset.to_string().as_bytes()).map_err(MsgqError::Db)?;
            }

            return Ok(Some(msg));
        }

        Ok(None)
    }

    pub fn list_channels(&self) -> Result<Vec<String>> {
        let start = format!("pubsub:{}:messages:", self.name).as_bytes().to_vec();
        let end = format!("pubsub:{}:messages;", self.name).as_bytes().to_vec();
        let start_sub = format!("pubsub:{}:subscribers:", self.name).as_bytes().to_vec();
        let end_sub = format!("pubsub:{}:subscribers;", self.name).as_bytes().to_vec();

        let mut channels = std::collections::HashSet::new();

        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            // Scan for messages keys
            if let Ok(results) = guard.scan(1, &start, &end) {
                for (key, _) in results {
                    let key_str = String::from_utf8_lossy(&key).to_string();
                    if let Some(ch) = key_str.strip_prefix(&format!("pubsub:{}:messages:", self.name)) {
                        channels.insert(ch.to_string());
                    }
                }
            }
            // Also scan for subscribers keys (channels with no messages but with subscribers)
            if let Ok(results) = guard.scan(1, &start_sub, &end_sub) {
                for (key, _) in results {
                    let key_str = String::from_utf8_lossy(&key).to_string();
                    if let Some(ch) = key_str.strip_prefix(&format!("pubsub:{}:subscribers:", self.name)) {
                        channels.insert(ch.to_string());
                    }
                }
            }
        }

        Ok(channels.into_iter().collect())
    }

    pub fn list_subscribers(&self, channel: &str) -> Result<Vec<String>> {
        Ok(self.get_subscribers(channel)?)
    }

    pub fn message_count(&self, channel: &str) -> Result<usize> {
        let channel_messages = self.get_channel_messages(channel)?;
        Ok(channel_messages.messages.len())
    }

    // Internal helpers
    fn channel_messages_key(&self, channel: &str) -> String {
        format!("pubsub:{}:messages:{}", self.name, channel)
    }

    fn channel_subscribers_key(&self, channel: &str) -> String {
        format!("pubsub:{}:subscribers:{}", self.name, channel)
    }

    fn offset_key(&self, channel: &str, subscriber_id: &str) -> String {
        format!("pubsub:{}:offset:{}:{}", self.name, channel, subscriber_id)
    }

    fn get_channel_messages(&self, channel: &str) -> Result<ChannelMessages> {
        let key = self.channel_messages_key(channel);
        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, key.as_bytes()) {
                return Ok(serde_json::from_slice(&data).unwrap_or_default());
            }
        }
        Ok(ChannelMessages::default())
    }

    fn get_subscribers(&self, channel: &str) -> Result<Vec<String>> {
        let key = self.channel_subscribers_key(channel);
        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, key.as_bytes()) {
                return Ok(serde_json::from_slice(&data).unwrap_or_default());
            }
        }
        Ok(vec![])
    }

    fn get_offset(&self, channel: &str, subscriber_id: &str) -> Result<usize> {
        let key = self.offset_key(channel, subscriber_id);
        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, key.as_bytes()) {
                let offset_str = String::from_utf8_lossy(&data);
                return Ok(offset_str.parse().unwrap_or(0));
            }
        }
        Ok(0)
    }
}