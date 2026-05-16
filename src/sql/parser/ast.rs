//! SQL AST 節點定義（移植自 sql6/src/parser/ast.rs）
//!
//! 支援的 SQL 語法：
//! - SELECT / INSERT / UPDATE / DELETE
//! - CREATE TABLE / DROP TABLE
//! - CREATE VIRTUAL TABLE ... USING fts5  ← FTS5 全文檢索
//! - WHERE / ORDER BY / GROUP BY / HAVING / LIMIT
//! - JOIN（僅 BTree，LSM 在 planner 層限制）

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement {
    Select(SelectStmt),
    Insert(InsertStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
    CreateTable(CreateTableStmt),
    CreateVirtualTable(CreateVirtualTableStmt),
    DropTable(DropTableStmt),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectStmt {
    pub columns: Vec<Expr>,
    pub from: Option<String>,
    pub where_clause: Option<Box<Expr>>,
    pub group_by: Option<Vec<Expr>>,
    pub order_by: Option<Vec<OrderBy>>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertStmt {
    pub table: String,
    pub columns: Vec<String>,
    pub values: Vec<Vec<Expr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStmt {
    pub table: String,
    pub set: Vec<(String, Expr)>,
    pub where_clause: Option<Box<Expr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStmt {
    pub table: String,
    pub where_clause: Option<Box<Expr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTableStmt {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub if_not_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVirtualTableStmt {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub tokenize: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropTableStmt {
    pub name: String,
    pub if_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnType {
    Integer, Real, Text, Blob, Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Column(String),
    Literal(Literal),
    FtsMatch(FtsMatchExpr),
    BinaryOp(Box<Expr>, BinOp, Box<Expr>),
    UnaryOp(UnOp, Box<Expr>),
    Function(String, Vec<Expr>),
    Subquery(Box<SelectStmt>),
    Wildcard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtsMatchExpr {
    pub table: String,
    pub query: String,
}

impl Expr {
    pub fn and(self, rhs: Expr) -> Expr {
        Expr::BinaryOp(Box::new(self), BinOp::And, Box::new(rhs))
    }
    pub fn or(self, rhs: Expr) -> Expr {
        Expr::BinaryOp(Box::new(self), BinOp::Or, Box::new(rhs))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinOp {
    And, Or, Not,
    Eq, Ne, Lt, Le, Gt, Ge,
    Plus, Minus, Star, Slash,
    Like, Glob, Match, Regexp, In,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnOp {
    Not, Neg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBy {
    pub expr: Expr,
    pub asc: bool,
}