//! SQL 執行器子模組 — 將 SQL 執行包裝為訊息佇列服務
//!
//! ```text
//! Client → [SQL Queue] → Worker → Database
//!                     ↑
//!                Message Queue (基於 tokio)
//! ```
//!
//! 支援同步 (SyncSqlExecutor) 與非同步 (AsyncSqlExecutor) 兩種模式。
//! 非同步版本支援並發限制 (ConcurrencyLimiter) 與優雅關閉 (GracefulShutdown)。

mod types;
mod async_sql;
mod sync_sql;

pub use types::{JobResult, ResultStore, SqlJob, SqlResultStore};
pub use async_sql::AsyncSqlExecutor;
pub use sync_sql::SyncSqlExecutor;

use std::collections::HashMap;