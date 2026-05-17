//! 詞法分析器 (Lexer)：將 SQL 字串切割為 Token 串
//!
//! 詞法分析是 SQL 處理的第一個階段。Lexer 讀取原始 SQL 字串，
//! 按照 SQL 語言的詞法規則，將其分割為具有意義的最小單元——Token。
//!
//! 例如，`SELECT * FROM users` 會被切割為：
//! ```text
//! [Select, Star, From, Ident("users"), Eof]
//! ```
//!
//! ## 處理方式
//!
//! 採用單一字元往前看的實作方式，逐字讀取輸入字串：
//! - 遇到字母或底線 → 讀取整個識別符/關鍵字
//! - 遇到數字 → 讀取整個數字（整數或浮點數）
//! - 遇到單引號 → 讀取字串字面值
//! - 遇到特殊字元 → 判斷運算子或標點
//! - 跳過空白與註解
//!
//! ## 支援的 Token 類型
//!
//! - **關鍵字**：SELECT、FROM、WHERE、INSERT、CREATE 等約 80+ 個 SQL 關鍵字
//! - **識別符**：表名、欄位名（Ident(String)）
//! - **字面值**：整數 (LitInt)、浮點數 (LitFloat)、字串 (LitStr)、NULL
//! - **運算子**：=、!=、<、>、<=、>=、+、-、*、/、%、||
//! - **標點**：(、)、,、;、.

/// SQL 詞法單元 (Token)
///
/// 每個 Token 代表 SQL 字串中的一個最小語義單元。
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── SQL 關鍵字 ──────────────────────────────────────────────────────
    /// SELECT 查詢關鍵字
    Select, From, Where, Insert, Into, Values,
    /// UPDATE/DELETE 操作關鍵字
    Update, Set, Delete,
    /// CREATE/DROP/ALTER DDL 關鍵字
    Create, Drop, Table,
    /// 索引相關
    Index, On, Primary, Key,
    /// 約束條件
    Not, Null, Unique,
    /// 邏輯運算
    And, Or, Is, In, Like, Between,
    /// ORDER BY 排序相關
    Order, By, Asc, Desc, Limit, Offset,
    /// JOIN 相關
    Join, Inner, Left, Right, Outer, Cross, Natural, Using,
    /// GROUP BY 聚合相關
    Group, Having, Distinct, All, As,
    /// 條件判斷
    If, Exists,
    /// 交易控制
    Begin, Commit, Rollback, Transaction,
    /// FTS5 全文搜尋
    Virtual, Match,
    /// CTE 公用表表達式
    With, Recursive,
    /// FOREIGN KEY 外部鍵約束
    References,
    /// 資料型別
    KwInteger, KwText, Real, Blob, Boolean,
    /// 布林字面值
    True, False,
    /// 其他 SQL 功能關鍵字
    Pragma, Explain, Alter, Rename, To, Add, Column, Do, Of,
    View, Reindex, Analyze, Temp, Conflict, Nothing, Union, Check, Cast, Default, GLOB,
    Trigger, Before, After, Instead, Each, Row, For, When, End, AutoIncrement,
    Attach, Detach, Database, Vacuum, Backup,

    // ── 識別符 ──────────────────────────────────────────────────────────
    /// 表名、欄位名等識別符（附原始字串）
    Ident(String),

    // ── 字面值 ──────────────────────────────────────────────────────────
    /// 整數字面值
    LitInt(i64),
    /// 浮點數字面值
    LitFloat(f64),
    /// 字串字面值（單引號包圍）
    LitStr(String),
    /// NULL 字面值
    LitNull,

    // ── 運算子 ──────────────────────────────────────────────────────────
    /// 等於 =`
    Eq,
    /// 不等於 `!=` 或 `<>`
    NotEq,
    /// 小於 `<`
    Lt,
    /// 小於等於 `<=`
    LtEq,
    /// 大於 `>`
    Gt,
    /// 大於等於 `>=`
    GtEq,
    /// 加號 `+`
    Plus,
    /// 減號 `-`
    Minus,
    /// 星號 `*`（SELECT * 或乘法）
    Star,
    /// 除號 `/`
    Slash,
    /// 百分號 `%`（取餘）
    Percent,
    /// 字串串接 `||`
    Concat,

    // ── 標點符號 ────────────────────────────────────────────────────────
    /// 左括號 `(`
    LParen,
    /// 右括號 `)`
    RParen,
    /// 逗號 `,`
    Comma,
    /// 分號 `;`
    Semicolon,
    /// 點號 `.`
    Dot,

    // ── 特殊 Token ──────────────────────────────────────────────────────
    /// 輸入結尾
    Eof,
    /// JSON Path 前綴 `@`（如 `@.field`）
    At,
}

// ── 關鍵字對照表 ─────────────────────────────────────────────────────────

fn keyword(s: &str) -> Option<Token> {
    match s.to_uppercase().as_str() {
        "SELECT"      => Some(Token::Select),
        "FROM"        => Some(Token::From),
        "WHERE"       => Some(Token::Where),
        "INSERT"      => Some(Token::Insert),
        "INTO"        => Some(Token::Into),
        "VALUES"      => Some(Token::Values),
        "UPDATE"      => Some(Token::Update),
        "SET"         => Some(Token::Set),
        "DELETE"      => Some(Token::Delete),
        "CREATE"      => Some(Token::Create),
        "DROP"        => Some(Token::Drop),
        "TABLE"       => Some(Token::Table),
        "INDEX"       => Some(Token::Index),
        "ON"          => Some(Token::On),
        "PRIMARY"     => Some(Token::Primary),
        "KEY"         => Some(Token::Key),
        "REFERENCES"  => Some(Token::References),
        "NOT"         => Some(Token::Not),
        "NULL"        => Some(Token::LitNull),
        "UNIQUE"      => Some(Token::Unique),
        "AND"         => Some(Token::And),
        "OR"          => Some(Token::Or),
        "IS"          => Some(Token::Is),
        "IN"          => Some(Token::In),
        "LIKE"        => Some(Token::Like),
        "BETWEEN"     => Some(Token::Between),
        "ORDER"       => Some(Token::Order),
        "BY"          => Some(Token::By),
        "ASC"         => Some(Token::Asc),
        "DESC"        => Some(Token::Desc),
        "LIMIT"       => Some(Token::Limit),
        "OFFSET"      => Some(Token::Offset),
        "JOIN"        => Some(Token::Join),
        "INNER"       => Some(Token::Inner),
        "LEFT"        => Some(Token::Left),
        "RIGHT"       => Some(Token::Right),
        "OUTER"       => Some(Token::Outer),
        "CROSS"       => Some(Token::Cross),
        "NATURAL"     => Some(Token::Natural),
        "USING"       => Some(Token::Using),
        "GROUP"       => Some(Token::Group),
        "HAVING"      => Some(Token::Having),
        "DISTINCT"    => Some(Token::Distinct),
        "ALL"         => Some(Token::All),
        "AS"          => Some(Token::As),
        "IF"          => Some(Token::If),
        "EXISTS"      => Some(Token::Exists),
        "BEGIN"       => Some(Token::Begin),
        "COMMIT"      => Some(Token::Commit),
        "ROLLBACK"    => Some(Token::Rollback),
        "TRANSACTION" => Some(Token::Transaction),
        "VIRTUAL"     => Some(Token::Virtual),
        "MATCH"       => Some(Token::Match),
        "WITH"        => Some(Token::With),
        "RECURSIVE"   => Some(Token::Recursive),
        "INTEGER"     => Some(Token::KwInteger),
        "INT"         => Some(Token::KwInteger),
        "TEXT"        => Some(Token::KwText),
        "VARCHAR"     => Some(Token::KwText),
        "REAL"        => Some(Token::Real),
        "FLOAT"       => Some(Token::Real),
        "BLOB"        => Some(Token::Blob),
        "BOOLEAN"     => Some(Token::Boolean),
        "BOOL"        => Some(Token::Boolean),
        "TRUE"        => Some(Token::True),
        "FALSE"       => Some(Token::False),
        "PRAGMA"      => Some(Token::Pragma),
        "EXPLAIN"     => Some(Token::Explain),
        "ALTER"       => Some(Token::Alter),
        "RENAME"      => Some(Token::Rename),
        "TO"          => Some(Token::To),
        "ADD"         => Some(Token::Add),
        "COLUMN"      => Some(Token::Column),
        "DO"          => Some(Token::Do),
        "VIEW"        => Some(Token::View),
        "REINDEX"     => Some(Token::Reindex),
        "ANALYZE"     => Some(Token::Analyze),
        "TEMP"        => Some(Token::Temp),
        "TEMPORARY"   => Some(Token::Temp),
        "CONFLICT"    => Some(Token::Conflict),
        "NOTHING"     => Some(Token::Nothing),
        "UNION"       => Some(Token::Union),
        "CHECK"       => Some(Token::Check),
        "CAST"        => Some(Token::Cast),
        "DEFAULT"     => Some(Token::Default),
        "GLOB"        => Some(Token::GLOB),
        "TRIGGER"     => Some(Token::Trigger),
        "BEFORE"      => Some(Token::Before),
        "AFTER"       => Some(Token::After),
        "INSTEAD"      => Some(Token::Instead),
        "OF"          => Some(Token::Of),
        "FOR"         => Some(Token::For),
        "EACH"        => Some(Token::Each),
        "ROW"         => Some(Token::Row),
        "WHEN"        => Some(Token::When),
        "END"         => Some(Token::End),
        "AUTOINCREMENT" => Some(Token::AutoIncrement),
        "ATTACH"       => Some(Token::Attach),
        "DETACH"       => Some(Token::Detach),
        "DATABASE"     => Some(Token::Database),
        "VACUUM"       => Some(Token::Vacuum),
        "BACKUP"       => Some(Token::Backup),
        _             => None,
    }
}

// ── Lexer 詞法分析器 ────────────────────────────────────────────────────

/// 詞法分析器：將 SQL 字串轉換為 Token 串
///
/// ## 使用方式
///
/// ```
/// use db6::sql::parser::lexer::Lexer;
/// let mut lexer = Lexer::new("SELECT * FROM users");
/// let tokens = lexer.tokenize().unwrap();
/// ```
pub struct Lexer {
    /// 輸入字元陣列（支援中文等多字節字元）
    input: Vec<char>,
    /// 目前讀取位置
    pos:   usize,
}

impl Lexer {
    /// 建立一個新的詞法分析器
    pub fn new(input: &str) -> Self {
        Lexer { input: input.chars().collect(), pos: 0 }
    }

    /// 掃描全部 token，遇到錯誤回傳 Err
    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let done = tok == Token::Eof;
            tokens.push(tok);
            if done { break; }
        }
        Ok(tokens)
    }

    /// 預覽目前字元（不消耗位置）
    fn peek(&self) -> Option<char> { self.input.get(self.pos).copied() }

    /// 預覽下一個字元（不消耗位置）
    fn peek2(&self) -> Option<char> { self.input.get(self.pos + 1).copied() }

    /// 讀取目前字元並移動到下一位置
    fn advance(&mut self) -> Option<char> {
        let c = self.input.get(self.pos).copied();
        if c.is_some() { self.pos += 1; }
        c
    }

    /// 讀取下一個 Token（核心方法）
    ///
    /// 先跳過空白與單行註解（-- 開頭到換行），
    /// 然後根據第一個字元的類型決定如何處理。
    fn next_token(&mut self) -> Result<Token, String> {
        // 跳過空白與單行註解
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => { self.advance(); }
                Some('-') if self.peek2() == Some('-') => {
                    while self.peek().map(|c| c != '\n').unwrap_or(false) { self.advance(); }
                }
                _ => break,
            }
        }

        // 根據字元類型分派處理
        match self.peek() {
            None => Ok(Token::Eof),
            Some(c) => match c {
                // 標點符號
                '(' => { self.advance(); Ok(Token::LParen) }
                ')' => { self.advance(); Ok(Token::RParen) }
                ',' => { self.advance(); Ok(Token::Comma) }
                ';' => { self.advance(); Ok(Token::Semicolon) }
                '.' => { self.advance(); Ok(Token::Dot) }
                // 運算子
                '+' => { self.advance(); Ok(Token::Plus) }
                '-' => { self.advance(); Ok(Token::Minus) }
                '*' => { self.advance(); Ok(Token::Star) }
                '/' => { self.advance(); Ok(Token::Slash) }
                '%' => { self.advance(); Ok(Token::Percent) }
                '=' => { self.advance(); Ok(Token::Eq) }
                '@' => { self.advance(); Ok(Token::At) }
                // 兩字元運算子
                '<' => {
                    self.advance();
                    match self.peek() {
                        Some('=') => { self.advance(); Ok(Token::LtEq) }
                        Some('>') => { self.advance(); Ok(Token::NotEq) }
                        _ => Ok(Token::Lt),
                    }
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); Ok(Token::GtEq) }
                    else { Ok(Token::Gt) }
                }
                '!' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); Ok(Token::NotEq) }
                    else { Err(format!("unexpected character '!'")) }
                }
                '|' => {
                    self.advance();
                    if self.peek() == Some('|') { self.advance(); Ok(Token::Concat) }
                    else { Err("expected '||'".to_string()) }
                }
                // 字串字面值（單引號）
                '\'' => self.lex_string(),
                // 反引號或雙引號識別符
                '`' | '"' => self.lex_quoted_ident(),
                // 數字
                c if c.is_ascii_digit() => self.lex_number(),
                // 識別符 / 關鍵字
                c if c.is_alphabetic() || c == '_' => self.lex_ident(),
                c => Err(format!("unexpected character '{}'", c)),
            }
        }
    }

    /// 讀取單引號包圍的字串字面值
    ///
    /// SQL 中兩個連續單引號 `''` 表示跳脫的單引號字元。
    fn lex_string(&mut self) -> Result<Token, String> {
        self.advance(); // 跳過開頭的單引號
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err("unterminated string".to_string()),
                Some('\'') => {
                    // 連續 '' 表示一個跳脫的單引號
                    if self.peek() == Some('\'') { self.advance(); s.push('\''); }
                    else { break; }
                }
                Some(c) => s.push(c),
            }
        }
        Ok(Token::LitStr(s))
    }

    /// 讀取反引號或雙引號包圍的識別符
    ///
    /// 例如 `` `my table` `` 或 `"column name"`。
    fn lex_quoted_ident(&mut self) -> Result<Token, String> {
        let close = if self.peek() == Some('`') { '`' } else { '"' };
        self.advance(); // 跳過開頭引號
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err("unterminated quoted identifier".to_string()),
                Some(c) if c == close => break,
                Some(c) => s.push(c),
            }
        }
        Ok(Token::Ident(s))
    }

    /// 讀取數字字面值（整數或浮點數）
    ///
    /// 如果數字後有 `.` 且 `.` 後還有數字，則為浮點數。
    fn lex_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            s.push(self.advance().unwrap());
        }
        // 檢查是否為浮點數格式（小數點後接數字）
        if self.peek() == Some('.') && self.peek2().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            s.push(self.advance().unwrap()); // 小數點
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                s.push(self.advance().unwrap());
            }
            return s.parse::<f64>()
                .map(Token::LitFloat)
                .map_err(|_| format!("invalid float: {}", s));
        }
        s.parse::<i64>()
            .map(Token::LitInt)
            .map_err(|_| format!("invalid integer: {}", s))
    }

    /// 讀取識別符或關鍵字
    ///
    /// 以字母或底線開頭，後續可包含字母、數字、底線。
    /// 讀取完整字串後查詢關鍵字對照表，匹配則回傳關鍵字 Token。
    fn lex_ident(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while self.peek().map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false) {
            s.push(self.advance().unwrap());
        }
        Ok(keyword(&s).unwrap_or(Token::Ident(s)))
    }
}

impl Token {
    /// 判斷 Token 是否為識別符
    pub fn is_ident(&self) -> bool {
        matches!(self, Token::Ident(_))
    }
}

// ── 測試 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(sql: &str) -> Vec<Token> {
        Lexer::new(sql).tokenize().unwrap()
    }

    #[test]
    fn basic_select() {
        let toks = lex("SELECT * FROM users;");
        assert_eq!(toks[0], Token::Select);
        assert_eq!(toks[1], Token::Star);
        assert_eq!(toks[2], Token::From);
        assert_eq!(toks[3], Token::Ident("users".into()));
        assert_eq!(toks[4], Token::Semicolon);
    }

    #[test]
    fn string_literal() {
        let toks = lex("'hello world'");
        assert_eq!(toks[0], Token::LitStr("hello world".into()));
    }

    #[test]
    fn escaped_quote() {
        let toks = lex("'it''s'");
        assert_eq!(toks[0], Token::LitStr("it's".into()));
    }

    #[test]
    fn numbers() {
        let toks = lex("42 3.14");
        assert_eq!(toks[0], Token::LitInt(42));
        assert_eq!(toks[1], Token::LitFloat(3.14));
    }

    #[test]
    fn operators() {
        let toks = lex("<= >= != <>");
        assert_eq!(toks[0], Token::LtEq);
        assert_eq!(toks[1], Token::GtEq);
        assert_eq!(toks[2], Token::NotEq);
        assert_eq!(toks[3], Token::NotEq);
    }

    #[test]
    fn keywords_case_insensitive() {
        let toks = lex("select FROM Where");
        assert_eq!(toks[0], Token::Select);
        assert_eq!(toks[1], Token::From);
        assert_eq!(toks[2], Token::Where);
    }

    #[test]
    fn line_comment() {
        let toks = lex("SELECT -- this is a comment\n* FROM t");
        assert_eq!(toks[0], Token::Select);
        assert_eq!(toks[1], Token::Star);
    }

    #[test]
    fn quoted_ident() {
        let toks = lex("`my table`");
        assert_eq!(toks[0], Token::Ident("my table".into()));
    }
}
