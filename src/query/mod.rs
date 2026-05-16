//! Query Module - Fluent Interface
//! 
//! 提供 Method Chaining 風格的 API for KV and SQL operations.

use std::path::Path;
use crate::error::{Error, Result};
use crate::kv::{KvStore as KvApi, KvEngine};
use crate::sql::ResultSet;

/// Db 主入口
/// 
/// # Example
/// ```ignore
/// use db6::query::Db;
/// 
/// let mut db = Db::new("memory").unwrap();
/// db.table("users").put(b"key", b"value").unwrap();
/// ```
pub struct Db {
    engine: KvEngine,
    table_map: std::collections::HashMap<String, u32>,
    next_table_id: u32,
}

impl Db {
    /// 建立記憶體資料庫
    /// 
    /// # Example
    /// ```ignore
    /// let mut db = Db::new("memory").unwrap();  // HashMemoryEngine
    /// let mut db = Db::new("btree").unwrap();  // BTreeMemoryEngine
    /// let mut db = Db::new("lsm").unwrap();    // LsmEngine
    /// ```
    pub fn new(engine_type: &str) -> Result<Self> {
        Ok(Db {
            engine: KvEngine::new(engine_type)?,
            table_map: std::collections::HashMap::new(),
            next_table_id: 1,
        })
    }

    /// 建立持久化資料庫
    /// 
    /// # Example
    /// ```ignore
    /// let mut db = Db::open("btree", "/path/to/db").unwrap();
    /// let mut db = Db::open("lsm", "/path/to/db").unwrap();
    /// ```
    pub fn open(engine_type: &str, path: &Path) -> Result<Self> {
        Ok(Db {
            engine: KvEngine::open(engine_type, path)?,
            table_map: std::collections::HashMap::new(),
            next_table_id: 1,
        })
    }

    /// 取得 table_id，若不存在則建立
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
    /// 
    /// # Example
    /// ```ignore
    /// db.table("users").put(b"key", b"value").unwrap();
    /// db.table("users").get(b"key").unwrap();
    /// ```
    pub fn table(&mut self, table_name: &str) -> TableQuery<'_> {
        let table_id = self.get_table_id(table_name);
        TableQuery {
            db: self,
            table_name: table_name.to_string(),
            table_id,
        }
    }

    /// SELECT fluent interface
    /// 
    /// # Example
    /// ```ignore
    /// db.select("name", "age")
    ///     .from("users")
    ///     .where("age > 18")
    ///     .execute().unwrap();
    /// ```
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

    /// 取得 engine 類型
    pub fn engine_type(&self) -> &'static str {
        self.engine.engine_type()
    }
}

/// Table fluent interface
/// 
/// 支援 method chaining:
/// ```ignore
/// db.table("users")
///     .put(b"key1", b"value1")?
///     .put(b"key2", b"value2")?
///     .flush()?;
/// ```
pub struct TableQuery<'a> {
    db: &'a mut Db,
    table_name: String,
    table_id: u32,
}

impl<'a> TableQuery<'a> {
    /// 寫入 key-value
    pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<&mut Self> {
        self.db.engine.put(self.table_id, key, value)?;
        Ok(self)
    }

    /// 讀取 value
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.db.engine.get(self.table_id, key)
    }

    /// 掃描 range
    pub fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        self.db.engine.scan(self.table_id, start, end)
    }

    /// 刪除 key
    pub fn delete(&mut self, key: &[u8]) -> Result<&mut Self> {
        self.db.engine.delete(self.table_id, key)?;
        Ok(self)
    }

    /// 批次寫入
    pub fn batch_put(&mut self, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<&mut Self> {
        self.db.engine.batch_put(self.table_id, pairs)?;
        Ok(self)
    }

    /// 範圍刪除
    pub fn range_delete(&mut self, start: &[u8], end: &[u8]) -> Result<&mut Self> {
        self.db.engine.range_delete(self.table_id, start, end)?;
        Ok(self)
    }

    ///  flush to disk
    pub fn flush(&mut self) -> Result<()> {
        self.db.engine.flush()
    }

    /// 取得 table 名稱
    pub fn table_name(&self) -> &str {
        &self.table_name
    }
}

/// SELECT fluent interface
/// 
/// 支援 method chaining:
/// ```ignore
/// db.select("name, age")
///     .from("users")
///     .where("age > 18")
///     .order_by("name")
///     .limit(10)
///     .execute()?;
/// ```
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
    /// FROM clause
    pub fn from(&mut self, table: &str) -> &mut Self {
        self.from = Some(table.to_string());
        self
    }

    /// WHERE clause
    pub fn where_(&mut self, condition: &str) -> &mut Self {
        self.where_clause = Some(condition.to_string());
        self
    }

    /// ORDER BY clause
    pub fn order_by(&mut self, field: &str) -> &mut Self {
        self.order_by = Some(field.to_string());
        self
    }

    /// GROUP BY clause
    pub fn group_by(&mut self, field: &str) -> &mut Self {
        self.group_by = Some(field.to_string());
        self
    }

    /// HAVING clause
    pub fn having(&mut self, condition: &str) -> &mut Self {
        self.having = Some(condition.to_string());
        self
    }

    /// LIMIT clause
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
        
        // Apply ORDER BY (if engine supports)
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
}