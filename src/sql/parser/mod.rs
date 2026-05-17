//! SQL 解析器 — 詞法分析 (Lexer) → 語法分析 (Parser) → 抽象語法樹 (AST)
//!
//! 包含三個子模組：
//! - `lexer`: 將 SQL 字串切割為 Token 串
//! - `ast`: 定義 SQL 語句的抽象語法樹節點
//! - `parser`: 使用遞迴下降法將 Token 串解析為 AST

pub mod lexer;
pub mod ast;
pub mod parser;

pub use parser::parse;