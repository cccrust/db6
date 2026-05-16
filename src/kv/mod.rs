//! KV API 統一介面
//! 
//! 提供 KvEngine 工廠，讓使用者更方便建立 KV store。

use std::path::Path;
use std::sync::{Arc, RwLock};
use crate::error::{Error, Result};
use crate::engine::{EngineStats, StorageEngine};

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
    Hash(Arc<RwLock<crate::engine::HashMemoryEngine>>),
    BTreeMem(Arc<RwLock<crate::engine::BTreeMemoryEngine>>),
    BTree(Arc<RwLock<crate::engine::BTreeEngine>>),
    Lsm(Arc<RwLock<crate::engine::LsmEngine>>),
}

impl KvEngine {
    pub fn new(engine_type: &str) -> Result<Self> {
        match engine_type.to_lowercase().as_str() {
            "memory" | "hash" => Ok(KvEngine::Hash(Arc::new(RwLock::new(crate::engine::HashMemoryEngine::new())))),
            "btree" | "btree-mem" => Ok(KvEngine::BTreeMem(Arc::new(RwLock::new(crate::engine::BTreeMemoryEngine::new())))),
            "lsm" => Ok(KvEngine::Lsm(Arc::new(RwLock::new(crate::engine::LsmEngine::new())))),
            _ => Err(crate::error::Error::InvalidEngine(engine_type.to_string())),
        }
    }

    pub fn open(engine_type: &str, path: &Path) -> Result<Self> {
        match engine_type.to_lowercase().as_str() {
            "btree" => Ok(KvEngine::BTree(Arc::new(RwLock::new(crate::engine::BTreeEngine::open(path)?)))),
            "lsm" => Ok(KvEngine::Lsm(Arc::new(RwLock::new(crate::engine::LsmEngine::open(path)?)))),
            _ => Err(crate::error::Error::InvalidEngine(format!("{} does not support open(path)", engine_type))),
        }
    }
}

impl KvStore for KvEngine {
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().put(table_id, key, value),
            KvEngine::BTreeMem(e) => e.write().unwrap().put(table_id, key, value),
            KvEngine::BTree(e) => e.write().unwrap().put(table_id, key, value),
            KvEngine::Lsm(e) => e.write().unwrap().put(table_id, key, value),
        }
    }

    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self {
            KvEngine::Hash(e) => e.read().unwrap().get(table_id, key),
            KvEngine::BTreeMem(e) => e.read().unwrap().get(table_id, key),
            KvEngine::BTree(e) => e.read().unwrap().get(table_id, key),
            KvEngine::Lsm(e) => e.read().unwrap().get(table_id, key),
        }
    }

    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().delete(table_id, key),
            KvEngine::BTreeMem(e) => e.write().unwrap().delete(table_id, key),
            KvEngine::BTree(e) => e.write().unwrap().delete(table_id, key),
            KvEngine::Lsm(e) => e.write().unwrap().delete(table_id, key),
        }
    }

    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        match self {
            KvEngine::Hash(e) => e.read().unwrap().scan(table_id, start, end),
            KvEngine::BTreeMem(e) => e.read().unwrap().scan(table_id, start, end),
            KvEngine::BTree(e) => e.read().unwrap().scan(table_id, start, end),
            KvEngine::Lsm(e) => e.read().unwrap().scan(table_id, start, end),
        }
    }

    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().batch_put(table_id, pairs),
            KvEngine::BTreeMem(e) => e.write().unwrap().batch_put(table_id, pairs),
            KvEngine::BTree(e) => e.write().unwrap().batch_put(table_id, pairs),
            KvEngine::Lsm(e) => e.write().unwrap().batch_put(table_id, pairs),
        }
    }

    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().range_delete(table_id, start, end),
            KvEngine::BTreeMem(e) => e.write().unwrap().range_delete(table_id, start, end),
            KvEngine::BTree(e) => e.write().unwrap().range_delete(table_id, start, end),
            KvEngine::Lsm(e) => e.write().unwrap().range_delete(table_id, start, end),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().flush(),
            KvEngine::BTreeMem(e) => e.write().unwrap().flush(),
            KvEngine::BTree(e) => e.write().unwrap().flush(),
            KvEngine::Lsm(e) => e.write().unwrap().flush(),
        }
    }

    fn engine_type(&self) -> &'static str {
        match self {
            KvEngine::Hash(e) => e.read().unwrap().engine_type(),
            KvEngine::BTreeMem(e) => e.read().unwrap().engine_type(),
            KvEngine::BTree(e) => e.read().unwrap().engine_type(),
            KvEngine::Lsm(e) => e.read().unwrap().engine_type(),
        }
    }
}

impl StorageEngine for KvEngine {
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>>
    where
        Self: Sized,
    {
        Ok(Box::new(KvEngine::open("btree", path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine>
    where
        Self: Sized,
    {
        Box::new(KvEngine::new("memory").unwrap())
    }

    fn engine_type(&self) -> &'static str {
        <Self as KvStore>::engine_type(self)
    }

    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        KvStore::get(self, table_id, key)
    }

    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        KvStore::put(self, table_id, key, value)
    }

    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        KvStore::delete(self, table_id, key)
    }

    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        KvStore::scan(self, table_id, start, end)
    }

    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        KvStore::batch_put(self, table_id, pairs)
    }

    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()> {
        KvStore::range_delete(self, table_id, start, end)
    }

    fn flush(&mut self) -> Result<()> {
        KvStore::flush(self)
    }

    fn sync(&mut self) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().sync(),
            KvEngine::BTreeMem(e) => e.write().unwrap().sync(),
            KvEngine::BTree(e) => e.write().unwrap().sync(),
            KvEngine::Lsm(e) => e.write().unwrap().sync(),
        }
    }

    fn begin_transaction(&mut self) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().begin_transaction(),
            KvEngine::BTreeMem(e) => e.write().unwrap().begin_transaction(),
            KvEngine::BTree(e) => e.write().unwrap().begin_transaction(),
            KvEngine::Lsm(e) => e.write().unwrap().begin_transaction(),
        }
    }

    fn commit_transaction(&mut self) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().commit_transaction(),
            KvEngine::BTreeMem(e) => e.write().unwrap().commit_transaction(),
            KvEngine::BTree(e) => e.write().unwrap().commit_transaction(),
            KvEngine::Lsm(e) => e.write().unwrap().commit_transaction(),
        }
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        match self {
            KvEngine::Hash(e) => e.write().unwrap().rollback_transaction(),
            KvEngine::BTreeMem(e) => e.write().unwrap().rollback_transaction(),
            KvEngine::BTree(e) => e.write().unwrap().rollback_transaction(),
            KvEngine::Lsm(e) => e.write().unwrap().rollback_transaction(),
        }
    }

    fn has_transaction(&self) -> bool {
        match self {
            KvEngine::Hash(e) => e.read().unwrap().has_transaction(),
            KvEngine::BTreeMem(e) => e.read().unwrap().has_transaction(),
            KvEngine::BTree(e) => e.read().unwrap().has_transaction(),
            KvEngine::Lsm(e) => e.read().unwrap().has_transaction(),
        }
    }

    fn stats(&self) -> EngineStats {
        match self {
            KvEngine::Hash(e) => e.read().unwrap().stats(),
            KvEngine::BTreeMem(e) => e.read().unwrap().stats(),
            KvEngine::BTree(e) => e.read().unwrap().stats(),
            KvEngine::Lsm(e) => e.read().unwrap().stats(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_engine_new() {
        let mut kv = KvEngine::new("memory").unwrap();
        KvStore::put(&mut kv, 1, b"key", b"value").unwrap();
        assert_eq!(KvStore::get(&kv, 1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_kv_engine_btree_mem() {
        let mut kv = KvEngine::new("btree").unwrap();
        KvStore::put(&mut kv, 1, b"key", b"value").unwrap();
        assert_eq!(KvStore::get(&kv, 1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_kv_engine_lsm() {
        let mut kv = KvEngine::new("lsm").unwrap();
        KvStore::put(&mut kv, 1, b"key", b"value").unwrap();
        assert_eq!(KvStore::get(&kv, 1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    #[test]
    fn test_kv_engine_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_kv_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        {
            let mut kv = KvEngine::open("btree", &temp_dir).unwrap();
            KvStore::put(&mut kv, 1, b"key1", b"value1").unwrap();
            KvStore::flush(&mut kv).unwrap();
        }
        
        {
            let kv = KvEngine::open("btree", &temp_dir).unwrap();
            assert_eq!(KvStore::get(&kv, 1, b"key1").unwrap(), Some(b"value1".to_vec()));
        }
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}