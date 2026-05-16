//! Hash Memory Engine - Redis-like, fast KV operations
//! 
//! Uses HashMap for O(1) operations, does NOT support ORDER BY or range scans.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use crate::engine::{EngineStats, StorageEngine};
use crate::error::Result;

pub struct HashMemoryEngine {
    tables: HashMap<u32, HashMap<Vec<u8>, Vec<u8>>>,
    path: Option<std::path::PathBuf>,
}

impl HashMemoryEngine {
    pub fn new() -> Self {
        HashMemoryEngine {
            tables: HashMap::new(),
            path: None,
        }
    }

    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        
        let data_path = path.join("hashtable.dat");
        
        let tables = if data_path.exists() {
            let mut file = File::open(&data_path)?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            
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

    fn save(&self) -> Result<()> {
        if let Some(ref path) = self.path {
            let temp_path = path.join("hashtable.tmp");
            let data_path = path.join("hashtable.dat");
            
            // Write to temp file
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)?;
            
            let data = bincode::serialize(&self.tables)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("bincode: {:?}", e)))?;
            
            file.write_all(&data)?;
            drop(file);
            
            // Atomic rename
            std::fs::rename(&temp_path, &data_path)?;
        }
        Ok(())
    }

    fn table_mut(&mut self, table_id: u32) -> &mut HashMap<Vec<u8>, Vec<u8>> {
        self.tables.entry(table_id).or_insert_with(HashMap::new)
    }
}

impl Default for HashMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageEngine for HashMemoryEngine {
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    fn engine_type(&self) -> &'static str {
        "memory-hash"
    }

    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.tables.get(&table_id).and_then(|t| t.get(key).cloned()))
    }

    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        self.table_mut(table_id).insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        self.table_mut(table_id).remove(key);
        Ok(())
    }

    fn scan(&self, table_id: u32, _start: &[u8], _end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        // HashMap 不支援範圍掃描，返回所有 key
        Ok(self.tables.get(&table_id)
            .map(|t| t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default())
    }

    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        let table = self.table_mut(table_id);
        for (key, value) in pairs {
            table.insert(key, value);
        }
        Ok(())
    }

    fn range_delete(&mut self, table_id: u32, _start: &[u8], _end: &[u8]) -> Result<()> {
        // HashMap 不支援範圍刪除，刪除所有
        self.tables.remove(&table_id);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.save()
    }

    fn sync(&mut self) -> Result<()> {
        self.save()
    }

    fn begin_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("HashMemoryEngine 不支援交易".into()))
    }

    fn commit_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("HashMemoryEngine 不支援交易".into()))
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("HashMemoryEngine 不支援交易".into()))
    }

    fn has_transaction(&self) -> bool {
        false
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_basic() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    #[test]
    fn test_hash_delete() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"k", b"v").unwrap();
        engine.delete(1, b"k").unwrap();
        assert_eq!(engine.get(1, b"k").unwrap(), None);
    }

    #[test]
    fn test_hash_multi_table() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"key", b"table1").unwrap();
        engine.put(2, b"key", b"table2").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), Some(b"table1".to_vec()));
        assert_eq!(engine.get(2, b"key").unwrap(), Some(b"table2".to_vec()));
    }

    #[test]
    fn test_hash_scan_all() {
        let mut engine = HashMemoryEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        
        let results = engine.scan(1, b"", b"").unwrap();
        assert_eq!(results.len(), 2);
    }

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

    #[test]
    fn test_hash_persistence() {
        let temp_dir = std::env::temp_dir().join("db6_hash_persist_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        
        // Write data
        {
            let mut engine = HashMemoryEngine::open(Path::new(&temp_dir)).unwrap();
            engine.put(1, b"key1", b"value1").unwrap();
            engine.put(1, b"key2", b"value2").unwrap();
            engine.flush().unwrap();
        }
        
        // Reopen and verify
        {
            let engine = HashMemoryEngine::open(Path::new(&temp_dir)).unwrap();
            assert_eq!(engine.get(1, b"key1").unwrap(), Some(b"value1".to_vec()));
            assert_eq!(engine.get(1, b"key2").unwrap(), Some(b"value2".to_vec()));
        }
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// Capability implementations for HashMemoryEngine
impl crate::engine::CanBatch for HashMemoryEngine {}
impl crate::engine::CanFts for HashMemoryEngine {}