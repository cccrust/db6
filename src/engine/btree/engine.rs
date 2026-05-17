//! BTree 儲存引擎實作
//!
//! 基於 BTree 的磁碟持久化引擎，使用 `RwLock` 保證執行緒安全。
//! 支援交易功能：交易期間的修改先暫存在 `tx_buffer` 中，
//! commit 時才一次寫入 BTree 主結構。
//!
//! 交易機制：
//! - begin: 設定 in_transaction = true
//! - put/delete: 修改寫入 tx_buffer（不影響主 BTree）
//! - get/scan: 先查主 BTree，再用 tx_buffer 覆蓋/刪除
//! - commit: tx_buffer 內容逐一應用到主 BTree
//! - rollback: 直接清除 tx_buffer

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

use crate::engine::{EngineStats, StorageEngine};
use crate::error::{Error, Result};

use super::tree::BTree;

/// BTree 引擎
///
/// - `tree`: 主 BTree 資料結構
/// - `in_transaction`: 是否在交易中
/// - `tx_buffer`: 交易緩衝區，table_id → { key → Some(value) 或 None(刪除) }
/// - `path`: 持久化路徑
pub struct BTreeEngine {
    tree: RwLock<BTree>,
    in_transaction: RwLock<bool>,
    tx_buffer: RwLock<BTreeMap<u32, BTreeMap<Vec<u8>, Option<Vec<u8>>>>>,
    path: std::path::PathBuf,
}

impl BTreeEngine {
    /// 建立一個新的記憶體 BTree 引擎
    pub fn new() -> Self {
        BTreeEngine {
            tree: RwLock::new(BTree::new()),
            in_transaction: RwLock::new(false),
            tx_buffer: RwLock::new(BTreeMap::new()),
            path: std::path::PathBuf::new(),
        }
    }

    /// 從磁碟路徑開啟或建立 BTree 引擎
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let tree = BTree::load(path)?;

        let mut engine = BTreeEngine {
            tree: RwLock::new(tree),
            in_transaction: RwLock::new(false),
            tx_buffer: RwLock::new(BTreeMap::new()),
            path: path.to_path_buf(),
        };

        engine.tree.write().unwrap().set_path(path.to_path_buf());

        Ok(engine)
    }
}

impl Default for BTreeEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===== StorageEngine trait 實作 =====

impl StorageEngine for BTreeEngine {
    fn open(path: &Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    /// 回傳引擎類型名稱：`"btree"`
    fn engine_type(&self) -> &'static str {
        "btree"
    }

    /// 讀取一筆資料
    ///
    /// 交易中：先查交易緩衝區，找不到再查主 BTree
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if *self.in_transaction.read().unwrap() {
            if let Some(table_buf) = self.tx_buffer.read().unwrap().get(&table_id) {
                if let Some(value) = table_buf.get(key) {
                    return Ok(value.clone());
                }
            }
        }
        Ok(self.tree.read().unwrap().get(key))
    }

    /// 寫入一筆資料
    ///
    /// 交易中：寫入 tx_buffer（不影響主 BTree）
    /// 非交易：直接寫入主 BTree
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            let table = tx.entry(table_id).or_insert_with(BTreeMap::new);
            table.insert(key.to_vec(), Some(value.to_vec()));
        } else {
            self.tree.write().unwrap().put(key.to_vec(), value.to_vec());
        }
        Ok(())
    }

    /// 刪除一筆資料
    ///
    /// 交易中：在 tx_buffer 中標記為 None
    /// 非交易：直接從主 BTree 刪除
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            let table = tx.entry(table_id).or_insert_with(BTreeMap::new);
            table.insert(key.to_vec(), None);
        } else {
            self.tree.write().unwrap().delete(key);
        }
        Ok(())
    }

    /// 範圍掃描
    ///
    /// 交易中：先掃描主 BTree，再用 tx_buffer 的修改覆蓋結果
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut results = self.tree.read().unwrap().scan(start, end);

        if *self.in_transaction.read().unwrap() {
            if let Some(table_buf) = self.tx_buffer.read().unwrap().get(&table_id) {
                for (key, value) in table_buf.iter() {
                    match value {
                        Some(v) => {
                            results.retain(|(k, _)| k != key);
                            results.push((key.clone(), v.clone()));
                        }
                        None => {
                            results.retain(|(k, _)| k != key);
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    /// 批量寫入
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            let table = tx.entry(table_id).or_insert_with(BTreeMap::new);
            for (key, value) in pairs {
                table.insert(key, Some(value));
            }
        } else {
            for (key, value) in pairs {
                self.tree.write().unwrap().put(key, value);
            }
        }
        Ok(())
    }

    /// 範圍刪除
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()> {
        let keys: Vec<Vec<u8>> = self.tree.read().unwrap().scan(start, end)
            .into_iter()
            .map(|(k, _)| k)
            .collect();

        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            let table = tx.entry(table_id).or_insert_with(BTreeMap::new);
            for key in keys {
                table.insert(key, None);
            }
        } else {
            for key in keys {
                self.tree.write().unwrap().delete(&key);
            }
        }
        Ok(())
    }

    /// 將 BTree flush 到磁碟
    fn flush(&mut self) -> Result<()> {
        self.tree.write().unwrap().flush()
    }

    /// 同 flush
    fn sync(&mut self) -> Result<()> {
        self.tree.write().unwrap().flush()
    }

    /// 開始交易
    ///
    /// 交易不可嵌套：如果在交易中再次 begin 會回傳錯誤。
    fn begin_transaction(&mut self) -> Result<()> {
        if *self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("Transaction already active".into()));
        }
        *self.in_transaction.write().unwrap() = true;
        Ok(())
    }

    /// 提交交易
    ///
    /// 將 tx_buffer 中的所有修改應用到主 BTree，然後 flush。
    fn commit_transaction(&mut self) -> Result<()> {
        if !*self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("No active transaction".into()));
        }

        // 複製所有修改（避免死鎖：先釋放 tx_buffer 的讀鎖）
        let changes: Vec<_> = {
            let tx = self.tx_buffer.read().unwrap();
            let mut result = Vec::new();
            for (_, table_changes) in tx.iter() {
                for (key, value) in table_changes.iter() {
                    result.push((key.clone(), value.clone()));
                }
            }
            result
        };

        // 逐一應用到主 BTree
        for (key, value) in changes {
            match value {
                Some(v) => self.tree.write().unwrap().put(key, v),
                None => { self.tree.write().unwrap().delete(&key); }
            }
        }

        self.tx_buffer.write().unwrap().clear();
        *self.in_transaction.write().unwrap() = false;
        self.tree.write().unwrap().flush()?;
        Ok(())
    }

    /// 回滾交易
    ///
    /// 直接清除 tx_buffer，不影響主 BTree。
    fn rollback_transaction(&mut self) -> Result<()> {
        if !*self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("No active transaction".into()));
        }
        self.tx_buffer.write().unwrap().clear();
        *self.in_transaction.write().unwrap() = false;
        Ok(())
    }

    /// 是否有活躍交易
    fn has_transaction(&self) -> bool {
        *self.in_transaction.read().unwrap()
    }

    /// 取得統計資訊
    fn stats(&self) -> EngineStats {
        EngineStats {
            key_count: 0,
            size_bytes: 0,
            cache_hit_rate: None,
            in_transaction: *self.in_transaction.read().unwrap(),
            engine: "btree",
        }
    }
}

// ===== 單元測試 =====

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試基本的 put/get 操作
    #[test]
    fn test_btree_basic() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    /// 測試範圍掃描
    #[test]
    fn test_btree_scan() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.put(1, b"c", b"3").unwrap();

        let results = engine.scan(1, b"a", b"c").unwrap();
        assert!(results.len() >= 2);
    }

    /// 測試刪除操作
    #[test]
    fn test_btree_delete() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"key", b"value").unwrap();
        engine.delete(1, b"key").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), None);
    }

    /// 測試交易：begin → put → commit → 資料持久化
    #[test]
    fn test_btree_transaction() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"a", b"1").unwrap();

        engine.begin_transaction().unwrap();
        engine.put(1, b"b", b"2").unwrap();
        assert_eq!(engine.get(1, b"b").unwrap(), Some(b"2".to_vec()));

        engine.commit_transaction().unwrap();
        assert_eq!(engine.get(1, b"b").unwrap(), Some(b"2".to_vec()));
    }

    /// 測試交易回滾：begin → put → rollback → 資料不被寫入
    #[test]
    fn test_btree_transaction_rollback() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"a", b"1").unwrap();

        engine.begin_transaction().unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.rollback_transaction().unwrap();

        assert_eq!(engine.get(1, b"b").unwrap(), None);
    }

    /// 測試磁碟持久化
    #[test]
    fn test_btree_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_btree_persist_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 階段一：寫入資料並 flush
        {
            let mut engine = BTreeEngine::open(Path::new(&temp_dir)).unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.flush().unwrap();
        }

        // 階段二：重新開啟並驗證資料仍在
        {
            let engine = BTreeEngine::open(Path::new(&temp_dir)).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// BTree 引擎支援的能力：
// - CanOrderBy: BTree 有序，支援 ORDER BY
// - CanScan: 支援範圍掃描
// - CanBatch: 支援批量操作
// - CanFts: 支援全文搜尋
// - CanTransaction: 支援交易
// - CanGroupBy: 支援分組聚合
impl crate::engine::CanOrderBy for BTreeEngine {}
impl crate::engine::CanScan for BTreeEngine {}
impl crate::engine::CanBatch for BTreeEngine {}
impl crate::engine::CanFts for BTreeEngine {}
impl crate::engine::CanTransaction for BTreeEngine {}
impl crate::engine::CanGroupBy for BTreeEngine {}