//! ConcurrencyLimiter — 基於 tokio Semaphore 的並發限制器
//!
//! 類似 mini-redis 的設計：限制同時執行的非同步任務數量，
//! 防止系統資源耗盡。每個任務在開始前需 `acquire()` 一個許可，
//! 結束後許可自動歸還。
//!
//! 與直接使用 `Arc<Semaphore>` 的差異：
//! - 使用 `acquire_owned()` 取得許可，許可的生命期不與借用綁定
//! - 支援動態調整限制數量 `set_limit()`
//! - 可透過 `available()` 查詢目前可用許可數

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// 並發限制器
pub struct ConcurrencyLimiter {
    /// 底層 Semaphore
    semaphore: Arc<Semaphore>,
    /// 限制數量（用於查詢，不影響 Semaphore 實際行為）
    limit: Arc<AtomicUsize>,
}

impl ConcurrencyLimiter {
    /// 建立一個新的並發限制器
    ///
    /// `limit`: 最大並發任務數
    pub fn new(limit: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
            limit: Arc::new(AtomicUsize::new(limit)),
        }
    }

    /// 同 `new()`，另一種命名方式
    pub fn with_limit(limit: usize) -> Self {
        Self::new(limit)
    }

    /// 非同步取得一個許可（可能等待）
    ///
    /// 當所有許可都被佔用時會等待，直到有許可被歸還。
    /// 使用 `acquire_owned()` 確保許可的生命期可跨越非同步邊界。
    pub async fn acquire(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        self.semaphore.clone().acquire_owned().await.map_err(|e| e.to_string())
    }

    /// 嘗試取得一個許可（不等待）
    ///
    /// 如果沒有可用許可，立即回傳 None。
    pub fn try_acquire(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.semaphore.clone().try_acquire_owned().ok()
    }

    /// 查詢目前可用的許可數量
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// 取得目前設定的並發限制
    pub fn limit(&self) -> usize {
        self.limit.load(Ordering::Relaxed)
    }

    /// 動態調整並發限制
    ///
    /// 注意：此方法只更新記錄值，不影響已建立的 Semaphore。
    pub fn set_limit(&self, new_limit: usize) {
        self.limit.store(new_limit, Ordering::Relaxed);
    }
}

impl Clone for ConcurrencyLimiter {
    fn clone(&self) -> Self {
        Self {
            semaphore: self.semaphore.clone(),
            limit: self.limit.clone(),
        }
    }
}