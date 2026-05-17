//! LSM-Tree 儲存引擎實作
//!
//! LSM-Tree (Log-Structured Merge-Tree) 是一種針對高寫入吞吐量優化的資料結構。
//! 核心概念：寫入先進入記憶體 (MemTable)，累積到一定大小後合併寫入磁碟 (SSTable)。
//!
//! 寫入路徑：
//!   put → MemTable → (flush) → SSTable
//! 讀取路徑：
//!   get → MemTable → Bloom Filter → SSTables (由新到舊)
//!
//! 限制：目前只支援 `table_id = 1`（單一表空間）。

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

use crate::engine::{EngineStats, StorageEngine};
use crate::error::{Error, Result};

use super::memtable::MemTable;
use super::sstable::SSTable;
use super::wal::Wal;
use super::bloom::BloomFilter;

/// LSM-Tree 引擎主結構
///
/// - `path`: 持久化路徑
/// - `memtable`: 記憶體寫入緩衝區
/// - `sstables`: 磁碟上的 SSTable 集合（由舊到新）
/// - `bloom`: 布隆過濾器，快速排除不存在的鍵
/// - `wal`: 預寫式日誌，確保資料不遺失
/// - `in_transaction`: 交易狀態
/// - `tx_buffer`: 交易緩衝區
pub struct LsmEngine {
    path: Option<std::path::PathBuf>,
    memtable: RwLock<MemTable>,
    sstables: RwLock<Vec<SSTable>>,
    bloom: RwLock<BloomFilter>,
    wal: RwLock<Option<Wal>>,
    in_transaction: RwLock<bool>,
    tx_buffer: RwLock<Option<BTreeMap<Vec<u8>, Option<Vec<u8>>>>>,
}

impl LsmEngine {
    /// 建立一個新的記憶體 LSM 引擎（無持久化）
    pub fn new() -> Self {
        LsmEngine {
            path: None,
            memtable: RwLock::new(MemTable::new()),
            sstables: RwLock::new(Vec::new()),
            bloom: RwLock::new(BloomFilter::new(1024)),
            wal: RwLock::new(None),
            in_transaction: RwLock::new(false),
            tx_buffer: RwLock::new(None),
        }
    }

    /// 從磁碟路徑開啟或建立 LSM 引擎
    ///
    /// 啟動流程：
    /// 1. 掃描目錄中所有的 `.sst` 檔案，載入現有 SSTable
    /// 2. 嘗試開啟 `wal.log` 並復原未 flush 的資料
    /// 3. 清空 WAL 並建立新的日誌
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let mut engine = Self::new();
        engine.path = Some(path.to_path_buf());

        // 步驟1：載入現有的 SSTable
        if path.exists() {
            if let Ok(entries) = std::fs::read_dir(path) {
                let mut sstables = engine.sstables.write().unwrap();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "sst") {
                        if let Ok(ss) = SSTable::open(&path) {
                            sstables.push(ss);
                        }
                    }
                }
            }
        }

        // 步驟2：從 WAL 復原資料
        let wal_path = path.join("wal.log");
        if wal_path.exists() {
            let wal = Wal::open(&wal_path)?;
            let recovered = wal.recover()?;

            if !recovered.is_empty() {
                let mut memtable = engine.memtable.write().unwrap();
                for (k, v) in recovered {
                    memtable.put(k, v);
                }
            }

            // 清空 WAL（資料已復原到 MemTable）
            let _ = wal.clear();

            // 建立新的 WAL
            engine.wal = RwLock::new(Some(Wal::create(&wal_path)?));
        } else {
            // 建立新的 WAL
            engine.wal = RwLock::new(Some(Wal::create(&wal_path)?));
        }

        Ok(engine)
    }

    /// 將 MemTable flush 到磁碟
    ///
    /// 流程：
    /// 1. 讀取 MemTable 中所有資料
    /// 2. 更新 Bloom Filter
    /// 3. 寫入 WAL
    /// 4. 建立新的 SSTable 檔案
    /// 5. 清空 MemTable
    fn flush_memtable(&mut self) -> Result<()> {
        let data = {
            let mem = self.memtable.read().unwrap();
            mem.all_data()
        };

        if data.is_empty() {
            return Ok(());
        }

        // 更新 Bloom Filter
        for (k, _) in &data {
            self.bloom.write().unwrap().insert(k);
        }

        // 寫入 WAL
        if let Ok(wal) = self.wal.read() {
            if let Some(w) = wal.as_ref() {
                for (k, v) in &data {
                    w.write(k, v)?;
                }
            }
        }

        // 建立 SSTable（僅在有設定路徑時）
        if let Some(ref path) = self.path {
            let sstable_path = path.join(format!("{}.sst", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()));

            let sstable = SSTable::create(&sstable_path, data)?;
            self.sstables.write().unwrap().push(sstable);
        }

        // 清空 MemTable
        self.memtable.write().unwrap().clear();

        Ok(())
    }

    /// 僅寫入 WAL（不產生 SSTable）
    ///
    /// 用於交易 commit 時確保資料持久化，
    /// 但暫不觸發 MemTable → SSTable 的合併。
    fn sync_wal_only(&self) -> Result<()> {
        let data = {
            let mem = self.memtable.read().unwrap();
            mem.all_data()
        };

        for (k, _) in &data {
            self.bloom.write().unwrap().insert(k);
        }

        if let Ok(wal) = self.wal.read() {
            if let Some(w) = wal.as_ref() {
                for (k, v) in data {
                    w.write(&k, &v)?;
                }
            }
        }

        Ok(())
    }
}

impl Default for LsmEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ===== StorageEngine trait 實作 =====

impl StorageEngine for LsmEngine {
    fn open(path: &Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    fn engine_type(&self) -> &'static str {
        "lsm"
    }

    /// 讀取一筆資料
    ///
    /// 查詢路徑（由快到慢）：
    /// 1. 交易緩衝區
    /// 2. MemTable（記憶體）
    /// 3. Bloom Filter（快速排除）
    /// 4. SSTable（磁碟，由新到舊）
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // 步驟1：查交易緩衝區
        if let Ok(tx) = self.tx_buffer.read() {
            if let Some(buffer) = tx.as_ref() {
                if let Some(value) = buffer.get(key) {
                    return match value {
                        Some(v) => Ok(Some(v.clone())),
                        None => Ok(None),
                    };
                }
            }
        }

        // 步驟2：查 MemTable
        let mem = self.memtable.read().unwrap();
        if let Some(v) = mem.get(key) {
            if v.is_data() {
                return Ok(Some(v.get_data().unwrap().clone()));
            } else {
                return Ok(None);
            }
        }
        drop(mem);

        // 步驟3：Bloom Filter 快速排除
        if !self.bloom.read().unwrap().might_contain(key) {
            return Ok(None);
        }

        // 步驟4：從 SSTable 由新到舊查詢
        let sstables = self.sstables.read().unwrap();
        for ss in sstables.iter().rev() {
            if let Some(v) = ss.get(key) {
                return Ok(Some(v));
            }
        }

        Ok(None)
    }

    /// 寫入一筆資料（與 BTree 引擎不同，LSM 的所有 table_id 都對應 table_id=1）
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        if table_id != 1 {
            return Err(Error::NotSupported("LSM engine only supports table_id=1".into()));
        }
        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            if tx.is_none() {
                *tx = Some(BTreeMap::new());
            }
            if let Some(ref mut buffer) = *tx {
                buffer.insert(key.to_vec(), Some(value.to_vec()));
            }
        } else {
            self.memtable.write().unwrap().put(key.to_vec(), value.to_vec());
        }
        Ok(())
    }

    /// 刪除一筆資料（使用 tombstone 標記）
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        if table_id != 1 {
            return Err(Error::NotSupported("LSM engine only supports table_id=1".into()));
        }
        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            if tx.is_none() {
                *tx = Some(BTreeMap::new());
            }
            if let Some(ref mut buffer) = *tx {
                buffer.insert(key.to_vec(), None);
            }
        } else {
            self.memtable.write().unwrap().delete(key.to_vec());
        }
        Ok(())
    }

    /// 範圍掃描（僅掃描 MemTable，不掃描 SSTable）
    fn scan(&self, _table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut results = self.memtable.read().unwrap().scan(start, end);

        // 套用交易緩衝區的修改
        if let Ok(tx) = self.tx_buffer.read() {
            if let Some(buffer) = tx.as_ref() {
                for (key, value) in buffer.iter() {
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
        if table_id != 1 {
            return Err(Error::NotSupported("LSM engine only supports table_id=1".into()));
        }
        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            if tx.is_none() {
                *tx = Some(BTreeMap::new());
            }
            if let Some(ref mut buffer) = *tx {
                for (key, value) in pairs {
                    buffer.insert(key, Some(value));
                }
            }
        } else {
            let mut mem = self.memtable.write().unwrap();
            for (key, value) in pairs {
                mem.put(key, value);
            }
        }
        Ok(())
    }

    /// 範圍刪除
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()> {
        if table_id != 1 {
            return Err(Error::NotSupported("LSM engine only supports table_id=1".into()));
        }
        let keys: Vec<Vec<u8>> = self.memtable.read().unwrap().scan(start, end)
            .into_iter()
            .map(|(k, _)| k)
            .collect();

        if *self.in_transaction.read().unwrap() {
            let mut tx = self.tx_buffer.write().unwrap();
            if tx.is_none() {
                *tx = Some(BTreeMap::new());
            }
            if let Some(ref mut buffer) = *tx {
                for key in keys {
                    buffer.insert(key, None);
                }
            }
        } else {
            let mut mem = self.memtable.write().unwrap();
            for key in keys {
                mem.delete(key);
            }
        }
        Ok(())
    }

    /// 將 MemTable flush 到 SSTable
    fn flush(&mut self) -> Result<()> {
        self.flush_memtable()
    }

    /// 同 flush
    fn sync(&mut self) -> Result<()> {
        self.flush_memtable()
    }

    /// 開始交易
    fn begin_transaction(&mut self) -> Result<()> {
        if *self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("Transaction already active".into()));
        }
        *self.in_transaction.write().unwrap() = true;
        Ok(())
    }

    /// 提交交易：將緩衝區寫入 MemTable 並同步 WAL
    fn commit_transaction(&mut self) -> Result<()> {
        if !*self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("No active transaction".into()));
        }

        if let Some(buffer) = self.tx_buffer.write().unwrap().take() {
            let mut mem = self.memtable.write().unwrap();
            for (key, value) in buffer.into_iter() {
                match value {
                    Some(v) => mem.put(key, v),
                    None => mem.delete(key),
                }
            }
        }

        *self.in_transaction.write().unwrap() = false;
        self.sync_wal_only()
    }

    /// 回滾交易：直接捨棄緩衝區
    fn rollback_transaction(&mut self) -> Result<()> {
        if !*self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("No active transaction".into()));
        }
        self.tx_buffer.write().unwrap().take();
        *self.in_transaction.write().unwrap() = false;
        Ok(())
    }

    /// 是否有活躍交易
    fn has_transaction(&self) -> bool {
        *self.in_transaction.read().unwrap()
    }

    /// 取得統計資訊（MemTable + SSTable 的鍵數量）
    fn stats(&self) -> EngineStats {
        let mem_keys = self.memtable.read().unwrap().len() as u64;
        let sstable_keys: u64 = self.sstables.read().unwrap().iter().map(|s| s.len()).sum();
        EngineStats {
            key_count: mem_keys + sstable_keys,
            size_bytes: 0,
            cache_hit_rate: None,
            in_transaction: *self.in_transaction.read().unwrap(),
            engine: "lsm",
        }
    }
}

// ===== 單元測試 =====

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試基本的 put/get 操作
    #[test]
    fn test_lsm_basic() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    /// 測試範圍掃描
    #[test]
    fn test_lsm_scan() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.put(1, b"c", b"3").unwrap();

        let results = engine.scan(1, b"a", b"c").unwrap();
        assert!(results.len() >= 2);
    }

    /// 測試刪除操作
    #[test]
    fn test_lsm_delete() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"key", b"value").unwrap();
        engine.delete(1, b"key").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), None);
    }

    /// 測試交易：begin → put → commit
    #[test]
    fn test_lsm_transaction() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"a", b"1").unwrap();

        engine.begin_transaction().unwrap();
        engine.put(1, b"b", b"2").unwrap();
        assert_eq!(engine.get(1, b"b").unwrap(), Some(b"2".to_vec()));

        engine.commit_transaction().unwrap();
        assert_eq!(engine.get(1, b"b").unwrap(), Some(b"2".to_vec()));
    }

    /// 測試交易回滾
    #[test]
    fn test_lsm_transaction_rollback() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"a", b"1").unwrap();

        engine.begin_transaction().unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.rollback_transaction().unwrap();

        assert_eq!(engine.get(1, b"b").unwrap(), None);
    }

    /// 測試多 table 不支援
    #[test]
    fn test_lsm_multi_table_unsupported() {
        let mut engine = LsmEngine::new();
        let result = engine.put(2, b"key", b"value");
        assert!(result.is_err());
    }

    /// 測試磁碟持久化
    #[test]
    fn test_lsm_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_lsm_persist_test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        {
            let mut engine = LsmEngine::open(&temp_dir).unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.flush().unwrap();
        }

        {
            let engine = LsmEngine::open(&temp_dir).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// 測試 WAL 復原：寫入交易資料 → 重新開啟 → 資料應從 WAL 復原
    #[test]
    fn test_lsm_wal_recovery() {
        let temp_dir = std::env::temp_dir().join("db6_lsm_wal_test");
        let _ = std::fs::remove_dir_all(&temp_dir);

        {
            let mut engine = LsmEngine::open(&temp_dir).unwrap();
            engine.begin_transaction().unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.commit_transaction().unwrap();
        }

        {
            let engine = LsmEngine::open(&temp_dir).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// LSM 引擎支援的能力：掃描、批次操作、交易
impl crate::engine::CanScan for LsmEngine {}
impl crate::engine::CanBatch for LsmEngine {}
impl crate::engine::CanTransaction for LsmEngine {}