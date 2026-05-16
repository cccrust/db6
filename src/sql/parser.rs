//! SQL parser — lexer + AST + parser (移植自 sql6/src/parser/).
//!
//! 支援：
//! - SELECT / INSERT / UPDATE / DELETE
//! - CREATE TABLE / DROP TABLE
//! - CREATE VIRTUAL TABLE ... USING fts5  ← FTS5 支援
//! - WHERE, ORDER BY, GROUP BY, HAVING
//! - JOIN (BTree only, LSM 會在 planner 層擋掉)

use crate::error::Result;

pub mod lexer;
pub mod ast;
pub mod parser;

pub fn parse(sql: &str) -> Result<Vec<ast::Statement>> {
    parser::parse(sql).map_err(|e| crate::error::Error::Sql(e.to_string()))
}