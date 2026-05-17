//! SQL 執行器 — 執行查詢計劃並回傳結果集
//!
//! 執行層負責：
//! - 遍歷 PlanNode 執行計劃樹
//! - 呼叫 StorageEngine 進行實際資料操作
//! - JSON 路徑運算（JSON_EXTRACT、JSON_SET 等）
//! - 交易管理（BEGIN/COMMIT/ROLLBACK）
//!
//! 移植自 sql6，修改點為將 pager 改為 StorageEngine trait。

pub mod executor;
pub mod json_path;
pub mod transaction;

pub use executor::{Executor, ResultSet, SqlExecutor};