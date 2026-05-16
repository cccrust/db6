//! SQL executor — query execution (移植自 sql6/src/planner/executor.rs).
//!
//! 需修改：將呼叫 pager 的部分改為呼叫 StorageEngine trait。

pub mod executor;
pub mod transaction;

pub use executor::{Executor, ResultSet, SqlExecutor};