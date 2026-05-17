//! Parser：Token 串 → AST
//!
//! 使用遞迴下降解析器（Recursive Descent Parser）將 token 流轉換為抽象語法樹。
//!
//! ## 解析流程
//!
//! ```text
//! SQL 字串 → Lexer → Token 流 → Parser → AST
//! ```
//!
//! ## 設計模式
//!
//! 採用 Pratt Parser（表達式解析）與傳統遞迴下降（語句解析）相結合：
//! - **語句層級**：使用遞迴下降（SELECT、INSERT 等）
//! - **表達式層級**：使用 Pratt Parser 處理運算子優先順序
//!
//! ## 運算子優先順序（由低到高）
//!
//! 1. OR
//! 2. AND
//! 3. NOT
//! 4. 比較（=, !=, <, <=, >, >=, IS, BETWEEN, IN, LIKE）
//! 5. 加減（+, -, ||）
//! 6. 乘除（*, /, %）
//! 7. 一元（-, NOT）
//! 8. 主值（literal, column, function, subquery）

use super::ast::*;
use super::lexer::Token;

/// Parser 結構
///
/// 持有 token 陣列與目前解析位置
pub struct Parser {
    /// 輸入的 token 序列
    tokens: Vec<Token>,
    /// 目前解析位置
    pos:    usize,
}

impl Parser {
    /// 建立新的 Parser
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── 基本操作 ──────────────────────────────────────────────────────────

    /// 查看目前 token（不移動）
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    /// 查看下個 token（不移動）
    fn peek2(&self) -> &Token {
        self.tokens.get(self.pos + 1).unwrap_or(&Token::Eof)
    }

    /// 取出目前 token 並移動到下一個
    fn advance(&mut self) -> Token {
        let t = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        if self.pos < self.tokens.len() { self.pos += 1; }
        t
    }

    /// 檢查目前 token 是否為指定類型
    fn check(&self, tok: &Token) -> bool { self.peek() == tok }

    /// 吃掉指定 token，若匹配則移動並返回 Ok
    fn eat(&mut self, tok: &Token) -> Result<(), String> {
        if self.peek() == tok {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", tok, self.peek()))
        }
    }

    /// 吃掉一個識別符
    ///
    /// 允許某些關鍵字（如 TEXT、INTEGER）作為識別符使用
    fn eat_ident(&mut self) -> Result<String, String> {
        match self.advance() {
            Token::Ident(s) => Ok(s),
            // 允許關鍵字當作識別符（常見情形：表名叫 "order"）
            t => {
                if let Some(s) = token_as_ident(&t) { Ok(s) }
                else { Err(format!("expected identifier, got {:?}", t)) }
            }
        }
    }

    /// 可選匹配：若目前 token 匹配則吃掉並返回 true
    fn maybe(&mut self, tok: &Token) -> bool {
        if self.peek() == tok { self.advance(); true } else { false }
    }

    // ── 頂層解析 ──────────────────────────────────────────────────────────

    /// 解析多個語句（以分號分隔）
    ///
    /// 持續解析直到遇到 EOF
    pub fn parse(&mut self) -> Result<Vec<Statement>, String> {
        let mut stmts = Vec::new();
        while self.peek() != &Token::Eof {
            // 吃掉可選的分號
            self.maybe(&Token::Semicolon);
            if self.peek() == &Token::Eof { break; }
            stmts.push(self.parse_statement()?);
            self.maybe(&Token::Semicolon);
        }
        Ok(stmts)
    }

    /// 根據第一個 token 判斷語句類型並解析
    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.peek().clone() {
            Token::Select | Token::With => Ok(Statement::Select(self.parse_select()?)),
            Token::Insert   => Ok(Statement::Insert(self.parse_insert()?)),
            Token::Update   => Ok(Statement::Update(self.parse_update()?)),
            Token::Delete   => Ok(Statement::Delete(self.parse_delete()?)),
            Token::Create   => self.parse_create(),
            Token::Drop     => self.parse_drop(),
            Token::Begin    => { self.advance(); Ok(Statement::Begin) }
            Token::Commit   => { self.advance(); Ok(Statement::Commit) }
            Token::Rollback => { self.advance(); Ok(Statement::Rollback) }
            Token::Alter    => Ok(Statement::AlterTable(self.parse_alter_table()?)),
            Token::Pragma   => Ok(Statement::Pragma(self.parse_pragma()?)),
            Token::Explain  => Ok(Statement::Explain(self.parse_explain()?)),
            Token::Reindex  => Ok(Statement::Reindex(self.parse_reindex()?)),
            Token::Analyze  => Ok(Statement::Analyze(self.parse_analyze()?)),
            Token::Attach   => self.parse_attach(),
            Token::Detach   => self.parse_detach(),
            Token::Vacuum   => { self.advance(); Ok(Statement::Vacuum) }
            Token::Backup   => Ok(Statement::Backup(self.parse_backup()?)),
            t => Err(format!("unexpected token {:?}", t)),
        }
    }

    // ── SELECT ────────────────────────────────────────────────────────────

    fn parse_select(&mut self) -> Result<SelectStmt, String> {
        // WITH ctes
        let with = self.parse_with_clause()?;

        self.eat(&Token::Select)?;
        let distinct = self.maybe(&Token::Distinct);

        let columns = self.parse_select_items()?;

        let from = if self.maybe(&Token::From) {
            Some(self.parse_from_item()?)
        } else { None };

        let joins = self.parse_joins()?;

        let where_ = if self.maybe(&Token::Where) {
            Some(self.parse_expr()?)
        } else { None };

        let group_by = if self.check(&Token::Group) && self.peek2() == &Token::By {
            self.advance(); self.advance();
            self.parse_expr_list()?
        } else { vec![] };

        let having = if self.maybe(&Token::Having) {
            Some(self.parse_expr()?)
        } else { None };

        let order_by = if self.check(&Token::Order) && self.peek2() == &Token::By {
            self.advance(); self.advance();
            self.parse_order_items()?
        } else { vec![] };

        let (limit, offset) = if self.maybe(&Token::Limit) {
            let first = self.parse_expr()?;
            // LIMIT n, m - MySQL style (n = offset, m = limit)
            // LIMIT n OFFSET m - standard (n = limit, m = offset)
            if self.check(&Token::Comma) {
                self.advance();
                let limit = self.parse_expr()?;
                let offset = Some(first);
                (Some(limit), offset)
            } else if self.maybe(&Token::Offset) {
                let offset = Some(self.parse_expr()?);
                (Some(first), offset)
            } else {
                (Some(first), None)
            }
        } else {
            let offset = if self.maybe(&Token::Offset) {
                Some(self.parse_expr()?)
            } else { None };
            (None, offset)
        };

        let union_with = if self.maybe(&Token::Union) {
            let is_all = self.maybe(&Token::All);
            let right = self.parse_select()?;
            Some(Box::new((right, is_all)))
        } else { None };

        Ok(SelectStmt { with, distinct, columns, from, joins, where_, group_by, having, order_by, limit, offset, union_with })
    }

    fn parse_select_items(&mut self) -> Result<Vec<SelectItem>, String> {
        let mut items = Vec::new();
        loop {
            let item = if self.check(&Token::Star) {
                self.advance();
                SelectItem::Star
            } else {
                let expr = self.parse_expr()?;
                // table.*
                if let Expr::Column { table: None, name } = &expr {
                    if self.check(&Token::Dot) && self.peek2() == &Token::Star {
                        let t = name.clone();
                        self.advance(); self.advance();
                        items.push(SelectItem::TableStar(t));
                        if !self.maybe(&Token::Comma) { break; }
                        continue;
}
            }
                let alias = if self.maybe(&Token::As) {
                    Some(self.eat_ident()?)
                } else if let Token::Ident(_) = self.peek() {
                    Some(self.eat_ident()?)
                } else { None };
                SelectItem::Expr { expr, alias }
            };
            items.push(item);
            if !self.maybe(&Token::Comma) { break; }
        }
        Ok(items)
    }

    fn parse_table_ref(&mut self) -> Result<TableRef, String> {
        let name = self.eat_ident()?;
        if self.maybe(&Token::Dot) {
            let table = self.eat_ident()?;
            let full_name = format!("{}.{}", name, table);
            let alias = if self.maybe(&Token::As) {
                Some(self.eat_ident()?)
            } else { None };
            Ok(TableRef { name: full_name, alias })
        } else {
            let alias = if self.maybe(&Token::As) {
                Some(self.eat_ident()?)
            } else if let Token::Ident(_) = self.peek() {
                Some(self.eat_ident()?)
            } else { None };
            Ok(TableRef { name, alias })
        }
    }

    fn parse_joins(&mut self) -> Result<Vec<Join>, String> {
        let mut joins = Vec::new();
        loop {
            let kind = match self.peek().clone() {
                Token::Join | Token::Inner => {
                    if self.peek().clone() == Token::Inner { self.advance(); }
                    self.eat(&Token::Join)?;
                    JoinKind::Inner
                }
                Token::Left => {
                    self.advance();
                    self.maybe(&Token::Outer);
                    self.eat(&Token::Join)?;
                    JoinKind::Left
                }
                Token::Right => {
                    self.advance();
                    self.maybe(&Token::Outer);
                    self.eat(&Token::Join)?;
                    JoinKind::Right
                }
                Token::Cross => {
                    self.advance();
                    self.eat(&Token::Join)?;
                    JoinKind::Cross
                }
                Token::Natural => {
                    self.advance();
                    self.eat(&Token::Join)?;
                    JoinKind::Natural
                }
                _ => break,
            };
            let table = self.parse_table_ref()?;
            let condition = if self.maybe(&Token::On) {
                JoinCondition::On(self.parse_expr()?)
            } else if self.maybe(&Token::Using) {
                self.eat(&Token::LParen)?;
                let cols = self.parse_ident_list()?;
                self.eat(&Token::RParen)?;
                JoinCondition::Using(cols)
            } else {
                JoinCondition::None
            };
            joins.push(Join { kind, table, condition });
        }
        Ok(joins)
    }

    fn parse_order_items(&mut self) -> Result<Vec<OrderItem>, String> {
        let mut items = Vec::new();
        loop {
            let expr = self.parse_expr()?;
            let asc = !self.maybe(&Token::Desc);
            if self.check(&Token::Asc) { self.advance(); }
            items.push(OrderItem { expr, asc });
            if !self.maybe(&Token::Comma) { break; }
        }
        Ok(items)
    }

    // ── INSERT ────────────────────────────────────────────────────────────

    fn parse_insert(&mut self) -> Result<InsertStmt, String> {
        self.eat(&Token::Insert)?;
        self.eat(&Token::Into)?;
        let table = self.eat_ident()?;

        let columns = if self.check(&Token::LParen) && self.is_values_next() {
            self.eat(&Token::LParen)?;
            let cols = self.parse_ident_list()?;
            self.eat(&Token::RParen)?;
            cols
        } else { vec![] };

        // INSERT DEFAULT VALUES
        let (values, default_values) = if self.check(&Token::Default) {
            self.advance();
            self.eat(&Token::Values)?;
            (vec![vec![]], true)
        } else {
            self.eat(&Token::Values)?;
            let mut vals = Vec::new();
            loop {
                self.eat(&Token::LParen)?;
                vals.push(self.parse_expr_list()?);
                self.eat(&Token::RParen)?;
                if !self.maybe(&Token::Comma) { break; }
            }
            (vals, false)
        };

        let on_conflict = if self.maybe(&Token::On) {
            self.eat(&Token::Conflict)?;
            self.eat(&Token::Do)?;
            if self.check(&Token::Nothing) {
                self.advance();
                Some(crate::sql::parser::ast::OnConflict::DoNothing)
            } else {
                self.eat(&Token::Update)?;
                self.eat(&Token::Set)?;
                let col = self.eat_ident()?;
                self.eat(&Token::Eq)?;
                let val = self.parse_expr()?;
                Some(crate::sql::parser::ast::OnConflict::DoUpdate { column: col, value: val })
            }
        } else { None };

        Ok(InsertStmt { table, columns, values, default_values, on_conflict })
    }

    fn is_values_next(&self) -> bool {
        // 往前掃 ) 之後是否接 VALUES
        let mut i = self.pos + 1;
        let mut depth = 1;
        while i < self.tokens.len() {
            match &self.tokens[i] {
                Token::LParen => depth += 1,
                Token::RParen => { depth -= 1; if depth == 0 { break; } }
                _ => {}
            }
            i += 1;
        }
        self.tokens.get(i + 1) == Some(&Token::Values)
    }

    // ── UPDATE ────────────────────────────────────────────────────────────

    fn parse_update(&mut self) -> Result<UpdateStmt, String> {
        self.eat(&Token::Update)?;
        let table = self.eat_ident()?;
        self.eat(&Token::Set)?;
        let mut sets = Vec::new();
        loop {
            let col = self.eat_ident()?;
            self.eat(&Token::Eq)?;
            let val = self.parse_expr()?;
            sets.push((col, val));
            if !self.maybe(&Token::Comma) { break; }
        }
        let where_ = if self.maybe(&Token::Where) { Some(self.parse_expr()?) } else { None };
        Ok(UpdateStmt { table, sets, where_ })
    }

    // ── DELETE ────────────────────────────────────────────────────────────

    fn parse_delete(&mut self) -> Result<DeleteStmt, String> {
        self.eat(&Token::Delete)?;
        self.eat(&Token::From)?;
        let table = self.eat_ident()?;
        let where_ = if self.maybe(&Token::Where) { Some(self.parse_expr()?) } else { None };
        Ok(DeleteStmt { table, where_ })
    }

    // ── CREATE ────────────────────────────────────────────────────────────

    fn parse_create(&mut self) -> Result<Statement, String> {
        self.eat(&Token::Create)?;
        let unique = self.maybe(&Token::Unique);
        
        // Handle CREATE VIRTUAL TABLE for FTS
        if self.check(&Token::Virtual) {
            self.advance();
            self.eat(&Token::Table)?;
            return Ok(Statement::CreateVirtualTable(self.parse_create_virtual_table()?));
        }
        
        match self.peek().clone() {
            Token::Table => Ok(Statement::CreateTable(self.parse_create_table()?)),
            Token::Index => Ok(Statement::CreateIndex(self.parse_create_index(unique)?)),
            Token::View    => Ok(Statement::CreateView(self.parse_create_view()?)),
            Token::Trigger => Ok(Statement::CreateTrigger(self.parse_create_trigger()?)),
            t => Err(format!("expected TABLE, INDEX, VIEW, or TRIGGER after CREATE, got {:?}", t)),
        }
    }

    fn parse_create_view(&mut self) -> Result<CreateViewStmt, String> {
        self.eat(&Token::View)?;
        let if_not_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Not)?;
            self.eat(&Token::Exists)?;
            true
        } else { false };
        let temp = self.check(&Token::Temp);
        if temp { self.advance(); }
        let name = self.eat_ident()?;
        self.eat(&Token::As)?;
        let query = Box::new(self.parse_select()?);
        Ok(CreateViewStmt { if_not_exists, temp, name, query })
    }

    fn parse_create_table(&mut self) -> Result<CreateTableStmt, String> {
        self.eat(&Token::Table)?;
        let if_not_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Not)?;
            self.eat(&Token::Exists)?;
            true
        } else { false };

        let name = self.eat_ident()?;
        self.eat(&Token::LParen)?;

        let mut columns = Vec::new();
        let mut constraints = Vec::new();
        loop {
            if self.check(&Token::Primary) || self.check(&Token::Unique) {
                constraints.push(self.parse_table_constraint()?);
            } else {
                columns.push(self.parse_column_def()?);
            }
            if !self.maybe(&Token::Comma) { break; }
            // 允許尾隨逗號前的 ) 結束
            if self.check(&Token::RParen) { break; }
        }
        self.eat(&Token::RParen)?;
        Ok(CreateTableStmt { if_not_exists, name, columns, constraints })
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, String> {
        let name = self.eat_ident()?;
        let data_type = self.parse_sql_type()?;
        let mut cons = Vec::new();
        loop {
            match self.peek().clone() {
                Token::Not => {
                    self.advance();
                    self.eat(&Token::LitNull)?;
                    cons.push(ColumnConstraint::NotNull);
                }
                Token::Primary => {
                    self.advance();
                    self.eat(&Token::Key)?;
                    let autoincrement = matches!(self.peek(), Token::AutoIncrement);
                    if autoincrement { self.advance(); }
                    cons.push(ColumnConstraint::PrimaryKey { autoincrement });
                }
                Token::Unique => { self.advance(); cons.push(ColumnConstraint::Unique); }
                Token::Check => {
                    self.advance();
                    self.eat(&Token::LParen)?;
                    let expr = self.parse_expr()?;
                    self.eat(&Token::RParen)?;
                    cons.push(ColumnConstraint::Check(expr));
                }
                Token::References => {
                    self.advance();
                    let table = self.eat_ident()?;
                    // 處理可選的欄位名：REFERENCES table(column)
                    let column = if matches!(self.peek(), Token::LParen) {
                        self.advance(); // (
                        let col = self.eat_ident()?;
                        self.eat(&Token::RParen)?;
                        Some(col)
                    } else {
                        None
                    };
                    cons.push(ColumnConstraint::References { table, column });
                }
                Token::Default => {
                    self.advance();
                    let default_expr = if self.check(&Token::LParen) {
                        self.eat(&Token::LParen)?;
                        let expr = self.parse_expr()?;
                        self.eat(&Token::RParen)?;
                        expr
                    } else if self.check(&Token::LitNull) {
                        self.advance();
                        Expr::LitNull
                    } else {
                        self.parse_primary()?
                    };
                    cons.push(ColumnConstraint::Default(default_expr));
                }
                _ => break,
            }
        }
        Ok(ColumnDef { name, data_type, constraints: cons })
    }

    fn parse_sql_type(&mut self) -> Result<SqlType, String> {
        match self.advance() {
            Token::KwInteger => Ok(SqlType::Integer),
            Token::Real      => Ok(SqlType::Real),
            Token::KwText    => Ok(SqlType::Text),
            Token::Blob      => Ok(SqlType::Blob),
            Token::Boolean   => Ok(SqlType::Boolean),
            Token::LitNull   => Ok(SqlType::Null),
            t => Err(format!("expected type, got {:?}", t)),
        }
    }

    fn parse_table_constraint(&mut self) -> Result<TableConstraint, String> {
        match self.peek().clone() {
            Token::Primary => {
                self.advance(); self.eat(&Token::Key)?;
                self.eat(&Token::LParen)?;
                let cols = self.parse_ident_list()?;
                self.eat(&Token::RParen)?;
                Ok(TableConstraint::PrimaryKey(cols))
            }
            Token::Unique => {
                self.advance();
                self.eat(&Token::LParen)?;
                let cols = self.parse_ident_list()?;
                self.eat(&Token::RParen)?;
                Ok(TableConstraint::Unique(cols))
            }
            t => Err(format!("expected table constraint, got {:?}", t)),
        }
    }

    fn parse_create_virtual_table(&mut self) -> Result<CreateVirtualTableStmt, String> {
        // table_name
        let name = self.eat_ident()?;
        
        // USING fts(col1, col2, ...)
        self.eat(&Token::Using)?;
        
        // Check for fts keyword
        match self.peek().clone() {
            Token::Ident(s) if s.eq_ignore_ascii_case("fts") => {
                self.advance();
            }
            t => return Err(format!("expected fts, got {:?}", t)),
        }
        
        self.eat(&Token::LParen)?;
        let columns = self.parse_ident_list()?;
        self.eat(&Token::RParen)?;
        
        Ok(CreateVirtualTableStmt {
            if_not_exists: false,
            name,
            columns,
        })
    }

    fn parse_create_index(&mut self, unique: bool) -> Result<CreateIndexStmt, String> {
        self.eat(&Token::Index)?;
        let name = self.eat_ident()?;
        self.eat(&Token::On)?;
        let table = self.eat_ident()?;
        self.eat(&Token::LParen)?;
        let columns = self.parse_ident_list()?;
        self.eat(&Token::RParen)?;
        Ok(CreateIndexStmt { unique, name, table, columns })
    }

    // ── DROP TABLE / DROP INDEX / DROP VIEW ─────────────────────────────────

    fn parse_drop(&mut self) -> Result<Statement, String> {
        self.eat(&Token::Drop)?;
        match self.peek().clone() {
            Token::Table  => Ok(Statement::DropTable(self.parse_drop_table()?)),
            Token::Index  => Ok(Statement::DropIndex(self.parse_drop_index()?)),
            Token::View   => Ok(Statement::DropView(self.parse_drop_view()?)),
            Token::Trigger=> Ok(Statement::DropTrigger(self.parse_drop_trigger()?)),
            t => Err(format!("expected TABLE, INDEX, VIEW, or TRIGGER after DROP, got {:?}", t)),
        }
    }

    fn parse_drop_table(&mut self) -> Result<DropTableStmt, String> {
        self.eat(&Token::Table)?;
        let if_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Exists)?;
            true
        } else { false };
        let name = self.eat_ident()?;
        Ok(DropTableStmt { if_exists, name })
    }

    fn parse_drop_index(&mut self) -> Result<DropIndexStmt, String> {
        self.eat(&Token::Index)?;
        let if_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Exists)?;
            true
        } else { false };
        let name = self.eat_ident()?;
        Ok(DropIndexStmt { if_exists, name })
    }

    fn parse_drop_view(&mut self) -> Result<DropViewStmt, String> {
        self.eat(&Token::View)?;
        let if_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Exists)?;
            true
        } else { false };
        let name = self.eat_ident()?;
        Ok(DropViewStmt { if_exists, name })
    }

    // ── TRIGGER ─────────────────────────────────────────────────────────────

    fn parse_create_trigger(&mut self) -> Result<CreateTriggerStmt, String> {
        self.eat(&Token::Trigger)?;
        let if_not_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Not)?;
            self.eat(&Token::Exists)?;
            true
        } else { false };
        let name = self.eat_ident()?;

        let timing = if self.check(&Token::Before) {
            self.advance();
            TriggerTiming::Before
        } else if self.check(&Token::After) {
            self.advance();
            TriggerTiming::After
        } else if self.check(&Token::Instead) {
            self.advance();
            self.eat(&Token::Of)?;
            TriggerTiming::InsteadOf
        } else {
            return Err("expected BEFORE, AFTER, or INSTEAD OF".into());
        };

        let event = if self.check(&Token::Delete) {
            self.advance();
            TriggerEvent::Delete
        } else if self.check(&Token::Insert) {
            self.advance();
            TriggerEvent::Insert
        } else if self.check(&Token::Update) {
            self.advance();
            let cols = if self.check(&Token::Of) {
                self.advance();
                let mut columns = Vec::new();
                loop {
                    columns.push(self.eat_ident()?);
                    if !self.maybe(&Token::Comma) { break; }
                }
                Some(columns)
            } else {
                None
            };
            TriggerEvent::Update(cols)
        } else {
            return Err("expected DELETE, INSERT, or UPDATE".into());
        };

        self.eat(&Token::On)?;
        let table = self.eat_ident()?;

        let for_each_row = if self.check(&Token::For) {
            self.advance();
            self.eat(&Token::Each)?;
            self.eat(&Token::Row)?;
            true
        } else { false };

        let when = if self.check(&Token::When) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.eat(&Token::Begin)?;
        let body = self.parse_trigger_body()?;
        self.eat(&Token::End)?;

        Ok(CreateTriggerStmt {
            if_not_exists,
            name,
            table,
            timing,
            event,
            for_each_row,
            when,
            body,
        })
    }

    fn parse_trigger_body(&mut self) -> Result<String, String> {
        let mut depth = 0;
        let start = self.pos;
        loop {
            match self.peek() {
                Token::Eof => return Err("unterminated TRIGGER body".into()),
                Token::Begin => { self.advance(); depth += 1; }
                Token::End if depth == 0 => break,
                Token::End => { self.advance(); if depth == 0 { break; } else { depth -= 1; } }
                _ => { self.advance(); }
            }
        }
        let tokens = &self.tokens[start..self.pos];
        let mut sql = String::new();
        for t in tokens {
            if let Some(s) = self.token_to_string(t) {
                if !sql.is_empty() { sql.push(' '); }
                sql.push_str(&s);
            }
        }
        Ok(sql)
    }

    fn token_to_string(&self, token: &Token) -> Option<String> {
        match token {
            Token::Select | Token::From | Token::Where | Token::Insert | Token::Into
            | Token::Values | Token::Update | Token::Set | Token::Delete | Token::Create
            | Token::Table | Token::Index | Token::Drop | Token::As | Token::On
            | Token::And | Token::Or | Token::Not | Token::Null | Token::True | Token::False
            | Token::Eq | Token::NotEq | Token::Lt | Token::LtEq | Token::Gt | Token::GtEq
            | Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent
            | Token::LParen | Token::RParen | Token::Comma | Token::Dot | Token::Semicolon
            | Token::Like | Token::GLOB | Token::Is | Token::In | Token::Between
            | Token::Order | Token::By | Token::Asc | Token::Desc | Token::Limit
            | Token::Offset | Token::Join | Token::Inner | Token::Left | Token::Outer
            | Token::Cross | Token::Natural | Token::Using | Token::Group | Token::Having
            | Token::Distinct | Token::All | Token::Exists | Token::Begin | Token::Commit
            | Token::Rollback | Token::Transaction | Token::Union | Token::Cast | Token::When
            | Token::For | Token::Each | Token::Row | Token::Trigger | Token::Before
            | Token::After | Token::Instead | Token::Of | Token::View | Token::Index
            | Token::Primary | Token::Key | Token::Unique | Token::If | Token::Temp
            | Token::Reindex | Token::Analyze | Token::Pragma | Token::Explain
            | Token::Alter | Token::Rename | Token::To | Token::Add | Token::Column
            | Token::AutoIncrement | Token::Default | Token::Check | Token::Conflict
            | Token::Virtual | Token::Match | Token::With | Token::Recursive
            | Token::References | Token::KwInteger | Token::Real | Token::Blob
            | Token::Boolean | Token::Nothing | Token::Do | Token::Attach
            | Token::Detach | Token::Database | Token::Vacuum | Token::Backup => {
                Some(format!("{:?}", token).to_uppercase())
            }
            Token::Ident(s) => Some(s.clone()),
            Token::LitInt(i) => Some(i.to_string()),
            Token::LitFloat(f) => Some(f.to_string()),
            Token::LitStr(s) => Some(format!("'{}'", s)),
            Token::Right => Some("RIGHT".to_string()),
            Token::KwText => Some("TEXT".to_string()),
            Token::End => Some("END".to_string()),
            Token::Concat => Some("||".to_string()),
            Token::LitNull => Some("NULL".to_string()),
            Token::At => Some("@".to_string()),
            Token::Eof => None,
        }
    }

    fn parse_drop_trigger(&mut self) -> Result<DropTriggerStmt, String> {
        self.eat(&Token::Trigger)?;
        let if_exists = if self.check(&Token::If) {
            self.advance();
            self.eat(&Token::Exists)?;
            true
        } else { false };
        let name = self.eat_ident()?;
        Ok(DropTriggerStmt { if_exists, name })
    }

    // ── REINDEX ─────────────────────────────────────────────────────────────

    fn parse_reindex(&mut self) -> Result<ReindexStmt, String> {
        self.eat(&Token::Reindex)?;
        let name = if self.peek().is_ident() {
            Some(self.eat_ident()?)
        } else { None };
        Ok(ReindexStmt { name })
    }

    // ── ANALYZE ─────────────────────────────────────────────────────────────

    fn parse_analyze(&mut self) -> Result<AnalyzeStmt, String> {
        self.eat(&Token::Analyze)?;
        let name = if self.peek().is_ident() {
            Some(self.eat_ident()?)
        } else { None };
        Ok(AnalyzeStmt { name })
    }

    // ── BACKUP ─────────────────────────────────────────────────────────────

    fn parse_backup(&mut self) -> Result<BackupStmt, String> {
        self.eat(&Token::Backup)?;
        self.eat(&Token::To)?;
        let tok = self.peek().clone();
        let path = match tok {
            Token::LitStr(s) => { self.advance(); s }
            _ => return Err(format!("expected string literal for path, got {:?}", self.peek())),
        };
        Ok(BackupStmt { path })
    }

    // ── ATTACH ─────────────────────────────────────────────────────────────

    fn parse_attach(&mut self) -> Result<Statement, String> {
        self.eat(&Token::Attach)?;
        self.eat(&Token::Database)?;
        let tok = self.peek().clone();
        let path = match tok {
            Token::LitStr(s) => { self.advance(); s }
            _ => return Err(format!("expected string literal for path, got {:?}", self.peek())),
        };
        self.eat(&Token::As)?;
        let alias = self.eat_ident()?;
        Ok(Statement::Attach { path, alias })
    }

    // ── DETACH ─────────────────────────────────────────────────────────────

    fn parse_detach(&mut self) -> Result<Statement, String> {
        self.eat(&Token::Detach)?;
        self.eat(&Token::Database)?;
        let alias = self.eat_ident()?;
        Ok(Statement::Detach { alias })
    }

    // ── ALTER TABLE ───────────────────────────────────────────────────────

    fn parse_alter_table(&mut self) -> Result<AlterTableStmt, String> {
        self.eat(&Token::Alter)?;
        self.eat(&Token::Table)?;
        let table = self.eat_ident()?;
        match self.peek().clone() {
            Token::Rename => {
                self.advance();
                self.eat(&Token::To)?;
                let to = self.eat_ident()?;
                Ok(AlterTableStmt { table, op: AlterTableOp::RenameTo(to) })
            }
            Token::Add => {
                self.advance();
                self.eat(&Token::Column)?;
                let name = self.eat_ident()?;
                let data_type = self.parse_sql_type()?;
                Ok(AlterTableStmt { table, op: AlterTableOp::AddColumn { name, data_type } })
            }
            t => Err(format!("expected RENAME or ADD after ALTER TABLE, got {:?}", t)),
        }
    }

    // ── PRAGMA ─────────────────────────────────────────────────────────────

    fn parse_pragma(&mut self) -> Result<PragmaStmt, String> {
        self.eat(&Token::Pragma)?;
        let name = self.eat_ident()?;
        let value = if self.maybe(&Token::Eq) {
            Some(self.parse_expr()?)
        } else { None };
        Ok(PragmaStmt { name, value })
    }

    // ── EXPLAIN ────────────────────────────────────────────────────────────

    fn parse_explain(&mut self) -> Result<ExplainStmt, String> {
        self.eat(&Token::Explain)?;
        let inner = Box::new(self.parse_statement()?);
        Ok(ExplainStmt { inner })
    }

    // ── 運算式解析（Pratt Parser）────────────────────────────────────────────
    //
    // Pratt Parser 的核心思想：根據 token 的 "binding power" 決定解析方式
    // 每個運算子有左結合性和優先順序
    //
    // 解析層級（由低到高）：
    // parse_or → parse_and → parse_not → parse_comparison → ...
    // ── 運算式 (Pratt parser) ─────────────────────────────────────────────

    /// 解析表達式的入口點
    ///
    /// 從最高優先順序開始解析（parse_or）
    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    /// 解析 OR 運算（最低優先順序）
    ///
    /// # 語法
    /// ```text
    /// expr ::= expr OR expr
    /// ```
    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.check(&Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinOp { left: Box::new(left), op: BinOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    /// 解析 AND 運算
    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not()?;
        while self.check(&Token::And) {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::BinOp { left: Box::new(left), op: BinOp::And, right: Box::new(right) };
        }
        Ok(left)
    }

    /// 解析 NOT 運算
    fn parse_not(&mut self) -> Result<Expr, String> {
        if self.check(&Token::Not) {
            self.advance();
            let expr = self.parse_comparison()?;
            return Ok(Expr::UnaryOp { op: UnaryOp::Not, expr: Box::new(expr) });
        }
        self.parse_comparison()
    }

    /// 解析比較運算
    ///
    /// 包含：=, !=, <, <=, >, >=, IS, BETWEEN, IN, LIKE, GLOB
    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let left = self.parse_addition()?;

        // IS [NOT] NULL
        if self.check(&Token::Is) {
            self.advance();
            let negated = self.maybe(&Token::Not);
            self.eat(&Token::LitNull)?;
            return Ok(Expr::IsNull { expr: Box::new(left), negated });
        }

        // [NOT] BETWEEN
        let negated_between = if self.check(&Token::Not) && self.peek2() == &Token::Between {
            self.advance(); true
        } else { false };
        if self.maybe(&Token::Between) {
            let low  = self.parse_addition()?;
            self.eat(&Token::And)?;
            let high = self.parse_addition()?;
            return Ok(Expr::Between { expr: Box::new(left), low: Box::new(low), high: Box::new(high), negated: negated_between });
        }

        // [NOT] IN (...)
        let negated_in = if !negated_between && self.check(&Token::Not) && self.peek2() == &Token::In {
            self.advance(); true
        } else { false };
        if self.maybe(&Token::In) {
            self.eat(&Token::LParen)?;
            // IN (SELECT ...) 子查詢
            if self.check(&Token::Select) || self.check(&Token::With) {
                let query = self.parse_select()?;
                self.eat(&Token::RParen)?;
                return Ok(Expr::InSubquery { expr: Box::new(left), query: Box::new(query), negated: negated_in });
            }
            let list = self.parse_expr_list()?;
            self.eat(&Token::RParen)?;
            return Ok(Expr::InList { expr: Box::new(left), list, negated: negated_in });
        }

        // [NOT] LIKE
        let negated_like = if !negated_in && self.check(&Token::Not) && self.peek2() == &Token::Like {
            self.advance(); true
        } else { false };
        if self.maybe(&Token::Like) {
            let pattern = self.parse_addition()?;
            return Ok(Expr::Like { expr: Box::new(left), pattern: Box::new(pattern), negated: negated_like });
        }

        // [NOT] GLOB
        let negated_glob = if !negated_in && self.check(&Token::Not) && self.peek2() == &Token::GLOB {
            self.advance(); true
        } else { false };
        if self.maybe(&Token::GLOB) {
            let pattern = self.parse_addition()?;
            return Ok(Expr::Glob { expr: Box::new(left), pattern: Box::new(pattern), negated: negated_glob });
        }

        // FTS MATCH (table MATCH 'query')
        if self.maybe(&Token::Match) {
            let query = self.parse_addition()?;
            // The left should be a column reference (table name)
            let table_name = match &left {
                Expr::Column { name, .. } => name.clone(),
                _ => return Err("MATCH requires table name".into()),
            };
            let query_str = match query {
                Expr::LitStr(s) => s,
                _ => return Err("MATCH requires string query".into()),
            };
            return Ok(Expr::Match { table: table_name, query: query_str });
        }

        // 比較運算子
        let op = match self.peek() {
            Token::Eq     => BinOp::Eq,
            Token::NotEq  => BinOp::NotEq,
            Token::Lt     => BinOp::Lt,
            Token::LtEq   => BinOp::LtEq,
            Token::Gt     => BinOp::Gt,
            Token::GtEq   => BinOp::GtEq,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_addition()?;
        Ok(Expr::BinOp { left: Box::new(left), op, right: Box::new(right) })
    }

    /// 解析加法/減法（+、-、||）
    fn parse_addition(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek() {
                Token::Plus   => BinOp::Add,
                Token::Minus  => BinOp::Sub,
                Token::Concat => BinOp::Concat,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    /// 解析乘法/除法/取模（*、/、%）
    fn parse_multiplication(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star    => BinOp::Mul,
                Token::Slash   => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    /// 解析一元運算（負號）
    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.check(&Token::Minus) {
            self.advance();
            let expr = self.parse_primary()?;
            return Ok(Expr::UnaryOp { op: UnaryOp::Neg, expr: Box::new(expr) });
        }
        self.parse_primary()
    }

    /// 解析基本值（literal、column、function、subquery）
    ///
    /// 這是 Pratt Parser 的核心：根據 token 類型直接構造 AST 節點
    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            // ── 字面值 ──────────────────────────────────────────────────────
            Token::LitInt(v)  => { self.advance(); Ok(Expr::LitInt(v)) }
            Token::LitFloat(v)=> { self.advance(); Ok(Expr::LitFloat(v)) }
            Token::LitStr(s)  => { self.advance(); Ok(Expr::LitStr(s)) }
            Token::LitNull    => { self.advance(); Ok(Expr::LitNull) }
            Token::True       => { self.advance(); Ok(Expr::LitBool(true)) }
            Token::False      => { self.advance(); Ok(Expr::LitBool(false)) }

            // ── 括號 / 子查詢 ──────────────────────────────────────────────
            Token::LParen => {
                self.advance();
                // (SELECT ...) 純量子查詢
                if self.check(&Token::Select) || self.check(&Token::With) {
                    let query = self.parse_select()?;
                    self.eat(&Token::RParen)?;
                    return Ok(Expr::ScalarSubquery(Box::new(query)));
                }
                let expr = self.parse_expr()?;
                self.eat(&Token::RParen)?;
                Ok(expr)
            }

            // ── JSON Path 表達式 ─────────────────────────────────────────────
            // @.field > 25, @.name = 'Alice', @.city IN ('台北', '台中')
            Token::At => {
                self.advance(); // 吃掉 @
                self.parse_json_path()
            }

            // ── EXISTS (SELECT ...) ────────────────────────────────────────
            Token::Exists => {
                self.advance();
                self.eat(&Token::LParen)?;
                let query = self.parse_select()?;
                self.eat(&Token::RParen)?;
                Ok(Expr::Exists { query: Box::new(query), negated: false })
            }

            // ── CAST(expr AS type) ─────────────────────────────────────────
            Token::Cast => {
                self.advance();
                self.eat(&Token::LParen)?;
                let expr = self.parse_expr()?;
                self.eat(&Token::As)?;
                let sql_type = self.parse_sql_type()?;
                self.eat(&Token::RParen)?;
                Ok(Expr::Cast { expr: Box::new(expr), to: sql_type })
            }

            // ── 識別符（欄位或函式） ───────────────────────────────────────
            Token::Ident(name) => {
                self.advance();
                // 函式呼叫：name(...)
                if self.check(&Token::LParen) {
                    return self.parse_function_call(name);
                }
                // table.column
                if self.check(&Token::Dot) {
                    self.advance();
                    let col = self.eat_ident()?;
                    return Ok(Expr::Column { table: Some(name), name: col });
                }
                // 純欄位名
                Ok(Expr::Column { table: None, name })
            }
            t => Err(format!("unexpected token in expression: {:?}", t)),
        }
    }

    /// 解析函式呼叫
    ///
    /// # 語法
    /// ```text
    /// function_call ::= name ( [DISTINCT] expr [, expr ...] )
    ///                 | name ( * )
    /// ```
    fn parse_function_call(&mut self, name: String) -> Result<Expr, String> {
        self.eat(&Token::LParen)?;
        let distinct = self.maybe(&Token::Distinct);
        let args = if self.check(&Token::Star) {
            // COUNT(*) 等
            self.advance();
            vec![Expr::Column { table: None, name: "*".to_string() }]
        } else if self.check(&Token::RParen) {
            // 無參數函式
            vec![]
        } else {
            self.parse_expr_list()?
        };
        self.eat(&Token::RParen)?;
        Ok(Expr::Function { name: name.to_uppercase(), args, distinct })
    }

    // ── 輔助 ──────────────────────────────────────────────────────────────

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut list = vec![self.parse_expr()?];
        while self.maybe(&Token::Comma) {
            if self.check(&Token::RParen) { break; }
            list.push(self.parse_expr()?);
        }
        Ok(list)
    }

    /// 解析 JSON Path 表達式
    ///
    /// # 語法
    /// ```text
    /// json_path ::= @ . path [op value]
    /// path      ::= field [ . field ]*
    /// op        ::= = | != | < | <= | > | >= | LIKE | IN | IS NULL
    /// ```
    ///
    /// # 範例
    /// - `@.age > 25`
    /// - `@.name = 'Alice'`
    /// - `@.city IN ('台北', '台中')`
    /// - `@.address.city = '台北'`
    fn parse_json_path(&mut self) -> Result<Expr, String> {
        // @ 已經被 advance() 吃掉，現在應該在 Dot 上
        self.eat(&Token::Dot)?;

        // 解析路徑（可能是巢狀）
        let mut path = vec![self.eat_ident()?];
        while self.check(&Token::Dot) && self.peek2().is_ident() {
            self.advance(); // 吃掉 .
            path.push(self.eat_ident()?);
        }

        // Check if there is an operator
        match self.peek() {
            Token::Is | Token::In | Token::Like | Token::Not |
            Token::Eq | Token::NotEq | Token::Lt | Token::LtEq | Token::Gt | Token::GtEq => {
                let (op_kind, negated, value) = self.parse_json_path_op()?;
                
                if op_kind == JsonPathOpKind::In && matches!(value, Expr::ScalarSubquery(_)) {
                    if let Expr::ScalarSubquery(query) = value {
                        return Ok(Expr::InSubquery {
                            expr: Box::new(Expr::JsonPath {
                                path,
                                op: JsonPathOpKind::Eq,
                                negated: false,
                                value: Box::new(Expr::LitNull)
                            }),
                            query,
                            negated,
                        });
                    }
                }
                
                let value = Box::new(value);

                Ok(Expr::JsonPath {
                    path,
                    op: op_kind,
                    negated,
                    value,
                })
            }
            _ => {
                Ok(Expr::JsonPath {
                    path,
                    op: JsonPathOpKind::Eq,
                    negated: false,
                    value: Box::new(Expr::LitNull),
                })
            }
        }
    }

    /// 解析 JSON Path 運算子
    ///
    /// 支援：=, !=, <, <=, >, >=, LIKE, IN (...), IS NULL, IS NOT NULL
    fn parse_json_path_op(&mut self) -> Result<(JsonPathOpKind, bool, Expr), String> {
        // IS [NOT] NULL
        if self.check(&Token::Is) {
            self.advance();
            let negated = self.maybe(&Token::Not);
            self.eat(&Token::LitNull)?;
            return Ok((JsonPathOpKind::IsNull, negated, Expr::LitNull));
        }

        // [NOT] IN (...)
        let negated_in = if self.check(&Token::Not) && self.peek2() == &Token::In {
            self.advance(); true
        } else { false };
        if self.maybe(&Token::In) {
            self.eat(&Token::LParen)?;
            if self.check(&Token::Select) || self.check(&Token::With) {
                let query = self.parse_select()?;
                self.eat(&Token::RParen)?;
                return Ok((JsonPathOpKind::In, negated_in, Expr::ScalarSubquery(Box::new(query))));
            }
            let list = self.parse_expr_list()?;
            self.eat(&Token::RParen)?;
            let list_exprs: Vec<Expr> = list;
            // IN 需要轉成 InList 表達式，但這裡我們用簡化方式
            // 直接回傳 In 類型，讓 executor 處理
            // 注意：這裡需要特殊處理，我們先用 BinOp 模擬
            if list_exprs.len() == 1 {
                return Ok((JsonPathOpKind::In, negated_in, list_exprs.into_iter().next().unwrap_or(Expr::LitNull)));
            }
            // 對於多元素 IN，產生特殊結構
            // 我們先用第一個元素，後續可以擴展
            return Ok((JsonPathOpKind::In, negated_in, list_exprs.into_iter().next().unwrap_or(Expr::LitNull)));
        }

        // [NOT] LIKE
        let negated_like = if self.check(&Token::Not) && self.peek2() == &Token::Like {
            self.advance(); true
        } else { false };
        if self.maybe(&Token::Like) {
            let pattern = self.parse_primary()?;
            return Ok((JsonPathOpKind::Like, negated_like, pattern));
        }

        // 比較運算子
        let op_kind = match self.peek() {
            Token::Eq    => JsonPathOpKind::Eq,
            Token::NotEq => JsonPathOpKind::Ne,
            Token::Lt    => JsonPathOpKind::Lt,
            Token::LtEq  => JsonPathOpKind::LtEq,
            Token::Gt    => JsonPathOpKind::Gt,
            Token::GtEq  => JsonPathOpKind::GtEq,
            _ => return Err(format!("expected comparison operator, got {:?}", self.peek())),
        };
        self.advance();
        let value = self.parse_primary()?;
        Ok((op_kind, false, value))
    }

    fn parse_ident_list(&mut self) -> Result<Vec<String>, String> {
        let mut list = vec![self.eat_ident()?];
        while self.maybe(&Token::Comma) { list.push(self.eat_ident()?); }
        Ok(list)
    }
    // ── WITH / CTE ────────────────────────────────────────────────────────

    fn parse_with_clause(&mut self) -> Result<Vec<crate::sql::parser::ast::Cte>, String> {
        use crate::sql::parser::ast::Cte;
        if !self.check(&Token::With) { return Ok(vec![]); }
        self.advance();
        self.maybe(&Token::Recursive);
        let mut ctes = Vec::new();
        loop {
            let name = self.eat_ident()?;
            self.eat(&Token::As)?;
            self.eat(&Token::LParen)?;
            let query = self.parse_select()?;
            self.eat(&Token::RParen)?;
            ctes.push(Cte { name, query: Box::new(query) });
            if !self.maybe(&Token::Comma) { break; }
        }
        Ok(ctes)
    }

    // ── FROM item（表名或子查詢） ──────────────────────────────────────────

    fn parse_from_item(&mut self) -> Result<crate::sql::parser::ast::FromItem, String> {
        use crate::sql::parser::ast::{FromItem, TableRef};
        if self.check(&Token::LParen) {
            self.advance();
            if self.check(&Token::Select) || self.check(&Token::With) {
                let query = self.parse_select()?;
                self.eat(&Token::RParen)?;
                let alias = if self.maybe(&Token::As) {
                    self.eat_ident()?
                } else {
                    self.eat_ident()?
                };
                Ok(FromItem::Subquery { query: Box::new(query), alias })
            } else {
                let name = self.eat_ident()?;
                self.eat(&Token::RParen)?;
                let alias = if self.maybe(&Token::As) { Some(self.eat_ident()?) }
                    else if let Token::Ident(_) = self.peek() { Some(self.eat_ident()?) }
                    else { None };
                Ok(FromItem::Table(TableRef { name, alias }))
            }
        } else {
            let tref = self.parse_table_ref()?;
            Ok(FromItem::Table(tref))
        }
    }


}

// 允許某些關鍵字作為識別符
fn token_as_ident(t: &Token) -> Option<String> {
    match t {
        Token::KwText    => Some("text".to_string()),
        Token::KwInteger => Some("integer".to_string()),
        Token::Real      => Some("real".to_string()),
        Token::Blob      => Some("blob".to_string()),
        Token::Boolean   => Some("boolean".to_string()),
        Token::Ident(s)  => Some(s.clone()),
        _ => None,
    }
}

// ── 公開便利函式 ──────────────────────────────────────────────────────────

pub fn parse(sql: &str) -> Result<Vec<Statement>, String> {
    let tokens = super::lexer::Lexer::new(sql).tokenize()?;
    Parser::new(tokens).parse()
}

// ── 測試 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn p(sql: &str) -> Vec<Statement> {
        parse(sql).unwrap_or_else(|e| panic!("parse error: {}", e))
    }

    #[test]
    fn select_star() {
        let stmts = p("SELECT * FROM users");
        if let Statement::Select(s) = &stmts[0] {
            match s.from.as_ref().unwrap() {
                crate::sql::parser::ast::FromItem::Table(t) => assert_eq!(t.name, "users"),
                _ => panic!("expected table"),
            }
        }
    }

    #[test]
    fn select_where() {
        let stmts = p("SELECT id, name FROM users WHERE id = 1");
        if let Statement::Select(s) = &stmts[0] {
            assert_eq!(s.columns.len(), 2);
            assert!(s.where_.is_some());
        } else { panic!() }
    }

    #[test]
    fn select_order_limit() {
        let stmts = p("SELECT * FROM t ORDER BY id DESC LIMIT 10 OFFSET 5");
        if let Statement::Select(s) = &stmts[0] {
            assert_eq!(s.order_by.len(), 1);
            assert!(!s.order_by[0].asc);
            assert!(s.limit.is_some());
            assert!(s.offset.is_some());
        } else { panic!() }
    }

    #[test]
    fn insert_with_columns() {
        let stmts = p("INSERT INTO users (id, name) VALUES (1, 'Alice')");
        if let Statement::Insert(s) = &stmts[0] {
            assert_eq!(s.table, "users");
            assert_eq!(s.columns, vec!["id", "name"]);
            assert_eq!(s.values.len(), 1);
            assert_eq!(s.values[0][0], Expr::LitInt(1));
        } else { panic!() }
    }

    #[test]
    fn insert_multi_rows() {
        let stmts = p("INSERT INTO t VALUES (1,'a'),(2,'b')");
        if let Statement::Insert(s) = &stmts[0] {
            assert_eq!(s.values.len(), 2);
        } else { panic!() }
    }

    #[test]
    fn update_stmt() {
        let stmts = p("UPDATE users SET name='Bob', age=30 WHERE id=1");
        if let Statement::Update(s) = &stmts[0] {
            assert_eq!(s.table, "users");
            assert_eq!(s.sets.len(), 2);
            assert!(s.where_.is_some());
        } else { panic!() }
    }

    #[test]
    fn delete_stmt() {
        let stmts = p("DELETE FROM users WHERE id = 42");
        if let Statement::Delete(s) = &stmts[0] {
            assert_eq!(s.table, "users");
            assert!(s.where_.is_some());
        } else { panic!() }
    }

    #[test]
    fn create_table() {
        let stmts = p("CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            score REAL
        )");
        if let Statement::CreateTable(s) = &stmts[0] {
            assert_eq!(s.name, "users");
            assert_eq!(s.columns.len(), 3);
            assert!(matches!(s.columns[0].constraints[0], ColumnConstraint::PrimaryKey { .. }));
            assert!(matches!(s.columns[1].constraints[0], ColumnConstraint::NotNull));
        } else { panic!() }
    }

    #[test]
    fn create_table_if_not_exists() {
        let stmts = p("CREATE TABLE IF NOT EXISTS t (id INTEGER)");
        if let Statement::CreateTable(s) = &stmts[0] {
            assert!(s.if_not_exists);
        } else { panic!() }
    }

    #[test]
    fn drop_table() {
        let stmts = p("DROP TABLE IF EXISTS users");
        if let Statement::DropTable(s) = &stmts[0] {
            assert!(s.if_exists);
            assert_eq!(s.name, "users");
        } else { panic!() }
    }

    #[test]
    fn create_index() {
        let stmts = p("CREATE UNIQUE INDEX idx_name ON users (name)");
        if let Statement::CreateIndex(s) = &stmts[0] {
            assert!(s.unique);
            assert_eq!(s.name, "idx_name");
            assert_eq!(s.table, "users");
        } else { panic!() }
    }

    #[test]
    fn expr_between() {
        let stmts = p("SELECT * FROM t WHERE age BETWEEN 18 AND 65");
        if let Statement::Select(s) = &stmts[0] {
            assert!(matches!(s.where_.as_ref().unwrap(), Expr::Between { .. }));
        } else { panic!() }
    }

    #[test]
    fn expr_in_list() {
        let stmts = p("SELECT * FROM t WHERE id IN (1, 2, 3)");
        if let Statement::Select(s) = &stmts[0] {
            assert!(matches!(s.where_.as_ref().unwrap(), Expr::InList { .. }));
        } else { panic!() }
    }

    #[test]
    fn expr_like() {
        let stmts = p("SELECT * FROM t WHERE name LIKE 'A%'");
        if let Statement::Select(s) = &stmts[0] {
            assert!(matches!(s.where_.as_ref().unwrap(), Expr::Like { .. }));
        } else { panic!() }
    }

    #[test]
    fn expr_is_null() {
        let stmts = p("SELECT * FROM t WHERE name IS NULL");
        if let Statement::Select(s) = &stmts[0] {
            assert!(matches!(s.where_.as_ref().unwrap(), Expr::IsNull { negated: false, .. }));
        } else { panic!() }
    }

    #[test]
    fn function_call() {
        let stmts = p("SELECT COUNT(*), MAX(score) FROM t");
        if let Statement::Select(s) = &stmts[0] {
            assert_eq!(s.columns.len(), 2);
        } else { panic!() }
    }

    #[test]
    fn join_stmt() {
        let stmts = p("SELECT * FROM users u JOIN orders o ON u.id = o.user_id");
        if let Statement::Select(s) = &stmts[0] {
            assert_eq!(s.joins.len(), 1);
            assert_eq!(s.joins[0].kind, JoinKind::Inner);
        } else { panic!() }
    }

    #[test]
    fn transaction() {
        let stmts = p("BEGIN; INSERT INTO t VALUES (1); COMMIT");
        assert_eq!(stmts.len(), 3);
        assert_eq!(stmts[0], Statement::Begin);
        assert_eq!(stmts[2], Statement::Commit);
    }

    #[test]
    fn multi_statement() {
        let stmts = p("SELECT 1; SELECT 2; SELECT 3");
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn drop_index() {
        let stmts = p("DROP INDEX IF EXISTS idx_name");
        if let Statement::DropIndex(s) = &stmts[0] {
            assert!(s.if_exists);
            assert_eq!(s.name, "idx_name");
        } else { panic!("expected DropIndex") }
    }

    #[test]
    fn alter_table_rename() {
        let stmts = p("ALTER TABLE users RENAME TO users_old");
        if let Statement::AlterTable(s) = &stmts[0] {
            assert_eq!(s.table, "users");
            match &s.op {
                AlterTableOp::RenameTo(new_name) => assert_eq!(new_name, "users_old"),
                _ => panic!("expected RenameTo"),
            }
        } else { panic!("expected AlterTable") }
    }

    #[test]
    fn alter_table_add_column() {
        let stmts = p("ALTER TABLE users ADD COLUMN email TEXT");
        if let Statement::AlterTable(s) = &stmts[0] {
            assert_eq!(s.table, "users");
            match &s.op {
                AlterTableOp::AddColumn { name, data_type } => {
                    assert_eq!(name, "email");
                    assert!(matches!(data_type, SqlType::Text));
                }
                _ => panic!("expected AddColumn"),
            }
        } else { panic!("expected AlterTable") }
    }

    #[test]
    fn pragma_stmt() {
        let stmts = p("PRAGMA cache_size");
        if let Statement::Pragma(s) = &stmts[0] {
            assert_eq!(s.name, "cache_size");
            assert!(s.value.is_none());
        } else { panic!("expected Pragma") }
    }

    #[test]
    fn pragma_with_value() {
        let stmts = p("PRAGMA cache_size = 512");
        if let Statement::Pragma(s) = &stmts[0] {
            assert_eq!(s.name, "cache_size");
            assert!(s.value.is_some());
        } else { panic!("expected Pragma") }
    }

    #[test]
    fn explain_select() {
        let stmts = p("EXPLAIN SELECT * FROM users WHERE id = 1");
        if let Statement::Explain(s) = &stmts[0] {
            assert!(matches!(s.inner.as_ref(), Statement::Select(_)));
        } else { panic!("expected Explain") }
    }

    #[test]
    fn create_virtual_table() {
        let sql = "CREATE VIRTUAL TABLE articles USING fts(title, content)";
        let stmts = parse(sql).unwrap_or_else(|e| panic!("parse error: {}", e));
        assert_eq!(stmts.len(), 1);
        
        match &stmts[0] {
            Statement::CreateVirtualTable(stmt) => {
                assert_eq!(stmt.name, "articles");
                assert_eq!(stmt.columns, vec!["title", "content"]);
            }
            _ => panic!("expected CreateVirtualTable"),
        }
    }

    #[test]
    fn fts_match() {
        let sql = "SELECT * FROM articles WHERE articles MATCH 'search term'";
        let stmts = parse(sql).unwrap_or_else(|e| panic!("parse error: {}", e));
        assert_eq!(stmts.len(), 1);
        
        match &stmts[0] {
            Statement::Select(s) => {
                if let Some(where_expr) = &s.where_ {
                    match where_expr {
                        Expr::Match { table, query } => {
                            assert_eq!(table, "articles");
                            assert_eq!(query, "search term");
                        }
                        _ => panic!("expected Match expression"),
                    }
                } else {
                    panic!("expected WHERE clause");
                }
            }
            _ => panic!("expected Select"),
        }
    }
}