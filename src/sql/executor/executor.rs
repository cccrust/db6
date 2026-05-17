//! Query executor - Basic SQL on KV stores

use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::sql::parser::parse;
use super::json_path::eval_expr;
use std::sync::atomic::{AtomicUsize, Ordering};

fn apply_group_by(
    rows: Vec<(Vec<u8>, Vec<u8>)>,
    group_by: &[crate::sql::parser::ast::Expr],
    having: &Option<crate::sql::parser::ast::Expr>,
    columns: &[crate::sql::parser::ast::SelectItem],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    use std::collections::HashMap;
    use crate::sql::parser::ast::Expr;
    use crate::sql::parser::ast::{BinOp, SelectItem};

    if group_by.is_empty() {
        return Ok(rows);
    }

    let mut groups: HashMap<String, Vec<(Vec<u8>, Vec<u8>)>> = HashMap::new();

    for (k, v) in rows {
        let group_key = match &group_by[0] {
            Expr::Column { table: _, name } if name == "key" => {
                String::from_utf8_lossy(&k).to_string()
            }
            Expr::Column { table: _, name } if name == "value" => {
                String::from_utf8_lossy(&v).to_string()
            }
            Expr::Column { table: _, name } => {
                let val = String::from_utf8_lossy(&v);
                json_path_get(&val, &[name.clone()])
            }
            Expr::JsonPath { path, .. } => {
                let val = String::from_utf8_lossy(&v);
                json_path_get(&val, path)
            }
            _ => String::from_utf8_lossy(&v).to_string(),
        };
        groups.entry(group_key).or_insert_with(Vec::new).push((k, v));
    }

    let mut result: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    for (group_key, group_rows) in groups {
        let grouped_row = group_key.clone().into_bytes();

        let value = if columns.iter().any(|c| matches!(c, SelectItem::Expr { expr: Expr::Function { name, .. }, .. } if name.eq_ignore_ascii_case("count"))) {
            group_rows.len().to_string()
        } else if columns.iter().any(|c| matches!(c, SelectItem::Expr { expr: Expr::Function { name, .. }, .. } if name.eq_ignore_ascii_case("sum"))) {
            let sum: f64 = group_rows.iter()
                .filter_map(|(_, v)| String::from_utf8_lossy(v).parse::<f64>().ok())
                .sum();
            sum.to_string()
        } else if columns.iter().any(|c| matches!(c, SelectItem::Expr { expr: Expr::Function { name, .. }, .. } if name.eq_ignore_ascii_case("avg"))) {
            let sum: f64 = group_rows.iter()
                .filter_map(|(_, v)| String::from_utf8_lossy(v).parse::<f64>().ok())
                .sum();
            let count = group_rows.len() as f64;
            if count > 0.0 {
                (sum / count).to_string()
            } else {
                "0".to_string()
            }
        } else if columns.iter().any(|c| matches!(c, SelectItem::Expr { expr: Expr::Function { name, .. }, .. } if name.eq_ignore_ascii_case("min"))) {
            group_rows.iter()
                .filter_map(|(_, v)| String::from_utf8_lossy(v).parse::<f64>().ok())
                .fold(f64::INFINITY, f64::min)
                .to_string()
        } else if columns.iter().any(|c| matches!(c, SelectItem::Expr { expr: Expr::Function { name, .. }, .. } if name.eq_ignore_ascii_case("max"))) {
            group_rows.iter()
                .filter_map(|(_, v)| String::from_utf8_lossy(v).parse::<f64>().ok())
                .fold(f64::NEG_INFINITY, f64::max)
                .to_string()
        } else {
            String::from_utf8_lossy(&group_rows[0].1).to_string()
        };

        if let Some(ref having_expr) = having {
            let mut passed = true;
            if let Expr::BinOp { left, op, right } = having_expr {
                if let Expr::Function { name: func_name, .. } = left.as_ref() {
                    let rhs = match right.as_ref() {
                        Expr::LitInt(n) => *n as f64,
                        Expr::LitFloat(f) => *f,
                        _ => 0.0,
                    };
                    let agg_value = value.parse::<f64>().unwrap_or(0.0);
                    passed = match op {
                        BinOp::Gt => agg_value > rhs,
                        BinOp::GtEq => agg_value >= rhs,
                        BinOp::Lt => agg_value < rhs,
                        BinOp::LtEq => agg_value <= rhs,
                        BinOp::Eq => (agg_value - rhs).abs() < 1e-9,
                        BinOp::NotEq => (agg_value - rhs).abs() >= 1e-9,
                        _ => true,
                    };
                }
            }
            if !passed {
                continue;
            }
        }

        result.push((grouped_row, value.into_bytes()));
    }

    Ok(result)
}

fn json_path_get(json_str: &str, path: &[String]) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
        let mut current = &v;
        for key in path {
            if let Some(next) = current.get(key) {
                current = next;
            } else {
                return String::new();
            }
        }
        return match current {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            _ => current.to_string(),
        };
    }
    String::new()
}

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

        if !select.group_by.is_empty() {
            rows = apply_group_by(rows, &select.group_by, &select.having, &select.columns)?;
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

        let eval_lit = |expr: &crate::sql::parser::ast::Expr| -> String {
            match expr {
                crate::sql::parser::ast::Expr::LitStr(s) => s.clone(),
                crate::sql::parser::ast::Expr::LitInt(i) => i.to_string(),
                crate::sql::parser::ast::Expr::LitFloat(f) => f.to_string(),
                crate::sql::parser::ast::Expr::LitNull => String::new(),
                _ => "".to_string(),
            }
        };

        let mut affected = 0;
        for row in &insert.values {
            if !row.is_empty() {
                let (key, value) = if row.len() >= 2 {
                    (eval_lit(&row[0]), eval_lit(&row[1]))
                } else {
                    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
                    (format!("{}:{}", table, id), eval_lit(&row[0]))
                };
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
    fn test_executor_insert_two_values() {
        let engine = crate::engine::BTreeMemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));

        exec.execute("INSERT INTO test VALUES ('key1', '{\"name\":\"Alice\",\"age\":30}')").unwrap();
        exec.execute("INSERT INTO test VALUES ('key2', '{\"name\":\"Bob\",\"age\":25}')").unwrap();

        let result = exec.execute("SELECT * FROM test").unwrap();
        assert_eq!(result.rows.len(), 2, "Should have 2 rows");
        assert!(result.rows[0][1].contains("Alice"), "First row should contain Alice JSON");
        assert!(result.rows[1][1].contains("Bob"), "Second row should contain Bob JSON");
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

        if !select.group_by.is_empty() {
            rows = apply_group_by(rows, &select.group_by, &select.having, &select.columns)?;
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

        let eval_lit = |expr: &crate::sql::parser::ast::Expr| -> String {
            match expr {
                crate::sql::parser::ast::Expr::LitStr(s) => s.clone(),
                crate::sql::parser::ast::Expr::LitInt(i) => i.to_string(),
                crate::sql::parser::ast::Expr::LitFloat(f) => f.to_string(),
                crate::sql::parser::ast::Expr::LitNull => String::new(),
                _ => "".to_string(),
            }
        };

        let mut affected = 0;
        for row in &insert.values {
            if !row.is_empty() {
                let (key, value) = if row.len() >= 2 {
                    (eval_lit(&row[0]), eval_lit(&row[1]))
                } else {
                    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
                    (format!("{}:{}", table, id), eval_lit(&row[0]))
                };
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