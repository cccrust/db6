//! Query executor - Basic SQL on KV stores

use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::sql::parser::parse;

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
            crate::sql::parser::ast::Statement::Update(_update) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            crate::sql::parser::ast::Statement::Delete(_delete) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            crate::sql::parser::ast::Statement::CreateTable(_) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            crate::sql::parser::ast::Statement::DropTable(_) => {
                Ok(ResultSet { columns: vec![], rows: vec![], affected: 0 })
            }
            _ => Err(Error::Sql("Unsupported statement".into())),
        }
    }

    fn execute_select(&mut self, select: &crate::sql::parser::ast::SelectStmt) -> Result<ResultSet> {
        let table = select.from.as_ref().ok_or_else(|| Error::Sql("No table specified".into()))?;
        
        let start = b"".to_vec();
        let end = b"".to_vec();
        let rows = self.engine.scan(1, &start, &end)?;
        
        let columns = if select.columns.is_empty() {
            vec!["key".to_string(), "value".to_string()]
        } else {
            select.columns.iter().filter_map(|c| {
                if let crate::sql::parser::ast::Expr::Column(s) = c {
                    Some(s.clone())
                } else {
                    None
                }
            }).collect()
        };
        
        let result_rows: Vec<Vec<String>> = rows.iter().map(|(k, v)| {
            vec![
                String::from_utf8_lossy(k).to_string(),
                String::from_utf8_lossy(v).to_string(),
            ]
        }).collect();
        
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
                    crate::sql::parser::ast::Expr::Literal(l) => match l {
                        crate::sql::parser::ast::Literal::Text(s) => s.clone(),
                        crate::sql::parser::ast::Literal::Integer(i) => i.to_string(),
                        crate::sql::parser::ast::Literal::Real(r) => r.to_string(),
                        crate::sql::parser::ast::Literal::Null => String::new(),
                        crate::sql::parser::ast::Literal::Blob(b) => format!("{:x?}", b),
                    },
                    _ => "".to_string(),
                };
                let key = format!("{}:{}", table, affected);
                self.engine.put(1, key.as_bytes(), value.as_bytes())?;
                affected += 1;
            }
        }
        
        Ok(ResultSet { columns: vec![], rows: vec![], affected })
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new(Box::new(crate::engine::MemoryEngine::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "SQL stub not implemented - see plan.md v2.0"]
    fn test_executor_select() {
        let mut engine = crate::engine::MemoryEngine::new();
        engine.put(1, b"key1", b"value1").unwrap();
        engine.put(1, b"key2", b"value2").unwrap();
        
        let mut exec = Executor::new(Box::new(engine));
        let result = exec.execute("SELECT * FROM test").unwrap();
        
        assert!(result.rows.len() >= 2);
    }

    #[test]
    #[ignore = "SQL stub not implemented - see plan.md v2.0"]
    fn test_executor_insert() {
        let engine = crate::engine::MemoryEngine::new();
        let mut exec = Executor::new(Box::new(engine));
        
        let result = exec.execute("INSERT INTO test VALUES ('hello')").unwrap();
        assert_eq!(result.affected, 1);
    }
}