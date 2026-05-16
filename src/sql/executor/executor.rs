//! Query executor - Basic SQL on KV stores

use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::sql::parser::parse;
use super::json_path::eval_expr;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub affected: usize,
}

pub struct Executor {
    engine: Box<dyn StorageEngine>,
}

impl Executor {
    pub fn new(engine: Box<dyn StorageEngine>) -> Self {
        Self { engine }
    }

    pub fn execute(&mut self, sql: &str) -> Result<ResultSet> {
        let stmts = parse(sql).map_err(|e| Error::Sql(e.to_string()))?;

        if stmts.is_empty() {
            return Ok(ResultSet::default());
        }

        let stmt = &stmts[0];

        match stmt {
            crate::sql::parser::ast::Statement::Select(select) => {
                self.execute_select(select)
            }
            crate::sql::parser::ast::Statement::Insert(insert) => {
                self.execute_insert(insert)
            }
            crate::sql::parser::ast::Statement::Update(update) => {
                self.execute_update(update)
            }
            crate::sql::parser::ast::Statement::Delete(delete) => {
                self.execute_delete(delete)
            }
            crate::sql::parser::ast::Statement::CreateTable(_) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            crate::sql::parser::ast::Statement::DropTable(_) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            crate::sql::parser::ast::Statement::CreateVirtualTable(_) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            _ => Err(Error::Sql("Unsupported statement".into())),
        }
    }

    fn execute_select(&mut self, select: &crate::sql::parser::ast::SelectStmt) -> Result<ResultSet> {
        let _table = select.from.as_ref().ok_or_else(|| Error::Sql("No table specified".into()))?;

        let start = b"".to_vec();
        let end = b"".to_vec();
        let mut rows = self.engine.scan(1, &start, &end)?;

        if let Some(ref where_expr) = select.where_ {
            rows.retain(|(k, v)| eval_expr(where_expr, v));
        }

        let columns = if select.columns.is_empty() {
            vec!["key".to_string(), "value".to_string()]
        } else {
            select.columns.iter().filter_map(|c| {
                match c {
                    crate::sql::parser::ast::SelectItem::Star => Some("*".to_string()),
                    crate::sql::parser::ast::SelectItem::TableStar(t) => Some(format!("{}.*", t)),
                    crate::sql::parser::ast::SelectItem::Expr { alias, .. } => alias.clone(),
                }
            }).collect()
        };

        if !select.order_by.is_empty() {
            let order = &select.order_by[0];
            rows.sort_by(|a, b| {
                let cmp = a.0.cmp(&b.0);
                if order.asc { cmp } else { cmp.reverse() }
            });
        }

        let mut result_rows: Vec<Vec<String>> = rows.iter().map(|(k, v)| {
            vec![
                String::from_utf8_lossy(k).to_string(),
                String::from_utf8_lossy(v).to_string(),
            ]
        }).collect();

        if let Some(limit_expr) = &select.limit {
            if let crate::sql::parser::ast::Expr::LitInt(n) = limit_expr {
                let n = *n as usize;
                if n < result_rows.len() {
                    result_rows.truncate(n);
                }
            }
        }

        Ok(ResultSet {
            columns,
            rows: result_rows,
            affected: 0,
        })
    }

    fn execute_insert(&mut self, insert: &crate::sql::parser::ast::InsertStmt) -> Result<ResultSet> {
        let table = &insert.table;

        let mut affected = 0;
        for row in &insert.values {
            if let Some(expr) = row.first() {
                let value = match expr {
                    crate::sql::parser::ast::Expr::LitStr(s) => s.clone(),
                    crate::sql::parser::ast::Expr::LitInt(i) => i.to_string(),
                    crate::sql::parser::ast::Expr::LitFloat(f) => f.to_string(),
                    crate::sql::parser::ast::Expr::LitNull => String::new(),
                    _ => "".to_string(),
                };
                let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
                let key = format!("{}:{}", table, id);
                self.engine.put(1, key.as_bytes(), value.as_bytes())?;
                affected += 1;
            }
        }

        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }

    fn execute_update(&mut self, update: &crate::sql::parser::ast::UpdateStmt) -> Result<ResultSet> {
        let rows = self.engine.scan(1, b"", b"")?;
        let mut affected = 0;

        let new_value = if !update.sets.is_empty() {
            let (_, expr) = &update.sets[0];
            match expr {
                crate::sql::parser::ast::Expr::LitStr(s) => Some(s.clone()),
                crate::sql::parser::ast::Expr::LitInt(i) => Some(i.to_string()),
                crate::sql::parser::ast::Expr::LitFloat(f) => Some(f.to_string()),
                crate::sql::parser::ast::Expr::LitNull => Some(String::new()),
                _ => None,
            }
        } else {
            None
        };

        if let Some(value) = new_value {
            for (key, _) in rows {
                self.engine.put(1, &key, value.as_bytes())?;
                affected += 1;
            }
        }

        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }

    fn execute_delete(&mut self, delete: &crate::sql::parser::ast::DeleteStmt) -> Result<ResultSet> {
        let rows = self.engine.scan(1, b"", b"")?;
        let mut affected = 0;

        for (key, _) in rows {
            self.engine.delete(1, &key)?;
            affected += 1;
        }

        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new(Box::new(crate::engine::BTreeMemoryEngine::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_insert_and_select() {
        let engine = crate::engine::BTreeMemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));

        exec.execute("INSERT INTO test VALUES ('{\"name\":\"Alice\",\"age\":30}')").unwrap();
        let result = exec.execute("SELECT * FROM test").unwrap();

        assert_eq!(result.rows.len(), 1, "Should have 1 row");
        assert_eq!(result.rows[0][1], "{\"name\":\"Alice\",\"age\":30}");
    }

    #[test]
    fn test_executor_multiple_insert() {
        let engine = crate::engine::BTreeMemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));

        exec.execute("INSERT INTO test VALUES ('v1')").unwrap();
        exec.execute("INSERT INTO test VALUES ('v2')").unwrap();
        exec.execute("INSERT INTO test VALUES ('v3')").unwrap();

        let result = exec.execute("SELECT * FROM test").unwrap();
        assert_eq!(result.rows.len(), 3, "Should have 3 rows");
    }

    #[test]
    fn test_json_path_filter() {
        let engine = crate::engine::BTreeMemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));

        exec.execute("INSERT INTO users VALUES ('{\"name\":\"Alice\",\"age\":30}')").unwrap();
        exec.execute("INSERT INTO users VALUES ('{\"name\":\"Bob\",\"age\":25}')").unwrap();
        exec.execute("INSERT INTO users VALUES ('{\"name\":\"Charlie\",\"age\":35}')").unwrap();

        let result = exec.execute("SELECT * FROM users WHERE @.age > 27").unwrap();
        assert_eq!(result.rows.len(), 2, "Should have 2 users with age > 27");
    }

    #[test]
    fn test_json_path_is_null() {
        let engine = crate::engine::BTreeMemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));

        exec.execute("INSERT INTO users VALUES ('{\"name\":\"Alice\"}')").unwrap();
        exec.execute("INSERT INTO users VALUES ('{\"name\":\"Bob\",\"phone\":\"123\"}')").unwrap();

        let result = exec.execute("SELECT * FROM users WHERE @.phone IS NULL").unwrap();
        assert_eq!(result.rows.len(), 1, "Should have 1 user without phone");
    }

    #[test]
    fn test_executor_insert() {
        let engine = crate::engine::BTreeMemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));

        let result = exec.execute("INSERT INTO test VALUES ('hello')").unwrap();
        assert_eq!(result.affected, 1);
    }
}

// =============================================================================
// Typed Executor with compile-time capability checking
// =============================================================================

pub struct SqlExecutor<E: crate::engine::StorageEngine> {
    engine: E,
}

impl<E: crate::engine::StorageEngine> SqlExecutor<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    pub fn execute_order_by(&mut self, sql: &str) -> Result<ResultSet>
    where
        E: crate::engine::CanOrderBy,
    {
        self.execute(sql)
    }

    pub fn execute_fts(&mut self, sql: &str) -> Result<ResultSet>
    where
        E: crate::engine::CanFts,
    {
        self.execute(sql)
    }

    pub fn execute_with_tx(&mut self, sql: &str) -> Result<ResultSet>
    where
        E: crate::engine::CanTransaction,
    {
        self.execute(sql)
    }

    pub fn execute(&mut self, sql: &str) -> Result<ResultSet> {
        let stmts = parse(sql).map_err(|e| Error::Sql(e.to_string()))?;

        if stmts.is_empty() {
            return Ok(ResultSet::default());
        }

        let stmt = &stmts[0];

        match stmt {
            crate::sql::parser::ast::Statement::Select(select) => {
                self.execute_select(select)
            }
            crate::sql::parser::ast::Statement::Insert(insert) => {
                self.execute_insert(insert)
            }
            crate::sql::parser::ast::Statement::Update(update) => {
                self.execute_update(update)
            }
            crate::sql::parser::ast::Statement::Delete(delete) => {
                self.execute_delete(delete)
            }
            _ => Err(Error::Sql("Unsupported statement".into())),
        }
    }

    fn execute_select(&mut self, select: &crate::sql::parser::ast::SelectStmt) -> Result<ResultSet> {
        let _table = select.from.as_ref().ok_or_else(|| Error::Sql("No table specified".into()))?;

        let start = b"".to_vec();
        let end = b"".to_vec();
        let mut rows = self.engine.scan(1, &start, &end)?;

        if let Some(ref where_expr) = select.where_ {
            rows.retain(|(k, v)| eval_expr(where_expr, v));
        }

        let columns = if select.columns.is_empty() {
            vec!["key".to_string(), "value".to_string()]
        } else {
            select.columns.iter().filter_map(|c| {
                match c {
                    crate::sql::parser::ast::SelectItem::Star => Some("*".to_string()),
                    crate::sql::parser::ast::SelectItem::TableStar(t) => Some(format!("{}.*", t)),
                    crate::sql::parser::ast::SelectItem::Expr { alias, .. } => alias.clone(),
                }
            }).collect()
        };

        if !select.order_by.is_empty() {
            let order = &select.order_by[0];
            rows.sort_by(|a, b| {
                let cmp = a.0.cmp(&b.0);
                if order.asc { cmp } else { cmp.reverse() }
            });
        }

        let mut result_rows: Vec<Vec<String>> = rows.iter().map(|(k, v)| {
            vec![
                String::from_utf8_lossy(k).to_string(),
                String::from_utf8_lossy(v).to_string(),
            ]
        }).collect();

        if let Some(limit_expr) = &select.limit {
            if let crate::sql::parser::ast::Expr::LitInt(n) = limit_expr {
                let n = *n as usize;
                if n < result_rows.len() {
                    result_rows.truncate(n);
                }
            }
        }

        Ok(ResultSet {
            columns,
            rows: result_rows,
            affected: 0,
        })
    }

    fn execute_insert(&mut self, insert: &crate::sql::parser::ast::InsertStmt) -> Result<ResultSet> {
        let table = &insert.table;

        let mut affected = 0;
        for row in &insert.values {
            if let Some(expr) = row.first() {
                let value = match expr {
                    crate::sql::parser::ast::Expr::LitStr(s) => s.clone(),
                    crate::sql::parser::ast::Expr::LitInt(i) => i.to_string(),
                    crate::sql::parser::ast::Expr::LitFloat(f) => f.to_string(),
                    crate::sql::parser::ast::Expr::LitNull => String::new(),
                    _ => "".to_string(),
                };
                let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
                let key = format!("{}:{}", table, id);
                self.engine.put(1, key.as_bytes(), value.as_bytes())?;
                affected += 1;
            }
        }

        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }

    fn execute_update(&mut self, update: &crate::sql::parser::ast::UpdateStmt) -> Result<ResultSet> {
        let rows = self.engine.scan(1, b"", b"")?;
        let mut affected = 0;

        let new_value = if !update.sets.is_empty() {
            let (_, expr) = &update.sets[0];
            match expr {
                crate::sql::parser::ast::Expr::LitStr(s) => Some(s.clone()),
                crate::sql::parser::ast::Expr::LitInt(i) => Some(i.to_string()),
                crate::sql::parser::ast::Expr::LitFloat(f) => Some(f.to_string()),
                crate::sql::parser::ast::Expr::LitNull => Some(String::new()),
                _ => None,
            }
        } else {
            None
        };

        if let Some(value) = new_value {
            for (key, _) in rows {
                self.engine.put(1, &key, value.as_bytes())?;
                affected += 1;
            }
        }

        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }

    fn execute_delete(&mut self, delete: &crate::sql::parser::ast::DeleteStmt) -> Result<ResultSet> {
        let rows = self.engine.scan(1, b"", b"")?;
        let mut affected = 0;

        for (key, _) in rows {
            self.engine.delete(1, &key)?;
            affected += 1;
        }

        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }
}