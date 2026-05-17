//! SQL 子系統 — 解析器 (Parser) → 規劃器 (Planner) → 執行器 (Executor)
//!
//! 此模組移植自 sql6 專案，處理 SQL 語言的完整生命週期：
//!
//! 1. **Parser**：將 SQL 字串解析為抽象語法樹 (AST)
//! 2. **Planner**：將 AST 轉換為可執行的查詢計劃 (PlanNode)
//! 3. **Executor**：執行查詢計劃並回傳結果集 (ResultSet)

pub mod parser;
pub mod planner;
pub mod executor;

pub use parser::parse;
pub use executor::{Executor, ResultSet, SqlExecutor};