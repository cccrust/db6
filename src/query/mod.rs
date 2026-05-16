//! Query Module - Fluent Interface
//! 
//! 提供 Method Chaining 風格的 API for KV and SQL operations.

use std::path::Path;
use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::kv::KvEngine;
use crate::sql::ResultSet;

/// Db 主入口
pub struct Db {
    engine: KvEngine,
    table_map: std::collections::HashMap<String, u32>,
    next_table_id: u32,
}

impl Db {
    /// 建立記憶體資料庫
    pub fn new(engine_type: &str) -> Result<Self> {
        Ok(Db {
            engine: KvEngine::new(engine_type)?,
            table_map: std::collections::HashMap::new(),
            next_table_id: 1,
        })
    }

    /// 建立持久化資料庫
    pub fn open(engine_type: &str, path: &Path) -> Result<Self> {
        Ok(Db {
            engine: KvEngine::open(engine_type, path)?,
            table_map: std::collections::HashMap::new(),
            next_table_id: 1,
        })
    }

    fn get_table_id(&mut self, table_name: &str) -> u32 {
        if let Some(id) = self.table_map.get(table_name) {
            return *id;
        }
        let id = self.next_table_id;
        self.next_table_id += 1;
        self.table_map.insert(table_name.to_string(), id);
        id
    }

    /// Table fluent interface
    pub fn table(&mut self, table_name: &str) -> TableQuery<'_> {
        let table_id = self.get_table_id(table_name);
        TableQuery {
            db: self,
            table_name: table_name.to_string(),
            table_id,
        }
    }

    /// SELECT fluent interface
    pub fn select(&mut self, columns: &str) -> SelectQuery<'_> {
        SelectQuery {
            db: Some(self),
            columns: columns.to_string(),
            from: None,
            where_clause: None,
            order_by: None,
            group_by: None,
            having: None,
            limit: None,
        }
    }

    /// INSERT fluent interface
    pub fn insert(&mut self) -> InsertQuery<'_> {
        InsertQuery {
            db: self,
            into: None,
            columns: None,
            values: vec![],
        }
    }

    /// DELETE fluent interface
    pub fn delete(&mut self) -> DeleteQuery<'_> {
        DeleteQuery {
            db: self,
            from: None,
            where_clause: None,
        }
    }

    /// UPDATE fluent interface
    pub fn update(&mut self, table: &str) -> UpdateQuery<'_> {
        UpdateQuery {
            db: self,
            table: table.to_string(),
            set_value: None,
            where_clause: None,
        }
    }

    /// Transaction fluent interface
    pub fn begin_tx(&mut self) -> TransactionQuery<'_> {
        self.engine.begin_transaction().ok();
        TransactionQuery { db: self }
    }

    pub fn engine_type(&self) -> &'static str {
        self.engine.engine_type()
    }
}

/// Transaction fluent interface
pub struct TransactionQuery<'a> {
    db: &'a mut Db,
}

impl<'a> TransactionQuery<'a> {
    pub fn commit(mut self) -> Result<()> {
        self.db.engine.commit_transaction()
    }

    pub fn rollback(mut self) -> Result<()> {
        self.db.engine.rollback_transaction()
    }
}

/// Table fluent interface
pub struct TableQuery<'a> {
    db: &'a mut Db,
    table_name: String,
    table_id: u32,
}

impl<'a> TableQuery<'a> {
    pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<&mut Self> {
        self.db.engine.put(self.table_id, key, value)?;
        Ok(self)
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.db.engine.get(self.table_id, key)
    }

    pub fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        self.db.engine.scan(self.table_id, start, end)
    }

    pub fn delete(&mut self, key: &[u8]) -> Result<&mut Self> {
        self.db.engine.delete(self.table_id, key)?;
        Ok(self)
    }

    pub fn batch_put(&mut self, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<&mut Self> {
        self.db.engine.batch_put(self.table_id, pairs)?;
        Ok(self)
    }

    pub fn range_delete(&mut self, start: &[u8], end: &[u8]) -> Result<&mut Self> {
        self.db.engine.range_delete(self.table_id, start, end)?;
        Ok(self)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.db.engine.flush()
    }

    pub fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// INSERT fluent interface
pub struct InsertQuery<'a> {
    db: &'a mut Db,
    into: Option<String>,
    columns: Option<Vec<String>>,
    values: Vec<(String, String)>,
}

impl<'a> InsertQuery<'a> {
    /// INTO table_name
    pub fn into_table(&mut self, table: &str) -> &mut Self {
        self.into = Some(table.to_string());
        self
    }

    /// 批次 values - [(key, value), ...]
    pub fn values(&mut self, values: Vec<(impl Into<String>, impl Into<String>)>) -> &mut Self {
        for (k, v) in values {
            self.values.push((k.into(), v.into()));
        }
        self
    }

    /// Execute INSERT
    pub fn execute(&mut self) -> Result<usize> {
        let table = self.into.as_ref()
            .ok_or_else(|| Error::Sql("No table specified for INSERT".into()))?;

        let table_id = self.db.get_table_id(table);
        let count = self.values.len();

        for (ref key, ref value) in &self.values {
            self.db.engine.put(table_id, key.as_bytes(), value.as_bytes())?;
        }

        Ok(count)
    }
}

/// DELETE fluent interface
pub struct DeleteQuery<'a> {
    db: &'a mut Db,
    from: Option<String>,
    where_clause: Option<String>,
}

impl<'a> DeleteQuery<'a> {
    pub fn from(&mut self, table: &str) -> &mut Self {
        self.from = Some(table.to_string());
        self
    }

    pub fn where_(&mut self, condition: &str) -> &mut Self {
        self.where_clause = Some(condition.to_string());
        self
    }

    pub fn execute(&mut self) -> Result<usize> {
        let table = self.from.as_ref()
            .ok_or_else(|| Error::Sql("No table specified for DELETE".into()))?;

        let table_id = self.db.get_table_id(table);

        let rows = self.db.engine.scan(table_id, b"", b"")?;
        let filtered = match &self.where_clause {
            Some(cond) => filter_rows(rows, cond),
            None => rows,
        };

        let count = filtered.len();
        for (key, _) in filtered {
            self.db.engine.delete(table_id, &key)?;
        }

        Ok(count)
    }
}

/// UPDATE fluent interface
pub struct UpdateQuery<'a> {
    db: &'a mut Db,
    table: String,
    set_value: Option<String>,
    where_clause: Option<String>,
}

impl<'a> UpdateQuery<'a> {
    pub fn set_value(&mut self, value: &str) -> &mut Self {
        self.set_value = Some(value.to_string());
        self
    }

    pub fn where_(&mut self, condition: &str) -> &mut Self {
        self.where_clause = Some(condition.to_string());
        self
    }

    pub fn execute(&mut self) -> Result<usize> {
        let table_id = self.db.get_table_id(&self.table);

        let set_value = self.set_value.as_ref()
            .ok_or_else(|| Error::Sql("No SET clause specified for UPDATE".into()))?;

        let rows = self.db.engine.scan(table_id, b"", b"")?;
        let filtered = match &self.where_clause {
            Some(cond) => filter_rows(rows, cond),
            None => rows,
        };

        let count = filtered.len();
        for (ref key, _) in filtered {
            self.db.engine.put(table_id, key, set_value.as_bytes())?;
        }

        Ok(count)
    }
}

/// SELECT fluent interface
pub struct SelectQuery<'a> {
    db: Option<&'a mut Db>,
    columns: String,
    from: Option<String>,
    where_clause: Option<String>,
    order_by: Option<String>,
    group_by: Option<String>,
    having: Option<String>,
    limit: Option<usize>,
}

impl<'a> SelectQuery<'a> {
    pub fn from(&mut self, table: &str) -> &mut Self {
        self.from = Some(table.to_string());
        self
    }

    pub fn where_(&mut self, condition: &str) -> &mut Self {
        self.where_clause = Some(condition.to_string());
        self
    }

    pub fn order_by(&mut self, field: &str) -> &mut Self {
        self.order_by = Some(field.to_string());
        self
    }

    pub fn group_by(&mut self, field: &str) -> &mut Self {
        self.group_by = Some(field.to_string());
        self
    }

    pub fn having(&mut self, condition: &str) -> &mut Self {
        self.having = Some(condition.to_string());
        self
    }

    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.limit = Some(n);
        self
    }

    /// Execute query
    pub fn execute(&mut self) -> Result<ResultSet> {
        let db = self.db.take().ok_or_else(|| Error::Sql("DB not available".into()))?;
        
        let table_name = self.from.as_ref()
            .ok_or_else(|| Error::Sql("No table specified".into()))?;
        
        let table_id = db.get_table_id(table_name);
        
        // Scan all data
        let mut rows = db.engine.scan(table_id, b"", b"")?;
        
        // Apply WHERE filtering
        if let Some(ref where_cond) = self.where_clause {
            rows = filter_rows(rows, where_cond);
        }
        
        // Apply ORDER BY
        if let Some(ref order) = self.order_by {
            if db.engine.engine_type() != "memory-hash" {
                rows.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        
        // Apply LIMIT
        if let Some(limit) = self.limit {
            rows.truncate(limit);
        }
        
        // Build result
        let columns: Vec<String> = if self.columns == "*" {
            vec!["key".to_string(), "value".to_string()]
        } else {
            self.columns.split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };
        
        let result_rows: Vec<Vec<String>> = rows.iter()
            .map(|(k, v)| {
                if columns.len() == 1 && columns[0] == "key" {
                    vec![String::from_utf8_lossy(k).to_string()]
                } else if columns.len() == 1 && columns[0] == "value" {
                    vec![String::from_utf8_lossy(v).to_string()]
                } else {
                    vec![
                        String::from_utf8_lossy(k).to_string(),
                        String::from_utf8_lossy(v).to_string(),
                    ]
                }
            })
            .collect();
        
        Ok(ResultSet {
            columns,
            rows: result_rows,
            affected: 0,
        })
    }
}

/// Filter rows based on WHERE condition
/// Supports: key = value, key > value, key >= value, key < value, key <= value, key != value, key LIKE pattern
/// Supports AND/OR combinations: "value = Bob AND key > 1" or "value = Bob OR key < 3"
fn filter_rows(rows: Vec<(Vec<u8>, Vec<u8>)>, condition: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
    let condition = condition.trim();

    // Check for AND/OR operators
    let has_and = condition.contains(" AND ");
    let has_or = condition.contains(" OR ");

    if has_and {
        // Split by AND and apply each condition
        let parts: Vec<&str> = condition.split(" AND ").collect();
        let mut result = rows;
        for part in parts {
            result = filter_single_condition(result, part.trim());
        }
        return result;
    }

    if has_or {
        // Split by OR and union results
        let parts: Vec<&str> = condition.split(" OR ").collect();
        let mut result: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut seen: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
        for part in parts {
            let filtered = filter_single_condition(rows.clone(), part.trim());
            for row in filtered {
                if seen.insert(row.0.clone()) {
                    result.push(row);
                }
            }
        }
        return result;
    }

    // Single condition
    filter_single_condition(rows, condition)
}

fn filter_single_condition(rows: Vec<(Vec<u8>, Vec<u8>)>, condition: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
    let condition = condition.trim();

    // Parse condition: "field op value"
    let parts: Vec<&str> = condition.split_whitespace().collect();
    if parts.len() < 3 {
        return rows;
    }

    let field = parts[0];
    let op = parts[1];
    let value_str = parts[2..].join(" ");

    rows.into_iter().filter(|(k, v)| {
        let field_val = if field == "key" {
            String::from_utf8_lossy(k).to_string()
        } else {
            String::from_utf8_lossy(v).to_string()
        };

        match op {
            "=" | "==" => field_val == value_str,
            "!=" => field_val != value_str,
            ">" => field_val > value_str,
            ">=" => field_val >= value_str,
            "<" => field_val < value_str,
            "<=" => field_val <= value_str,
            "LIKE" => {
                let pattern = value_str.trim_matches(|c| c == '\'' || c == '%');
                if pattern.starts_with('%') && pattern.ends_with('%') {
                    field_val.contains(&pattern[1..pattern.len()-1])
                } else if pattern.ends_with('%') {
                    field_val.starts_with(&pattern[..pattern.len()-1])
                } else if pattern.starts_with('%') {
                    field_val.ends_with(&pattern[1..])
                } else {
                    field_val == pattern
                }
            }
            _ => true,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_new() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"key", b"value").unwrap();
        assert_eq!(db.table("users").get(b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_table_chaining() {
        let mut db = Db::new("memory").unwrap();
        db.table("users")
            .put(b"k1", b"v1")
            .unwrap()
            .put(b"k2", b"v2")
            .unwrap();
        
        let val = db.table("users").get(b"k1").unwrap();
        assert_eq!(val, Some(b"v1".to_vec()));
    }

    #[test]
    fn test_select() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();
        
        let result = db.select("*")
            .from("users")
            .limit(10)
            .execute()
            .unwrap();
        
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_insert() {
        let mut db = Db::new("memory").unwrap();
        
        let count = db.insert()
            .into_table("users")
            .values(vec![("1", "Alice"), ("2", "Bob")])
            .execute()
            .unwrap();
        
        assert_eq!(count, 2);
        assert_eq!(db.table("users").get(b"1").unwrap(), Some(b"Alice".to_vec()));
    }

    #[test]
    fn test_where() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();
        db.table("users").put(b"3", b"Charlie").unwrap();

        // WHERE value = Bob
        let result = db.select("key, value")
            .from("users")
            .where_("value = Bob")
            .execute()
            .unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][1], "Bob");
    }

    #[test]
    fn test_delete() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();
        db.table("users").put(b"3", b"Charlie").unwrap();

        // DELETE where value = Bob
        let count = db.delete()
            .from("users")
            .where_("value = Bob")
            .execute()
            .unwrap();

        assert_eq!(count, 1);
        assert_eq!(db.table("users").get(b"2").unwrap(), None);
        assert_eq!(db.table("users").get(b"1").unwrap(), Some(b"Alice".to_vec()));
    }

    #[test]
    fn test_update() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();
        db.table("users").put(b"3", b"Charlie").unwrap();

        // UPDATE users SET value = "Robert" where value = "Bob"
        let count = db.update("users")
            .set_value("Robert")
            .where_("value = Bob")
            .execute()
            .unwrap();

        assert_eq!(count, 1);
        // Now row with key="2" has value="Robert"
        assert_eq!(db.table("users").get(b"2").unwrap(), Some(b"Robert".to_vec()));
    }

    #[test]
    fn test_transaction_commit() {
        let temp_dir = std::env::temp_dir().join("db6_tx_test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let mut db = Db::open("lsm", &temp_dir).unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();

        db.begin_tx()
            .commit()
            .unwrap();

        assert_eq!(db.table("users").get(b"1").unwrap(), Some(b"Alice".to_vec()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_transaction_rollback() {
        let temp_dir = std::env::temp_dir().join("db6_tx_test2");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let mut db = Db::open("lsm", &temp_dir).unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();

        db.begin_tx()
            .rollback()
            .unwrap();

        // After rollback, all data should still exist
        assert_eq!(db.table("users").get(b"2").unwrap(), Some(b"Bob".to_vec()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_where_and() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();
        db.table("users").put(b"3", b"Charlie").unwrap();

        // WHERE key = 2 AND value = Bob
        let result = db.select("key, value")
            .from("users")
            .where_("key = 2 AND value = Bob")
            .execute()
            .unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], "2");
        assert_eq!(result.rows[0][1], "Bob");
    }

    #[test]
    fn test_where_or() {
        let mut db = Db::new("memory").unwrap();
        db.table("users").put(b"1", b"Alice").unwrap();
        db.table("users").put(b"2", b"Bob").unwrap();
        db.table("users").put(b"3", b"Charlie").unwrap();

        // WHERE key = 1 OR value = Charlie
        let result = db.select("key, value")
            .from("users")
            .where_("key = 1 OR value = Charlie")
            .execute()
            .unwrap();

        assert_eq!(result.rows.len(), 2);
        let keys: Vec<&str> = result.rows.iter().map(|r| r[0].as_str()).collect();
        assert!(keys.contains(&"1"));
        assert!(keys.contains(&"3"));
    }
}