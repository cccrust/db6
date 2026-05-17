//! BTree 記憶體引擎 — 類似 SQLite，支援 SQL 操作
//!
//! 使用 `BTreeMap` 實作，所有操作是 O(log n) 時間複雜度。
//! BTreeMap 的鍵是有序的，因此支援 ORDER BY 和範圍掃描。
//!
//! 適合需要排序、範圍查詢的場景。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use crate::engine::{EngineStats, StorageEngine};
use crate::error::Result;

/// BTree 記憶體引擎
///
/// 內部結構：`HashMap<table_id, BTreeMap<key, value>>`
/// - 外層 HashMap 以 table_id 區分不同的表空間
/// - 內層 BTreeMap 以有序方式儲存鍵值對，支援範圍掃描
pub struct BTreeMemoryEngine {
    /// 多層表格：table_id → BTreeMap<Vec<u8>, Vec<u8>>
    tables: std::collections::HashMap<u32, BTreeMap<Vec<u8>, Vec<u8>>>,
    /// 可選的磁碟路徑（用於持久化）
    path: Option<std::path::PathBuf>,
}

impl BTreeMemoryEngine {
    /// 建立一個新的記憶體引擎，無持久化
    pub fn new() -> Self {
        BTreeMemoryEngine {
            tables: std::collections::HashMap::new(),
            path: None,
        }
    }

    /// 從磁碟載入或建立持久化引擎
    ///
    /// 資料儲存在 `path/btree.dat` 中，使用 bincode 序列化。
    pub fn open(path: &Path) -> Result<Self> {
        // 確保目錄存在
        std::fs::create_dir_all(path)?;

        let data_path = path.join("btree.dat");

        // 嘗試從檔案載入已存在的資料
        let tables = if data_path.exists() {
            let mut file = File::open(&data_path)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;

            match bincode::deserialize(&contents) {
                Ok(t) => t,
                Err(_) => std::collections::HashMap::new(),
            }
        } else {
            std::collections::HashMap::new()
        };

        Ok(BTreeMemoryEngine {
            tables,
            path: Some(path.to_path_buf()),
        })
    }

    /// 將資料寫回磁碟（持久化）
    ///
    /// 使用 atomic write 模式防止資料損毀：
    /// 1. 先寫入暫存檔 `btree.tmp`
    /// 2. 再透過 `rename` 原子操作取代原檔案
    fn save(&self) -> Result<()> {
        if let Some(ref path) = self.path {
            let temp_path = path.join("btree.tmp");
            let data_path = path.join("btree.dat");

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)?;

            let data = bincode::serialize(&self.tables)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("bincode: {:?}", e)))?;

            file.write_all(&data)?;
            drop(file);

            // 原子交換
            std::fs::rename(&temp_path, &data_path)?;
        }
        Ok(())
    }

    /// 取得指定 table_id 的 BTreeMap 參考（唯讀）
    fn table(&self, table_id: u32) -> Option<&BTreeMap<Vec<u8>, Vec<u8>>> {
        self.tables.get(&table_id)
    }

    /// 取得或建立指定 table_id 的 BTreeMap 可變參考
    fn table_mut(&mut self, table_id: u32) -> &mut BTreeMap<Vec<u8>, Vec<u8>> {
        if !self.tables.contains_key(&table_id) {
            self.tables.insert(table_id, BTreeMap::new());
        }
        self.tables.get_mut(&table_id).unwrap()
    }
}

impl Default for BTreeMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===== StorageEngine trait 實作 =====

impl StorageEngine for BTreeMemoryEngine {
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    /// 回傳引擎類型名稱：`"memory-btree"`
    fn engine_type(&self) -> &'static str {
        "memory-btree"
    }

    /// 讀取一筆資料，O(log n) 時間複雜度
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.table(table_id).and_then(|t| t.get(key).cloned()))
    }

    /// 寫入一筆資料，O(log n) 時間複雜度
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        self.table_mut(table_id).insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    /// 刪除一筆資料
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        self.table_mut(table_id).remove(key);
        Ok(())
    }

    /// 範圍掃描 [start, end)
    ///
    /// BTree 的 key 是有序的，因此可以使用 `range()` 方法
    /// 取得指定範圍內的鍵值對。空字串代表無限邊界。
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        use std::collections::Bound;
        let table = match self.table(table_id) {
            Some(t) => t,
            None => return Ok(Vec::new()),
        };

        let start_bound = if start.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Included(start.to_vec())
        };
        let end_bound = if end.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(end.to_vec())
        };

        let iter = table.range((start_bound, end_bound));
        Ok(iter.map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// 批量寫入多筆資料
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        let table = self.table_mut(table_id);
        for (key, value) in pairs {
            table.insert(key, value);
        }
        Ok(())
    }

    /// 範圍刪除 [start, end)
    ///
    /// 先掃描出範圍內的鍵，再逐一刪除。
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()> {
        use std::collections::Bound;
        let table = self.table_mut(table_id);

        let start_bound = if start.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Included(start.to_vec())
        };
        let end_bound = if end.is_empty() {
            Bound::Unbounded
        } else {
            Bound::Excluded(end.to_vec())
        };

        // 先收集要刪除的所有鍵（避免疊代時修改集合）
        let keys: Vec<Vec<u8>> = table.range((start_bound, end_bound)).map(|(k, _)| k.clone()).collect();
        for key in keys {
            table.remove(&key);
        }
        Ok(())
    }

    /// 將記憶體資料 flush 到磁碟
    fn flush(&mut self) -> Result<()> {
        self.save()
    }

    /// 同 flush（記憶體引擎無需 fsync）
    fn sync(&mut self) -> Result<()> {
        self.save()
    }

    /// 開始交易：記憶體引擎不支援交易
    fn begin_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("Memory engine 不支援交易".into()))
    }

    /// 提交交易：記憶體引擎不支援交易
    fn commit_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("Memory engine 不支援交易".into()))
    }

    /// 回滾交易：記憶體引擎不支援交易
    fn rollback_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("Memory engine 不支援交易".into()))
    }

    /// 是否有活躍交易：永遠回傳 false
    fn has_transaction(&self) -> bool {
        false
    }

    /// 取得引擎統計資訊
    fn stats(&self) -> EngineStats {
        let total_keys: u64 = self.tables.values().map(|t| t.len() as u64).sum();
        EngineStats {
            key_count: total_keys,
            size_bytes: 0,
            cache_hit_rate: None,
            in_transaction: false,
            engine: "memory-btree",
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
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    /// 測試範圍掃描 [a, c) 應傳回 a, b 兩個鍵
    #[test]
    fn test_btree_scan() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.put(1, b"c", b"3").unwrap();

        let results = engine.scan(1, b"a", b"c").unwrap();
        assert_eq!(results.len(), 2); // a and b
    }

    /// 測試刪除操作
    #[test]
    fn test_btree_delete() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"k", b"v").unwrap();
        engine.delete(1, b"k").unwrap();
        assert_eq!(engine.get(1, b"k").unwrap(), None);
    }

    /// 測試多 table 隔離
    #[test]
    fn test_btree_multi_table() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"key", b"table1").unwrap();
        engine.put(2, b"key", b"table2").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), Some(b"table1".to_vec()));
        assert_eq!(engine.get(2, b"key").unwrap(), Some(b"table2".to_vec()));
    }

    /// 測試掃描回傳結果的順序：BTreeMap 保證按鍵遞增排序
    #[test]
    fn test_btree_order() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"c", b"3").unwrap();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();

        let results = engine.scan(1, b"", b"").unwrap();
        let keys: Vec<_> = results.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(keys, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    /// 測試磁碟持久化
    #[test]
    fn test_btree_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_btree_mem_persist_test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        // 階段一：寫入資料並 flush
        {
            let mut engine = BTreeMemoryEngine::open(Path::new(&temp_dir)).unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.flush().unwrap();
        }

        // 階段二：重新開啟並驗證資料仍在
        {
            let engine = BTreeMemoryEngine::open(Path::new(&temp_dir)).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// BTree 記憶體引擎支援的能力：
// - CanOrderBy: 支援 ORDER BY（BTreeMap 有序）
// - CanScan: 支援範圍掃描
// - CanBatch: 支援批量操作
// - CanFts: 支援全文搜尋
// - CanGroupBy: 支援分組聚合
impl crate::engine::CanOrderBy for BTreeMemoryEngine {}
impl crate::engine::CanScan for BTreeMemoryEngine {}
impl crate::engine::CanBatch for BTreeMemoryEngine {}
impl crate::engine::CanFts for BTreeMemoryEngine {}
impl crate::engine::CanGroupBy for BTreeMemoryEngine {}