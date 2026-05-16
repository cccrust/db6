//! LsmEngine implementation

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

use crate::engine::{EngineStats, StorageEngine};
use crate::error::{Error, Result};

use super::memtable::MemTable;
use super::sstable::SSTable;
use super::wal::Wal;
use super::bloom::BloomFilter;

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

    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        
        let mut engine = Self::new();
        engine.path = Some(path.to_path_buf());
        
        // Try to load existing SSTables
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
        
        // Try to open WAL and recover
        let wal_path = path.join("wal.log");
        if wal_path.exists() {
            let wal = Wal::open(&wal_path)?;
            let recovered = wal.recover()?;
            
            if !recovered.is_empty() {
                // Load recovered data into memtable
                let mut memtable = engine.memtable.write().unwrap();
                for (k, v) in recovered {
                    memtable.put(k, v);
                }
            }
            
            // Clear WAL after recovery
            let _ = wal.clear();
            
            // Set WAL for new writes
            engine.wal = RwLock::new(Some(Wal::create(&wal_path)?));
        } else {
            // Create new WAL
            engine.wal = RwLock::new(Some(Wal::create(&wal_path)?));
        }
        
        Ok(engine)
    }

    fn flush_memtable(&mut self) -> Result<()> {
        let data = {
            let mem = self.memtable.read().unwrap();
            mem.all_data()
        };
        
        if data.is_empty() {
            return Ok(());
        }
        
        // Update bloom filter with all keys
        for (k, _) in &data {
            self.bloom.write().unwrap().insert(k);
        }
        
        // Write to WAL
        if let Ok(wal) = self.wal.read() {
            if let Some(w) = wal.as_ref() {
                for (k, v) in &data {
                    w.write(k, v)?;
                }
            }
        }
        
        // Write to SSTable if path is set
        if let Some(ref path) = self.path {
            let sstable_path = path.join(format!("{}.sst", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()));
            
            let sstable = SSTable::create(&sstable_path, data)?;
            self.sstables.write().unwrap().push(sstable);
        }
        
        // Clear memtable after flush
        self.memtable.write().unwrap().clear();
        
        Ok(())
    }

    fn sync_wal_only(&self) -> Result<()> {
        let data = {
            let mem = self.memtable.read().unwrap();
            mem.all_data()
        };
        
        // Update bloom filter with all keys
        for (k, _) in &data {
            self.bloom.write().unwrap().insert(k);
        }
        
        // Write to WAL only
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

    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Check transaction buffer first
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

        // Check memtable
        let mem = self.memtable.read().unwrap();
        if let Some(v) = mem.get(key) {
            if v.is_data() {
                return Ok(Some(v.get_data().unwrap().clone()));
            } else {
                return Ok(None);
            }
        }
        drop(mem);

        // Check bloom filter first
        if !self.bloom.read().unwrap().might_contain(key) {
            return Ok(None);
        }

        // Check SSTables
        let sstables = self.sstables.read().unwrap();
        for ss in sstables.iter().rev() {
            if let Some(v) = ss.get(key) {
                return Ok(Some(v));
            }
        }

        Ok(None)
    }

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

    fn scan(&self, _table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut results = self.memtable.read().unwrap().scan(start, end);
        
        // Apply transaction buffer
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

    fn flush(&mut self) -> Result<()> {
        self.flush_memtable()
    }

    fn sync(&mut self) -> Result<()> {
        self.flush_memtable()
    }

    fn begin_transaction(&mut self) -> Result<()> {
        if *self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("Transaction already active".into()));
        }
        *self.in_transaction.write().unwrap() = true;
        Ok(())
    }

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

    fn rollback_transaction(&mut self) -> Result<()> {
        if !*self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("No active transaction".into()));
        }
        self.tx_buffer.write().unwrap().take();
        *self.in_transaction.write().unwrap() = false;
        Ok(())
    }

    fn has_transaction(&self) -> bool {
        *self.in_transaction.read().unwrap()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsm_basic() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    #[test]
    fn test_lsm_scan() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.put(1, b"c", b"3").unwrap();

        let results = engine.scan(1, b"a", b"c").unwrap();
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_lsm_delete() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"key", b"value").unwrap();
        engine.delete(1, b"key").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), None);
    }

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

    #[test]
    fn test_lsm_transaction_rollback() {
        let mut engine = LsmEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        
        engine.begin_transaction().unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.rollback_transaction().unwrap();
        
        assert_eq!(engine.get(1, b"b").unwrap(), None);
    }

    #[test]
    fn test_lsm_multi_table_unsupported() {
        let mut engine = LsmEngine::new();
        let result = engine.put(2, b"key", b"value");
        assert!(result.is_err());
    }

    #[test]
    fn test_lsm_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_lsm_persist_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        // Write data and flush
        {
            let mut engine = LsmEngine::open(&temp_dir).unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.flush().unwrap();
        }
        
        // Reopen and verify
        {
            let engine = LsmEngine::open(&temp_dir).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_lsm_wal_recovery() {
        let temp_dir = std::env::temp_dir().join("db6_lsm_wal_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        // Write data with transaction (writes to WAL)
        {
            let mut engine = LsmEngine::open(&temp_dir).unwrap();
            engine.begin_transaction().unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.commit_transaction().unwrap();
        }
        
        // Reopen and verify - WAL should be recovered
        {
            let engine = LsmEngine::open(&temp_dir).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// Capability implementations for LsmEngine
impl crate::engine::CanScan for LsmEngine {}
impl crate::engine::CanBatch for LsmEngine {}
impl crate::engine::CanTransaction for LsmEngine {}