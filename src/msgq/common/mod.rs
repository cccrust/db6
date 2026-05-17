//! 共用元件 — 非同步程式設計的基礎工具
//!
//! 提供在多個 msgq 元件間共享的基礎元件：
//! - ConcurrencyLimiter: 基於 tokio Semaphore 的並發控制
//! - GracefulShutdown: 基於 tokio Notify 的優雅關閉機制
//!
//! 設計靈感來自 mini-redis 專案。

mod limiter;
mod shutdown;

pub use limiter::ConcurrencyLimiter;
pub use shutdown::GracefulShutdown;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 預設並發限制數量
pub const DEFAULT_CONCURRENCY_LIMIT: usize = 100;

/// 建立一個新的 AtomicUsize，用於追蹤並發限制
pub fn new_atomic_usize(value: usize) -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(value))
}