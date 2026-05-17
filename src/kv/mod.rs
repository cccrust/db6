//! KV API 統一介面 — 工廠模式 + 統一的 KvStore 抽象層
//!
//! 提供 `KvEngine` 列舉作為工廠介面，讓使用者可以透過字串名稱
//! 選擇儲存引擎，而不需要直接與具體的引擎型別打交道。
//!
//! KvEngine 使用 `Arc<RwLock<...>>` 實現執行緒安全的共享存取。

use std::path::Path;
use std::sync::{Arc, RwLock};
use crate::error::{Error, Result};
use crate::engine::{EngineStats, StorageEngine};

/// KV Store 統一介面
///
/// 定義了所有 KV 操作的基本方法，與 StorageEngine 不同的是，
/// 此 trait 直接服務於上層的 SQL 執行器。
pub trait KvStore {
    /// 寫入一筆鍵值資料
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;
    /// 讀取一筆鍵值資料
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;
    /// 刪除一筆鍵值資料
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;
    /// 範圍掃描 [start, end)
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
    /// 批量寫入
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()>;
    /// 範圍刪除
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()>;
    /// 將資料 flush 到磁碟
    fn flush(&mut self) -> Result<()>;
    /// 取得引擎類型名稱
    fn engine_type(&self) -> &'static str;
}

/// KV Engine 列舉
///
/// 封裝了四種不同的儲存引擎實作，透過 `new()` 或 `open()` 工廠方法建立。
///
/// | 變體 | new() 名稱 | open() 支援 | 特性 |
/// |------|-----------|------------|------|
/// | Hash | "memory"/"hash" | 否 | O(1) 隨機存取 |
/// | BTreeMem | "btree"/"btree-mem" | 否 | 有序、範圍掃描 |
/// | BTree | 不直接支援 | "btree" | 磁碟持久化、交易 |
/// | Lsm | "lsm" | "lsm" | 高寫入吞吐量 |
pub enum KvEngine {
    /// HashMap 記憶體引擎（O(1)，不支援 ORDER BY）
    Hash(Arc<RwLock<crate::engine::HashMemoryEngine>>),
    /// BTreeMap 記憶體引擎（O(log n)，支援 ORDER BY）
    BTreeMem(Arc<RwLock<crate::engine::BTreeMemoryEngine>>),
    /// 磁碟 BTree 引擎（支援交易）
    BTree(Arc<RwLock<crate::engine::BTreeEngine>>),
    /// LSM-Tree 引擎（高寫入吞吐量）
    Lsm(Arc<RwLock<crate::engine::LsmEngine>>),
}

impl KvEngine {
    /// 建立記憶體模式引擎
    ///
    /// - `"memory"` 或 `"hash"`: HashMap 引擎
    /// - `"btree"` 或 `"btree-mem"`: BTreeMap 引擎
    /// - `"lsm"`: LSM 引擎（純記憶體模式）
    pub fn new(engine_type: &str) -> Result<Self> {
        match engine_type.to_lowercase().as_str() {
            "memory" | "hash" => Ok(KvEngine::Hash(Arc::new(RwLock::new(crate::engine::HashMemoryEngine::new())))),
            "btree" | "btree-mem" => Ok(KvEngine::BTreeMem(Arc::new(RwLock::new(crate::engine::BTreeMemoryEngine::new())))),
            "lsm" => Ok(KvEngine::Lsm(Arc::new(RwLock::new(crate::engine::LsmEngine::new())))),
            _ => Err(crate::error::Error::InvalidEngine(engine_type.to_string())),
        }
    }

    /// 建立磁碟持久化引擎
    ///
    /// - `"btree"`: 從路徑開啟 BTree 引擎
    /// - `"lsm"`: 從路徑開啟 LSM 引擎
    pub fn open(engine_type: &str, path: &Path) -> Result<Self> {
        match engine_type.to_lowercase().as_str() {
            "btree" => Ok(KvEngine::BTree(Arc::new(RwLock::new(crate::engine::BTreeEngine::open(path)?)))),
            "lsm" => Ok(KvEngine::Lsm(Arc::new(RwLock::new(crate::engine::LsmEngine::open(path)?)))),
            _ => Err(crate::error::Error::InvalidEngine(format!("{} does not support open(path)", engine_type))),
        }
    }
}

// ===== KvStore trait 實作：委派給具體引擎 =====
// 透過 match 委派給內部的具體引擎實作。

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

// ===== StorageEngine trait 實作 =====

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

// ===== 單元測試 =====

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試透過 KvEngine::new("memory") 建立引擎並操作
    #[test]
    fn test_kv_engine_new() {
        let mut kv = KvEngine::new("memory").unwrap();
        KvStore::put(&mut kv, 1, b"key", b"value").unwrap();
        assert_eq!(KvStore::get(&kv, 1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    /// 測試 BTree 記憶體引擎
    #[test]
    fn test_kv_engine_btree_mem() {
        let mut kv = KvEngine::new("btree").unwrap();
        KvStore::put(&mut kv, 1, b"key", b"value").unwrap();
        assert_eq!(KvStore::get(&kv, 1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    /// 測試 LSM 引擎
    #[test]
    fn test_kv_engine_lsm() {
        let mut kv = KvEngine::new("lsm").unwrap();
        KvStore::put(&mut kv, 1, b"key", b"value").unwrap();
        assert_eq!(KvStore::get(&kv, 1, b"key").unwrap(), Some(b"value".to_vec()));
    }

    /// 測試磁碟持久化
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