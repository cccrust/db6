//! Hash 記憶體引擎 — 類似 Redis，快速 KV 操作
//!
//! 使用 `HashMap` 實作，所有操作都是 O(1) 時間複雜度。
//! **不支援** ORDER BY 或範圍掃描，因為 HashMap 不保證鍵的順序。
//!
//! 適合需要快速隨機存取、不需要排序的場景。

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use crate::engine::{EngineStats, StorageEngine};
use crate::error::Result;

/// Hash 記憶體引擎
///
/// 內部結構：`HashMap<table_id, HashMap<key, value>>`
/// - 外層 HashMap 以 table_id 區分不同的表空間
/// - 內層 HashMap 儲存實際的鍵值對
pub struct HashMemoryEngine {
    /// 多層表格：table_id → HashMap<Vec<u8>, Vec<u8>>
    tables: HashMap<u32, HashMap<Vec<u8>, Vec<u8>>>,
    /// 可選的磁碟路徑（用於持久化）
    path: Option<std::path::PathBuf>,
}

impl HashMemoryEngine {
    /// 建立一個新的記憶體引擎，無持久化
    pub fn new() -> Self {
        HashMemoryEngine {
            tables: HashMap::new(),
            path: None,
        }
    }

    /// 從磁碟載入或建立持久化引擎
    ///
    /// 資料儲存在 `path/hashtable.dat` 中，使用 bincode 序列化。
    pub fn open(path: &Path) -> Result<Self> {
        // 確保目錄存在
        std::fs::create_dir_all(path)?;

        let data_path = path.join("hashtable.dat");

        // 嘗試從檔案載入已存在的資料
        let tables = if data_path.exists() {
            let mut file = File::open(&data_path)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;

            // 如果反序列化失敗（格式變更等），使用空 HashMap
            match bincode::deserialize(&contents) {
                Ok(t) => t,
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };

        Ok(HashMemoryEngine {
            tables,
            path: Some(path.to_path_buf()),
        })
    }

    /// 將資料寫回磁碟（持久化）
    ///
    /// 使用 atomic write 模式：
    /// 1. 先寫入暫存檔 `hashtable.tmp`
    /// 2. 再透過 `rename` 原子操作取代原檔案
    /// 防止寫入中途程式崩潰導致資料損毀
    fn save(&self) -> Result<()> {
        if let Some(ref path) = self.path {
            let temp_path = path.join("hashtable.tmp");
            let data_path = path.join("hashtable.dat");

            // 步驟1：寫入暫存檔
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)?;

            let data = bincode::serialize(&self.tables)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("bincode: {:?}", e)))?;

            file.write_all(&data)?;
            drop(file);

            // 步驟2：原子交換檔案
            std::fs::rename(&temp_path, &data_path)?;
        }
        Ok(())
    }

    /// 取得或建立指定 table_id 對應的 HashMap
    fn table_mut(&mut self, table_id: u32) -> &mut HashMap<Vec<u8>, Vec<u8>> {
        self.tables.entry(table_id).or_insert_with(HashMap::new)
    }
}

impl Default for HashMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===== StorageEngine trait 實作 =====

impl StorageEngine for HashMemoryEngine {
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    /// 回傳引擎類型名稱：`"memory-hash"`
    fn engine_type(&self) -> &'static str {
        "memory-hash"
    }

    /// 讀取一筆資料，O(1) 時間複雜度
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.tables.get(&table_id).and_then(|t| t.get(key).cloned()))
    }

    /// 寫入一筆資料，O(1) 時間複雜度
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        self.table_mut(table_id).insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    /// 刪除一筆資料
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        self.table_mut(table_id).remove(key);
        Ok(())
    }

    /// 範圍掃描：由於 HashMap 不支援範圍掃描，此方法傳回該 table 的所有資料
    ///
    /// 注意：`start` 和 `end` 參數會被忽略！
    fn scan(&self, table_id: u32, _start: &[u8], _end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.tables.get(&table_id)
            .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default())
    }

    /// 批量寫入多筆資料
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        let table = self.table_mut(table_id);
        for (key, value) in pairs {
            table.insert(key, value);
        }
        Ok(())
    }

    /// 範圍刪除：HashMap 版本會刪除整個 table 的所有資料
    fn range_delete(&mut self, table_id: u32, _start: &[u8], _end: &[u8]) -> Result<()> {
        self.tables.remove(&table_id);
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

    /// 開始交易：Hash 引擎不支援交易
    fn begin_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("HashMemoryEngine 不支援交易".into()))
    }

    /// 提交交易：Hash 引擎不支援交易
    fn commit_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("HashMemoryEngine 不支援交易".into()))
    }

    /// 回滾交易：Hash 引擎不支援交易
    fn rollback_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("HashMemoryEngine 不支援交易".into()))
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
            engine: "memory-hash",
        }
    }
}

// ===== 單元測試 =====

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試基本的 put/get 操作
    #[test]
    fn test_hash_basic() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    /// 測試刪除操作
    #[test]
    fn test_hash_delete() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"k", b"v").unwrap();
        engine.delete(1, b"k").unwrap();
        assert_eq!(engine.get(1, b"k").unwrap(), None);
    }

    /// 測試多 table 隔離：同一個鍵在不同 table 中有不同值
    #[test]
    fn test_hash_multi_table() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"key", b"table1").unwrap();
        engine.put(2, b"key", b"table2").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), Some(b"table1".to_vec()));
        assert_eq!(engine.get(2, b"key").unwrap(), Some(b"table2".to_vec()));
    }

    /// 測試掃描（傳回所有鍵值對）
    #[test]
    fn test_hash_scan_all() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();

        let results = engine.scan(1, b"", b"").unwrap();
        assert_eq!(results.len(), 2);
    }

    /// 測試批量寫入
    #[test]
    fn test_hash_batch_put() {
        let mut engine = HashMemoryEngine::new();
        let pairs = vec![
            (b"k1".to_vec(), b"v1".to_vec()),
            (b"k2".to_vec(), b"v2".to_vec()),
        ];
        engine.batch_put(1, pairs).unwrap();
        assert_eq!(engine.get(1, b"k1").unwrap(), Some(b"v1".to_vec()));
        assert_eq!(engine.get(1, b"k2").unwrap(), Some(b"v2".to_vec()));
    }

    /// 測試磁碟持久化：寫入 → flush → 重新開啟 → 驗證資料仍在
    #[test]
    fn test_hash_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_hash_persist_test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        // 階段一：寫入資料並 flush
        {
            let mut engine = HashMemoryEngine::open(Path::new(&temp_dir)).unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.flush().unwrap();
        }

        // 階段二：重新開啟並驗證資料仍在
        {
            let engine = HashMemoryEngine::open(Path::new(&temp_dir)).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }

        // 清理測試目錄
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// Hash 引擎的能力標記：支援批次操作與全文搜尋
impl crate::engine::CanBatch for HashMemoryEngine {}
impl crate::engine::CanFts for HashMemoryEngine {}