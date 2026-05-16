//! KV API 統一介面
//! 
//! 提供 KvEngine 工廠，讓使用者更方便建立 KV store。

use std::path::Path;
use crate::error::Result;
use crate::engine::StorageEngine;

/// KV Store 統一介面
pub trait KvStore {
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()>;
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn engine_type(&self) -> &'static str;
}

/// KV Engine 枚举
/// 
/// 使用 `KvEngine::new("memory")` 或 `KvEngine::open("btree", "/path")` 建立
pub enum KvEngine {
    /// HashMemoryEngine - Redis-like, O(1) 操作，無 ORDER BY
    Hash(crate::engine::HashMemoryEngine),
    /// BTreeMemoryEngine - SQLite-like, O(log n)，支援 ORDER BY
    BTreeMem(crate::engine::BTreeMemoryEngine),
    /// BTreeEngine - 磁碟持久化 BTree
    BTree(crate::engine::BTreeEngine),
    /// LsmEngine - LSM tree with WAL
    Lsm(crate::engine::LsmEngine),
}

impl KvEngine {
    /// 建立記憶體 KV store
    /// 
    /// # Example
    /// ```ignore
    /// use db6::kv::{KvStore, KvEngine};
    /// let mut kv = KvEngine::new("memory")?;  // 使用 HashMemoryEngine
    /// let mut kv = KvEngine::new("btree")?;  // 使用 BTreeMemoryEngine
    /// let mut kv = KvEngine::new("lsm")?;     // 使用 LsmEngine (memory mode)
    /// ```
    pub fn new(engine_type: &str) -> Result<Self> {
        match engine_type.to_lowercase().as_str() {
            "memory" | "hash" => Ok(KvEngine::Hash(crate::engine::HashMemoryEngine::new())),
            "btree" | "btree-mem" => Ok(KvEngine::BTreeMem(crate::engine::BTreeMemoryEngine::new())),
            "lsm" => Ok(KvEngine::Lsm(crate::engine::LsmEngine::new())),
            _ => Err(crate::error::Error::InvalidEngine(engine_type.to_string())),
        }
    }

    /// 建立持久化 KV store
    /// 
    /// # Example
    /// ```ignore
    /// use db6::kv::{KvStore, KvEngine};
    /// let mut kv = KvEngine::open("btree", "/data/myapp")?;
    /// let mut kv = KvEngine::open("lsm", "/data/myapp")?;
    /// ```
    pub fn open(engine_type: &str, path: &Path) -> Result<Self> {
        match engine_type.to_lowercase().as_str() {
            "btree" => Ok(KvEngine::BTree(crate::engine::BTreeEngine::open(path)?)),
            "lsm" => Ok(KvEngine::Lsm(crate::engine::LsmEngine::open(path)?)),
            _ => Err(crate::error::Error::InvalidEngine(format!("{} does not support open(path)", engine_type))),
        }
    }
}

impl KvStore for KvEngine {
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.put(table_id, key, value),
            KvEngine::BTreeMem(e) => e.put(table_id, key, value),
            KvEngine::BTree(e) => e.put(table_id, key, value),
            KvEngine::Lsm(e) => e.put(table_id, key, value),
        }
    }

    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self {
            KvEngine::Hash(e) => e.get(table_id, key),
            KvEngine::BTreeMem(e) => e.get(table_id, key),
            KvEngine::BTree(e) => e.get(table_id, key),
            KvEngine::Lsm(e) => e.get(table_id, key),
        }
    }

    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.delete(table_id, key),
            KvEngine::BTreeMem(e) => e.delete(table_id, key),
            KvEngine::BTree(e) => e.delete(table_id, key),
            KvEngine::Lsm(e) => e.delete(table_id, key),
        }
    }

    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        match self {
            KvEngine::Hash(e) => e.scan(table_id, start, end),
            KvEngine::BTreeMem(e) => e.scan(table_id, start, end),
            KvEngine::BTree(e) => e.scan(table_id, start, end),
            KvEngine::Lsm(e) => e.scan(table_id, start, end),
        }
    }

    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.batch_put(table_id, pairs),
            KvEngine::BTreeMem(e) => e.batch_put(table_id, pairs),
            KvEngine::BTree(e) => e.batch_put(table_id, pairs),
            KvEngine::Lsm(e) => e.batch_put(table_id, pairs),
        }
    }

    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.range_delete(table_id, start, end),
            KvEngine::BTreeMem(e) => e.range_delete(table_id, start, end),
            KvEngine::BTree(e) => e.range_delete(table_id, start, end),
            KvEngine::Lsm(e) => e.range_delete(table_id, start, end),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.flush(),
            KvEngine::BTreeMem(e) => e.flush(),
            KvEngine::BTree(e) => e.flush(),
            KvEngine::Lsm(e) => e.flush(),
        }
    }

    fn engine_type(&self) -> &'static str {
        match self {
            KvEngine::Hash(e) => e.engine_type(),
            KvEngine::BTreeMem(e) => e.engine_type(),
            KvEngine::BTree(e) => e.engine_type(),
            KvEngine::Lsm(e) => e.engine_type(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_engine_new() {
        let mut kv = KvEngine::new("memory").unwrap();
        kv.put(1, b"key", b"value").unwrap();
        assert_eq!(kv.get(1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_kv_engine_btree_mem() {
        let mut kv = KvEngine::new("btree").unwrap();
        kv.put(1, b"key", b"value").unwrap();
        assert_eq!(kv.get(1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_kv_engine_lsm() {
        let mut kv = KvEngine::new("lsm").unwrap();
        kv.put(1, b"key", b"value").unwrap();
        assert_eq!(kv.get(1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_kv_engine_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_kv_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        {
            let mut kv = KvEngine::open("btree", &temp_dir).unwrap();
            kv.put(1, b"key1", b"value1").unwrap();
            kv.flush().unwrap();
        }
        
        {
            let kv = KvEngine::open("btree", &temp_dir).unwrap();
            assert_eq!(kv.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
        }
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}