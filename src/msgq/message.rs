//! 同步佇列訊息結構
//!
//! 定義訊息佇列系統中傳遞的基本單位 SyncQueueMessage，
//! 包含訊息 ID、內容、中繼資料與傳遞控制資訊。

use serde::{Deserialize, Serialize};

/// 同步佇列訊息
///
/// - `id`: 唯一識別碼（基於時間戳 + 亂數）
/// - `payload`: 訊息內容（經由 serde_bytes 序列化）
/// - `enqueued_at`: 入隊時間戳（毫秒）
/// - `delivery_count`: 已傳送次數
/// - `visibility_timeout`: 可見性超時（秒）
/// - `visible_after`: 在此時間之前訊息不可見
/// - `priority`: 優先級（0-255，越大越優先）
/// - `metadata`: 中繼資料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueMessage {
    pub id: String,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    pub enqueued_at: u64,
    pub delivery_count: u32,
    pub visibility_timeout: u64,
    pub visible_after: u64,
    pub priority: u8,
    pub metadata: Option<String>,
}

impl SyncQueueMessage {
    /// 建立新的佇列訊息
    ///
    /// 自動產生唯一的訊息 ID（時間戳 + 亂數）。
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

    /// 設定中繼資料（builder 模式）
    pub fn with_metadata(mut self, metadata: String) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// 檢查訊息是否可見
    ///
    /// 基於 `visible_after` 與當前時間比較。
    pub fn is_visible(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        now >= self.visible_after
    }

    /// 將 payload 解析為 UTF-8 字串
    pub fn payload_str(&self) -> Option<String> {
        String::from_utf8(self.payload.clone()).ok()
    }
}
