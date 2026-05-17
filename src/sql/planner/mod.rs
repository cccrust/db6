//! SQL 規劃器 — 將 AST 轉換為可執行的查詢計劃
//!
//! 規劃層負責：
//! - 語意檢查（表是否存在、欄位是否存在）
//! - 約束驗證（NOT NULL、UNIQUE、CHECK）
//! - 查詢最佳化（謂詞下推等）
//! - 產生 PlanNode 執行計劃

pub mod plan;
pub mod planner;
pub mod constraints;

pub use planner::Planner;
pub use plan::{Plan, ScanPlan, FtsPlan};