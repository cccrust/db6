//! AST（Abstract Syntax Tree）：SQL 語句的抽象語法樹節點定義
//!
//! ## 設計理念
//!
//! AST 是 SQL 語句的樹狀結構表示，每個節點代表一個語法結構：
//! - **Statement（語句）**：執行的單位，如 SELECT、INSERT
//! - **Expr（運算式）**：有值的表達，如 `1 + 2`、`name LIKE 'A%'`
//! - **SelectItem（選擇項）**：SELECT 後的欄位
//!
//! ## 遍歷方式
//!
//! AST 是遞迴結構，通常使用 visitor 模式遍歷：
//! ```text
//! SELECT name FROM users WHERE age > 18
//!         ↓
//! Statement::Select(SelectStmt {
//!     columns: [SelectItem::Expr(...)],
//!     from: Some(FromItem::Table(...)),
//!     where_: Some(Expr::BinOp(...)),
//! })
//! ```

// ── 頂層語句 ──────────────────────────────────────────────────────────────

/// SQL 語句的根類型列舉
///
/// 所有可執行的 SQL 語句都會被解析為此列舉的某個變體。
///
/// # 變體說明
///
/// | 變體 | 對應 SQL | 說明 |
/// |------|----------|------|
/// | `Select` | SELECT ... | 查詢語句 |
/// | `Insert` | INSERT INTO ... | 插入資料 |
/// | `Update` | UPDATE ... SET ... | 更新資料 |
/// | `Delete` | DELETE FROM ... | 刪除資料 |
/// | `CreateTable` | CREATE TABLE ... | 建立表格 |
/// | `DropTable` | DROP TABLE ... | 刪除表格 |
/// | `CreateIndex` | CREATE INDEX ... | 建立索引 |
/// | `Begin` | BEGIN | 開始交易 |
/// | `Commit` | COMMIT | 提交交易 |
/// | `Rollback` | ROLLBACK | 回滾交易 |
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// SELECT 查詢語句
    Select(SelectStmt),
    /// INSERT 插入語句
    Insert(InsertStmt),
    /// UPDATE 更新語句
    Update(UpdateStmt),
    /// DELETE 刪除語句
    Delete(DeleteStmt),
    /// CREATE TABLE 建立表格
    CreateTable(CreateTableStmt),
    /// DROP TABLE 刪除表格
    DropTable(DropTableStmt),
    /// CREATE INDEX 建立索引
    CreateIndex(CreateIndexStmt),
    /// DROP INDEX 刪除索引
    DropIndex(DropIndexStmt),
    /// ALTER TABLE 修改表格結構
    AlterTable(AlterTableStmt),
    /// PRAGMA 指令
    Pragma(PragmaStmt),
    /// EXPLAIN 查詢计划
    Explain(ExplainStmt),
    /// CREATE VIEW 建立視圖
    CreateView(CreateViewStmt),
    /// DROP VIEW 刪除視圖
    DropView(DropViewStmt),
    /// CREATE TRIGGER 建立觸發器
    CreateTrigger(CreateTriggerStmt),
    /// DROP TRIGGER 刪除觸發器
    DropTrigger(DropTriggerStmt),
    /// REINDEX 重建索引
    Reindex(ReindexStmt),
    /// ANALYZE 分析資料庫
    Analyze(AnalyzeStmt),
    /// ATTACH DATABASE 附加資料庫
    Attach { path: String, alias: String },
    /// DETACH DATABASE 分離資料庫
    Detach { alias: String },
    /// VACUUM 清理資料庫
    Vacuum,
    /// BEGIN 開始交易
    Begin,
    /// COMMIT 提交交易
    Commit,
    /// ROLLBACK 回滾交易
    Rollback,
}

// ── SELECT ────────────────────────────────────────────────────────────────

/// SELECT 查詢語句結構
///
/// 包含查詢的所有子句：
/// - WITH：CTE（公用表達式）
/// - SELECT：DISTINCT、欄位列表
/// - FROM：資料來源、JOIN
/// - WHERE：過濾條件
/// - GROUP BY / HAVING：分組
/// - ORDER BY：排序
/// - LIMIT / OFFSET：分頁
/// - UNION：集合運算
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStmt {
    /// WITH ... AS (...)  公用表達式
    pub with:      Vec<Cte>,
    /// DISTINCT 去重複
    pub distinct:  bool,
    /// 選擇的欄位列表
    pub columns:   Vec<SelectItem>,
    /// FROM 子句（表格名稱或子查詢）
    pub from:      Option<FromItem>,
    /// JOIN 子句列表
    pub joins:     Vec<Join>,
    /// WHERE 條件
    pub where_:    Option<Expr>,
    /// GROUP BY 分組欄位
    pub group_by:  Vec<Expr>,
    /// HAVING 條件（分組後過濾）
    pub having:    Option<Expr>,
    /// ORDER BY 排序
    pub order_by:  Vec<OrderItem>,
    /// LIMIT 限制筆數
    pub limit:     Option<Expr>,
    /// OFFSET 偏移量
    pub offset:    Option<Expr>,
    /// UNION 集合運算（right, is_all）
    pub union_with: Option<Box<(SelectStmt, bool)>>,
}

/// SELECT 的欄位選擇項
///
/// 有三種形式：
/// - `Star`：*（所有欄位）
/// - `TableStar`：table.*（指定表的所有欄位）
/// - `Expr`：運算式，可帶別名
#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    /// * 所有欄位
    Star,
    /// table.* 指定表格的所有欄位
    TableStar(String),
    /// 運算式，可選別名
    Expr { expr: Expr, alias: Option<String> },
}

/// 表格引用（含可選別名）
///
/// 用於 FROM、JOIN 中引用表格
#[derive(Debug, Clone, PartialEq)]
pub struct TableRef {
    /// 表格名稱
    pub name:  String,
    /// 別名（AS 之後的名稱）
    pub alias: Option<String>,
}

/// FROM 子句的資料來源
///
/// 可以是：
/// - 表格名稱（含可選別名）
/// - 子查詢（必須帶別名）
#[derive(Debug, Clone, PartialEq)]
pub enum FromItem {
    /// 表格引用
    Table(TableRef),
    /// 子查詢（需帶別名）
    Subquery { query: Box<SelectStmt>, alias: String },
}

/// CTE（Common Table Expression）公用表達式
///
/// 語法：`WITH name AS (query)`
///
/// # 範例
/// ```sql
/// WITH active_users AS (
///     SELECT * FROM users WHERE active = true
/// )
/// SELECT * FROM active_users WHERE id > 100
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    /// CTE 名稱
    pub name:  String,
    /// 查詢定義
    pub query: Box<SelectStmt>,
}

/// JOIN 連接操作
///
/// 包含連接的類型、目標表格、連接條件
#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    /// 連接類型
    pub kind:      JoinKind,
    /// 目標表格引用
    pub table:     TableRef,
    /// 連接條件
    pub condition: JoinCondition,
}

/// JOIN 連接類型
///
/// | 類型 | 說明 |
/// |------|------|
/// | Inner | 內連接，只保留匹配列 |
/// | Left | 左外連接，保留左表所有列 |
/// | Right | 右外連接，保留右表所有列 |
/// | Full | 全外連接 |
/// | Cross | 交叉連接（笛卡爾積） |
/// | Natural | 自然連接（同名欄位自動匹配） |
#[derive(Debug, Clone, PartialEq)]
pub enum JoinKind {
    Inner, Left, Right, Full, Cross, Natural,
}

/// JOIN 連接條件
///
/// | 類型 | 語法 |
/// |------|------|
/// | On | ON expr |
/// | Using | USING (col1, col2, ...) |
/// | None | 無條件（只用於 CROSS JOIN） |
#[derive(Debug, Clone, PartialEq)]
pub enum JoinCondition {
    On(Expr),
    Using(Vec<String>),
    None,
}

/// ORDER BY 排序項
///
/// # 範例
/// ```sql
/// ORDER BY name ASC, created_at DESC
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderItem {
    /// 排序的運算式
    pub expr: Expr,
    /// 是否升序（true=ASC, false=DESC）
    pub asc:  bool,
}

// ── INSERT ────────────────────────────────────────────────────────────────

/// INSERT 插入語句
///
/// # 語法
/// ```sql
/// INSERT INTO table (col1, col2, ...) VALUES (v1, v2, ...), ...
/// INSERT INTO table DEFAULT VALUES
/// INSERT INTO table ... ON CONFLICT DO NOTHING
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStmt {
    /// 目標表格名稱
    pub table:   String,
    /// 欄位名稱列表（空表示不指定，使用所有欄位）
    pub columns: Vec<String>,
    /// 要插入的值（多組用於批量插入）
    pub values:  Vec<Vec<Expr>>,
    /// 是否為 DEFAULT VALUES
    pub default_values: bool,
    /// ON CONFLICT 處理方式
    pub on_conflict: Option<OnConflict>,
}

/// ON CONFLICT 衝突處理策略
#[derive(Debug, Clone, PartialEq)]
pub enum OnConflict {
    /// DO NOTHING（忽略衝突）
    DoNothing,
    /// DO UPDATE SET column = value（更新現有列）
    DoUpdate { column: String, value: Expr },
}

// ── UPDATE ────────────────────────────────────────────────────────────────

/// UPDATE 更新語句
///
/// # 語法
/// ```sql
/// UPDATE table SET col1 = val1, col2 = val2 WHERE condition
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStmt {
    /// 目標表格
    pub table:   String,
    /// 要更新的欄位與值
    pub sets:    Vec<(String, Expr)>,
    /// WHERE 條件（可選）
    pub where_:  Option<Expr>,
}

// ── DELETE ────────────────────────────────────────────────────────────────

/// DELETE 刪除語句
///
/// # 語法
/// ```sql
/// DELETE FROM table WHERE condition
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStmt {
    /// 目標表格
    pub table:  String,
    /// WHERE 條件（可選，全刪除時為 None）
    pub where_: Option<Expr>,
}

// ── CREATE TABLE ─────────────────────────────────────────────────────────

/// CREATE TABLE 建立表格語句
///
/// # 語法
/// ```sql
/// CREATE TABLE [IF NOT EXISTS] name (
///     column1 type [constraints],
///     column2 type [constraints],
///     [table_constraints]
/// )
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStmt {
    /// IF NOT EXISTS
    pub if_not_exists: bool,
    /// 表格名稱
    pub name:          String,
    /// 欄位定義列表
    pub columns:       Vec<ColumnDef>,
    /// 表格層級約束
    pub constraints:   Vec<TableConstraint>,
}

/// 欄位定義
///
/// 包含欄位名稱、類型、約束
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name:        String,
    pub data_type:   SqlType,
    pub constraints: Vec<ColumnConstraint>,
}

/// SQL 資料類型
///
/// | 類型 | 說明 |
/// |------|------|
/// | Integer | 64 位元帶符號整數 |
/// | Real | 64 位元浮點數 |
/// | Text | UTF-8 字串 |
/// | Blob | 二進位資料 |
/// | Boolean | true/false |
/// | Null | NULL 值 |
#[derive(Debug, Clone, PartialEq)]
pub enum SqlType {
    Integer, Real, Text, Blob, Boolean, Null,
}

/// 欄位層級約束
///
/// | 約束 | 說明 |
/// |------|------|
/// | NotNull | 非空 |
/// | PrimaryKey | 主鍵 |
/// | Unique | 唯一 |
/// | Default(expr) | 預設值 |
/// | Check(expr) | CHECK 約束 |
/// | References | 外鍵參照 |
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    /// 非空約束
    NotNull,
    /// 主鍵約束
    PrimaryKey { autoincrement: bool },
    /// 唯一約束
    Unique,
    /// 預設值
    Default(Expr),
    /// CHECK 約束
    Check(Expr),
    /// 外鍵參照
    References { table: String, column: Option<String> },
}

/// 表格層級約束
#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    /// 主鍵約束（多欄位）
    PrimaryKey(Vec<String>),
    /// 唯一約束（多欄位）
    Unique(Vec<String>),
}

// ── DROP TABLE ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DropTableStmt {
    pub if_exists: bool,
    pub name:      String,
}

// ── CREATE INDEX ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStmt {
    pub unique:    bool,
    pub name:      String,
    pub table:     String,
    pub columns:   Vec<String>,
}

// ── DROP INDEX ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DropIndexStmt {
    pub if_exists: bool,
    pub name:      String,
}

// ── ALTER TABLE ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableOp {
    RenameTo(String),
    AddColumn { name: String, data_type: SqlType },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTableStmt {
    pub table: String,
    pub op:    AlterTableOp,
}

// ── PRAGMA ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct PragmaStmt {
    pub name:  String,
    pub value: Option<Expr>,
}

// ── EXPLAIN ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ExplainStmt {
    pub inner: Box<Statement>,
}

// ── CREATE VIEW ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CreateViewStmt {
    pub if_not_exists: bool,
    pub temp:         bool,
    pub name:         String,
    pub query:        Box<SelectStmt>,
}

// ── DROP VIEW ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DropViewStmt {
    pub if_exists: bool,
    pub name:      String,
}

// ── TRIGGER ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTriggerStmt {
    pub if_not_exists: bool,
    pub name:          String,
    pub table:         String,
    pub timing:        TriggerTiming,
    pub event:         TriggerEvent,
    pub for_each_row:  bool,
    pub when:          Option<Box<Expr>>,
    pub body:          String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerEvent {
    Delete,
    Insert,
    Update(Option<Vec<String>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTriggerStmt {
    pub if_exists: bool,
    pub name:      String,
}

// ── REINDEX ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ReindexStmt {
    pub name: Option<String>,
}

// ── ANALYZE ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyzeStmt {
    pub name: Option<String>,
}

// ── 運算式 ────────────────────────────────────────────────────────────────

/// 運算式（Expression）
///
/// 運算式是有值的語法結構，用於：
/// - SELECT 的欄位
/// - WHERE 條件
/// - SET 子句
/// - VALUES 子句
///
/// # 運算式類型
///
/// | 類型 | 範例 | 說明 |
/// |------|------|------|
/// | LitInt | `42` | 整數常值 |
/// | LitFloat | `3.14` | 浮點常值 |
/// | LitStr | `'hello'` | 字串常值 |
/// | LitBool | `TRUE` | 布林常值 |
/// | LitNull | `NULL` | 空值 |
/// | Column | `name`, `t.name` | 欄位參照 |
/// | Function | `COUNT(*)` | 函式呼叫 |
/// | BinOp | `a + b`, `x > 5` | 二元運算 |
/// | UnaryOp | `-x`, `NOT y` | 一元運算 |
/// | IsNull | `x IS NULL` | 空值判斷 |
/// | Between | `n BETWEEN 1 AND 10` | 範圍判斷 |
/// | InList | `x IN (1, 2, 3)` | 列表成員判斷 |
/// | Like | `name LIKE 'A%'` | 模糊匹配 |
/// | Cast | `CAST(x AS INTEGER)` | 類型轉換 |
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // ── 字面值 ────────────────────────────────────────────────────────────

    /// 整數常值
    LitInt(i64),
    /// 浮點數常值
    LitFloat(f64),
    /// 字串常值（單引號包圍）
    LitStr(String),
    /// 布林常值（TRUE / FALSE）
    LitBool(bool),
    /// 空值
    LitNull,

    // ── 欄位參照 ─────────────────────────────────────────────────────────

    /// 欄位參照
    ///
    /// # 範例
    /// - `name` → Column { table: None, name: "name" }
    /// - `t.name` → Column { table: Some("t"), name: "name" }
    Column { table: Option<String>, name: String },

    // ── 函式 ─────────────────────────────────────────────────────────────

    /// 函式呼叫
    ///
    /// # 範例
    /// - `COUNT(*)` → Function { name: "COUNT", args: [*], distinct: false }
    /// - `SUM(DISTINCT x)` → Function { name: "SUM", args: [x], distinct: true }
    Function { name: String, args: Vec<Expr>, distinct: bool },

    // ── 二元運算 ─────────────────────────────────────────────────────────

    /// 二元運算（left op right）
    ///
    /// # 支援的運算子
    /// - 比較：=, !=, <, <=, >, >=
    /// - 邏輯：AND, OR
    /// - 算術：+, -, *, /, %
    /// - 字串：||
    BinOp { left: Box<Expr>, op: BinOp, right: Box<Expr> },

    // ── 一元運算 ─────────────────────────────────────────────────────────

    /// 一元運算（op expr）
    ///
    /// # 支援的運算子
    /// - Neg：負號（-x）
    /// - Not：邏輯非（NOT x）
    UnaryOp { op: UnaryOp, expr: Box<Expr> },

    // ── 空值判斷 ─────────────────────────────────────────────────────────

    /// IS [NOT] NULL
    ///
    /// # 範例
    /// - `x IS NULL` → IsNull { expr: x, negated: false }
    /// - `x IS NOT NULL` → IsNull { expr: x, negated: true }
    IsNull  { expr: Box<Expr>, negated: bool },

    // ── 範圍判斷 ─────────────────────────────────────────────────────────

    /// BETWEEN ... AND ...
    ///
    /// # 範例
    /// - `age BETWEEN 18 AND 65` → Between { expr: age, low: 18, high: 65, negated: false }
    /// - `age NOT BETWEEN 18 AND 65` → negated: true
    Between { expr: Box<Expr>, low: Box<Expr>, high: Box<Expr>, negated: bool },

    // ── 列表成員判斷 ─────────────────────────────────────────────────────

    /// IN (...) 列表判斷
    ///
    /// # 範例
    /// - `id IN (1, 2, 3)` → InList { expr: id, list: [1, 2, 3], negated: false }
    InList  { expr: Box<Expr>, list: Vec<Expr>, negated: bool },

    /// IN (SELECT ...) 子查詢
    InSubquery { expr: Box<Expr>, query: Box<SelectStmt>, negated: bool },

    /// EXISTS (SELECT ...)
    Exists { query: Box<SelectStmt>, negated: bool },

    /// 純量子查詢（當作單一值使用）
    ScalarSubquery(Box<SelectStmt>),

    // ── 模糊匹配 ─────────────────────────────────────────────────────────

    /// LIKE 模糊匹配
    ///
    /// # 範例
    /// - `name LIKE 'A%'` → Like { expr: name, pattern: 'A%', negated: false }
    Like    { expr: Box<Expr>, pattern: Box<Expr>, negated: bool },

    /// GLOB 模糊匹配（區分大小寫，使用 * 和 ?）
    Glob    { expr: Box<Expr>, pattern: Box<Expr>, negated: bool },

    // ── 類型轉換 ─────────────────────────────────────────────────────────

    /// CAST(expr AS type)
    Cast { expr: Box<Expr>, to: SqlType },

    /// 子查詢（預留）
    Subquery(Box<SelectStmt>),
}

/// 二元運算子
///
/// # 類別
/// - 比較運算子：Eq, NotEq, Lt, LtEq, Gt, GtEq
/// - 邏輯運算子：And, Or
/// - 算術運算子：Add, Sub, Mul, Div, Mod
/// - 字串運算子：Concat（||）
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Eq, NotEq, Lt, LtEq, Gt, GtEq,  // 比較
    And, Or,                         // 邏輯
    Add, Sub, Mul, Div, Mod,         // 算術
    Concat,                           // 字串連接（||）
}

/// 一元運算子
///
/// | 運算子 | 說明 | 範例 |
/// |--------|------|------|
/// | Neg | 負號 | -5 |
/// | Not | 邏輯非 | NOT x |
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,    // -
    Not,    // NOT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_lit_int() {
        let e = Expr::LitInt(42);
        assert!(matches!(e, Expr::LitInt(42)));
    }

    #[test]
    fn test_expr_lit_float() {
        let e = Expr::LitFloat(3.14);
        assert!(matches!(e, Expr::LitFloat(3.14)));
    }

    #[test]
    fn test_expr_lit_str() {
        let e = Expr::LitStr("hello".to_string());
        assert!(matches!(e, Expr::LitStr(s) if s == "hello"));
    }

    #[test]
    fn test_expr_lit_bool() {
        let t = Expr::LitBool(true);
        let f = Expr::LitBool(false);
        assert!(matches!(t, Expr::LitBool(true)));
        assert!(matches!(f, Expr::LitBool(false)));
    }

    #[test]
    fn test_expr_lit_null() {
        let e = Expr::LitNull;
        assert!(matches!(e, Expr::LitNull));
    }

    #[test]
    fn test_expr_column() {
        let e = Expr::Column { table: None, name: "id".to_string() };
        assert!(matches!(e, Expr::Column { table: None, name } if name == "id"));
    }

    #[test]
    fn test_expr_column_with_table() {
        let e = Expr::Column { table: Some("users".to_string()), name: "id".to_string() };
        assert!(matches!(e, Expr::Column { table: Some(t), name } if t == "users" && name == "id"));
    }

    #[test]
    fn test_expr_function() {
        let e = Expr::Function { name: "COUNT".to_string(), args: vec![Expr::Column { table: None, name: "*".to_string() }], distinct: false };
        assert!(matches!(e, Expr::Function { name, .. } if name == "COUNT"));
    }

    #[test]
    fn test_expr_binop() {
        use BinOp::*;
        let e = Expr::BinOp {
            left: Box::new(Expr::LitInt(1)),
            op: Add,
            right: Box::new(Expr::LitInt(2)),
        };
        assert!(matches!(e, Expr::BinOp { op: Add, .. }));
    }

    #[test]
    fn test_expr_unary_op() {
        let e = Expr::UnaryOp { op: UnaryOp::Neg, expr: Box::new(Expr::LitInt(5)) };
        assert!(matches!(e, Expr::UnaryOp { op: UnaryOp::Neg, .. }));
    }

    #[test]
    fn test_expr_is_null() {
        let e = Expr::IsNull { expr: Box::new(Expr::Column { table: None, name: "x".to_string() }), negated: false };
        assert!(matches!(e, Expr::IsNull { negated: false, .. }));
    }

    #[test]
    fn test_expr_is_not_null() {
        let e = Expr::IsNull { expr: Box::new(Expr::Column { table: None, name: "x".to_string() }), negated: true };
        assert!(matches!(e, Expr::IsNull { negated: true, .. }));
    }

    #[test]
    fn test_binop_equality() {
        let eq = BinOp::Eq;
        let ne = BinOp::NotEq;
        assert!(matches!(eq, BinOp::Eq));
        assert!(matches!(ne, BinOp::NotEq));
    }

    #[test]
    fn test_binop_comparison() {
        assert!(matches!(BinOp::Lt, BinOp::Lt));
        assert!(matches!(BinOp::LtEq, BinOp::LtEq));
        assert!(matches!(BinOp::Gt, BinOp::Gt));
        assert!(matches!(BinOp::GtEq, BinOp::GtEq));
    }

    #[test]
    fn test_binop_logical() {
        assert!(matches!(BinOp::And, BinOp::And));
        assert!(matches!(BinOp::Or, BinOp::Or));
    }

    #[test]
    fn test_binop_arithmetic() {
        assert!(matches!(BinOp::Add, BinOp::Add));
        assert!(matches!(BinOp::Sub, BinOp::Sub));
        assert!(matches!(BinOp::Mul, BinOp::Mul));
        assert!(matches!(BinOp::Div, BinOp::Div));
        assert!(matches!(BinOp::Mod, BinOp::Mod));
    }

    #[test]
    fn test_unary_op() {
        assert!(matches!(UnaryOp::Neg, UnaryOp::Neg));
        assert!(matches!(UnaryOp::Not, UnaryOp::Not));
    }

    #[test]
    fn test_select_item_star() {
        assert!(matches!(SelectItem::Star, SelectItem::Star));
    }

    #[test]
    fn test_select_item_table_star() {
        let si = SelectItem::TableStar("users".to_string());
        match si {
            SelectItem::TableStar(s) => assert_eq!(s, "users"),
            _ => panic!("not TableStar"),
        }
    }

    #[test]
    fn test_select_item_expr() {
        let si = SelectItem::Expr { expr: Expr::LitInt(1), alias: Some("one".to_string()) };
        assert!(matches!(si, SelectItem::Expr { alias: Some(a), .. } if a == "one"));
    }

    #[test]
    fn test_table_ref() {
        let t = TableRef { name: "users".to_string(), alias: Some("u".to_string()) };
        assert_eq!(t.name, "users");
        assert_eq!(t.alias, Some("u".to_string()));
    }

    #[test]
    fn test_from_item_table() {
        let fi = FromItem::Table(TableRef { name: "users".to_string(), alias: None });
        assert!(matches!(fi, FromItem::Table(t) if t.name == "users"));
    }

    #[test]
    fn test_order_item() {
        let oi = OrderItem { expr: Expr::LitInt(1), asc: true };
        assert!(oi.asc);
        assert!(matches!(oi.expr, Expr::LitInt(1)));
    }

    #[test]
    fn test_join_kind() {
        assert!(matches!(JoinKind::Inner, JoinKind::Inner));
        assert!(matches!(JoinKind::Left, JoinKind::Left));
        assert!(matches!(JoinKind::Right, JoinKind::Right));
        assert!(matches!(JoinKind::Full, JoinKind::Full));
        assert!(matches!(JoinKind::Cross, JoinKind::Cross));
        assert!(matches!(JoinKind::Natural, JoinKind::Natural));
    }

    #[test]
    fn test_join_condition() {
        let on = JoinCondition::On(Expr::LitInt(1));
        let using = JoinCondition::Using(vec!["id".to_string()]);
        let none = JoinCondition::None;
        assert!(matches!(on, JoinCondition::On(_)));
        assert!(matches!(using, JoinCondition::Using(_)));
        assert!(matches!(none, JoinCondition::None));
    }

    #[test]
    fn test_insert_stmt() {
        let stmt = InsertStmt {
            table: "users".to_string(),
            columns: vec!["name".to_string()],
            values: vec![vec![Expr::LitStr("Alice".to_string())]],
            default_values: false,
            on_conflict: None,
        };
        assert_eq!(stmt.table, "users");
        assert_eq!(stmt.columns.len(), 1);
    }

    #[test]
    fn test_update_stmt() {
        let stmt = UpdateStmt {
            table: "users".to_string(),
            sets: vec![("age".to_string(), Expr::LitInt(30))],
            where_: None,
        };
        assert_eq!(stmt.table, "users");
        assert_eq!(stmt.sets.len(), 1);
    }

    #[test]
    fn test_delete_stmt() {
        let stmt = DeleteStmt { table: "users".to_string(), where_: None };
        assert_eq!(stmt.table, "users");
    }

    #[test]
    fn test_create_table_stmt() {
        let stmt = CreateTableStmt {
            if_not_exists: true,
            name: "users".to_string(),
            columns: vec![],
            constraints: vec![],
        };
        assert!(stmt.if_not_exists);
        assert_eq!(stmt.name, "users");
    }

    #[test]
    fn test_drop_table_stmt() {
        let stmt = DropTableStmt { if_exists: true, name: "users".to_string() };
        assert!(stmt.if_exists);
        assert_eq!(stmt.name, "users");
    }

    #[test]
    fn test_sql_type() {
        assert!(matches!(SqlType::Integer, SqlType::Integer));
        assert!(matches!(SqlType::Real, SqlType::Real));
        assert!(matches!(SqlType::Text, SqlType::Text));
        assert!(matches!(SqlType::Blob, SqlType::Blob));
        assert!(matches!(SqlType::Boolean, SqlType::Boolean));
        assert!(matches!(SqlType::Null, SqlType::Null));
    }

    #[test]
    fn test_column_constraint() {
        let not_null = ColumnConstraint::NotNull;
        let pk = ColumnConstraint::PrimaryKey { autoincrement: false };
        let unique = ColumnConstraint::Unique;
        assert!(matches!(not_null, ColumnConstraint::NotNull));
        assert!(matches!(pk, ColumnConstraint::PrimaryKey { autoincrement: false }));
        assert!(matches!(unique, ColumnConstraint::Unique));
    }

    #[test]
    fn test_trigger_timing() {
        assert!(matches!(TriggerTiming::Before, TriggerTiming::Before));
        assert!(matches!(TriggerTiming::After, TriggerTiming::After));
        assert!(matches!(TriggerTiming::InsteadOf, TriggerTiming::InsteadOf));
    }

    #[test]
    fn test_trigger_event() {
        assert!(matches!(TriggerEvent::Delete, TriggerEvent::Delete));
        assert!(matches!(TriggerEvent::Insert, TriggerEvent::Insert));
        assert!(matches!(TriggerEvent::Update(None), TriggerEvent::Update(None)));
        if let TriggerEvent::Update(Some(cols)) = TriggerEvent::Update(Some(vec!["id".to_string()])) {
            assert_eq!(cols.len(), 1);
        } else {
            panic!("expected Update with Some");
        }
    }

    #[test]
    fn test_statement() {
        let s = Statement::Begin;
        assert!(matches!(s, Statement::Begin));

        let s = Statement::Commit;
        assert!(matches!(s, Statement::Commit));

        let s = Statement::Rollback;
        assert!(matches!(s, Statement::Rollback));

        let s = Statement::Vacuum;
        assert!(matches!(s, Statement::Vacuum));
    }

    #[test]
    fn test_expr_clone() {
        let e = Expr::LitInt(42);
        let cloned = e.clone();
        assert_eq!(e, cloned);
    }

    #[test]
    fn test_statement_clone() {
        let stmt = Statement::Begin;
        let cloned = stmt.clone();
        assert_eq!(stmt, cloned);
    }
}
