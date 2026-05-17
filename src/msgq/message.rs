//! Sync Queue Message Structure

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueMessage {
    pub id: String,
    pub payload: Vec<u8>,
    pub enqueued_at: u64,
    pub delivery_count: u32,
    pub visibility_timeout: u64,
    pub visible_after: u64,
    pub priority: u8,
    pub metadata: Option<String>,
}

impl SyncQueueMessage {
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
            delivery_count: 0,
            visibility_timeout,
            visible_after: 0,
            priority: 0,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: String) -> Self {
        self.metadata = Some(metadata);
        self
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