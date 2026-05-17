//! SQL Queue Module
//!
//! 將 SQL 執行包裝為 Message Queue 服務
//!
//! ```text
//! Client → [SQL Queue] → Worker → Database
//!                     ↑
//!                Message Queue (基於 tokio)
//! ```

mod types;
mod async_sql;
mod sync_sql;

pub use types::{JobResult, ResultStore, SqlJob, SqlResultStore};
pub use async_sql::AsyncSqlExecutor;
pub use sync_sql::SyncSqlExecutor;

use std::collections::HashMap;