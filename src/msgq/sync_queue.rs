//! Sync Queue Implementation

use crate::kv::{KvEngine, KvStore};
use crate::msgq::{error::*, message::SyncQueueMessage};
use std::sync::{Arc, RwLock};

pub struct SyncQueue {
    name: String,
    engine: Arc<RwLock<KvEngine>>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct QueueMeta {
    pub total_enqueued: u64,
    pub completed: u64,
    pub nacked: u64,
}

impl SyncQueue {
    pub fn new(name: &str, engine: Arc<RwLock<KvEngine>>) -> Self {
        Self {
            name: name.to_string(),
            engine,
        }
    }

    pub fn enqueue(&mut self, payload: Vec<u8>, visibility_timeout: u64) -> Result<String> {
        let mut msg = SyncQueueMessage::new(payload, visibility_timeout);
        let msg_id = msg.id.clone();

        let msg_json = serde_json::to_vec(&msg)?;
        let msg_key = format!("queue:{}:msg:{}", self.name, msg_id);
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, msg_key.as_bytes(), &msg_json).map_err(MsgqError::Db)?;
        }

        let mut index = self.read_index()?;
        index.push(msg_id.clone());
        let index_key = format!("queue:{}:index", self.name);
        let index_json = serde_json::to_vec(&index)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, index_key.as_bytes(), &index_json).map_err(MsgqError::Db)?;
        }

        self.update_meta(|m| m.total_enqueued += 1)?;

        Ok(msg_id)
    }

    pub fn dequeue(&mut self, wait_timeout_secs: u64) -> Result<Option<SyncQueueMessage>> {
        let index = self.read_index()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for msg_id in &index {
            let msg_key = format!("queue:{}:msg:{}", self.name, msg_id);
            let data_opt = {
                let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
                guard.get(1, msg_key.as_bytes()).map_err(MsgqError::Db)?
            };

            if let Some(data) = data_opt {
                if let Ok(mut msg) = serde_json::from_slice::<SyncQueueMessage>(&data) {
                    if msg.is_visible() {
                        let in_flight = self.is_inflight(msg_id)?;
                        // If in flight and timeout not expired, skip
                        if in_flight {
                            // Check if timeout expired - if so, remove from inflight
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;
                            if msg.visible_after > now {
                                continue; // still in timeout, skip
                            }
                            // timeout expired, remove from inflight
                            self.remove_inflight(msg_id)?;
                        }

                        self.add_inflight(msg_id)?;

                        msg.delivery_count += 1;
                        msg.visible_after = now + msg.visibility_timeout * 1000;

                        let updated_json = serde_json::to_vec(&msg)?;
                        let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
                        guard.put(1, msg_key.as_bytes(), &updated_json).map_err(MsgqError::Db)?;

                        return Ok(Some(msg));
                    }
                }
            }
        }

        if wait_timeout_secs > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            return self.dequeue(wait_timeout_secs - 1);
        }

        Ok(None)
    }

    pub fn ack(&mut self, msg_id: &str) -> Result<()> {
        let msg_key = format!("queue:{}:msg:{}", self.name, msg_id);

        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if guard.get(1, msg_key.as_bytes()).map_err(MsgqError::Db)?.is_none() {
                return Err(MsgqError::MessageNotFound(msg_id.to_string()));
            }
        }

        self.remove_from_index(msg_id)?;
        self.remove_inflight(msg_id)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.delete(1, msg_key.as_bytes()).map_err(MsgqError::Db)?;
        }
        self.update_meta(|m| m.completed += 1)?;

        Ok(())
    }

    pub fn nack(&mut self, msg_id: &str) -> Result<()> {
        let msg_key = format!("queue:{}:msg:{}", self.name, msg_id);

        let data = {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.get(1, msg_key.as_bytes()).map_err(MsgqError::Db)?
                .ok_or_else(|| MsgqError::MessageNotFound(msg_id.to_string()))?
        };

        let mut msg: SyncQueueMessage = serde_json::from_slice(&data)?;

        self.remove_inflight(msg_id)?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        msg.visible_after = now;

        let updated_json = serde_json::to_vec(&msg)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, msg_key.as_bytes(), &updated_json).map_err(MsgqError::Db)?;
        }
        self.update_meta(|m| m.nacked += 1)?;

        Ok(())
    }

    pub fn peek(&self) -> Result<Option<SyncQueueMessage>> {
        let index = self.read_index()?;

        for msg_id in &index {
            let msg_key = format!("queue:{}:msg:{}", self.name, msg_id);
            let data_opt = {
                let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
                guard.get(1, msg_key.as_bytes()).map_err(MsgqError::Db)?
            };
            if let Some(data) = data_opt {
                if let Ok(msg) = serde_json::from_slice::<SyncQueueMessage>(&data) {
                    if msg.is_visible() {
                        return Ok(Some(msg));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn length(&self) -> Result<usize> {
        let index = self.read_index()?;
        Ok(index.len())
    }

    pub fn purge(&mut self) -> Result<()> {
        let index = self.read_index()?;

        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            for msg_id in &index {
                let msg_key = format!("queue:{}:msg:{}", self.name, msg_id);
                guard.delete(1, msg_key.as_bytes()).map_err(MsgqError::Db)?;
            }

            let index_key = format!("queue:{}:index", self.name);
            guard.delete(1, index_key.as_bytes()).map_err(MsgqError::Db)?;

            let inflight_key = format!("queue:{}:inflight", self.name);
            guard.delete(1, inflight_key.as_bytes()).map_err(MsgqError::Db)?;
        }

        self.reset_meta()?;

        Ok(())
    }

    fn read_index(&self) -> Result<Vec<String>> {
        let index_key = format!("queue:{}:index", self.name);
        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, index_key.as_bytes()) {
                return Ok(serde_json::from_slice(&data)?);
            }
        }
        Ok(vec![])
    }

    fn remove_from_index(&mut self, msg_id: &str) -> Result<()> {
        let index_key = format!("queue:{}:index", self.name);
        let mut index = self.read_index()?;
        index.retain(|id| id != msg_id);
        let index_json = serde_json::to_vec(&index)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, index_key.as_bytes(), &index_json).map_err(MsgqError::Db)?;
        }
        Ok(())
    }

    fn is_inflight(&self, msg_id: &str) -> Result<bool> {
        let inflight_key = format!("queue:{}:inflight", self.name);
        {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, inflight_key.as_bytes()) {
                let inflight: Vec<String> = serde_json::from_slice(&data)?;
                return Ok(inflight.iter().any(|id| id == msg_id));
            }
        }
        Ok(false)
    }

    fn add_inflight(&mut self, msg_id: &str) -> Result<()> {
        let inflight_key = format!("queue:{}:inflight", self.name);
        let mut inflight: Vec<String> = {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, inflight_key.as_bytes()) {
                serde_json::from_slice(&data).unwrap_or_default()
            } else {
                vec![]
            }
        };
        inflight.push(msg_id.to_string());
        let inflight_json = serde_json::to_vec(&inflight)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, inflight_key.as_bytes(), &inflight_json).map_err(MsgqError::Db)?;
        }
        Ok(())
    }

    fn remove_inflight(&mut self, msg_id: &str) -> Result<()> {
        let inflight_key = format!("queue:{}:inflight", self.name);
        let mut inflight: Vec<String> = {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, inflight_key.as_bytes()) {
                serde_json::from_slice(&data).unwrap_or_default()
            } else {
                vec![]
            }
        };
        inflight.retain(|id| id != msg_id);
        let inflight_json = serde_json::to_vec(&inflight)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, inflight_key.as_bytes(), &inflight_json).map_err(MsgqError::Db)?;
        }
        Ok(())
    }

    fn update_meta<F: FnOnce(&mut QueueMeta)>(&mut self, f: F) -> Result<()> {
        let meta_key = format!("queue:{}:meta", self.name);
        let mut meta: QueueMeta = {
            let guard = self.engine.read().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            if let Ok(Some(data)) = guard.get(1, meta_key.as_bytes()) {
                serde_json::from_slice(&data).unwrap_or_default()
            } else {
                QueueMeta::default()
            }
        };
        f(&mut meta);
        let meta_json = serde_json::to_vec(&meta)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, meta_key.as_bytes(), &meta_json).map_err(MsgqError::Db)?;
        }
        Ok(())
    }

    fn reset_meta(&mut self) -> Result<()> {
        let meta_key = format!("queue:{}:meta", self.name);
        let meta = QueueMeta::default();
        let meta_json = serde_json::to_vec(&meta)?;
        {
            let mut guard = self.engine.write().map_err(|_| MsgqError::InvalidEngine("lock poisoned".into()))?;
            guard.put(1, meta_key.as_bytes(), &meta_json).map_err(MsgqError::Db)?;
        }
        Ok(())
    }
}