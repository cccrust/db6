//! Hash Memory Engine - Redis-like, fast KV operations
//! 
//! Uses HashMap for O(1) operations, does NOT support ORDER BY or range scans.

use std::collections::HashMap;
use crate::engine::{EngineStats, StorageEngine};
use crate::error::Result;

pub struct HashMemoryEngine {
    tables: HashMap<u32, HashMap<Vec<u8>, Vec<u8>>>,
}

impl HashMemoryEngine {
    pub fn new() -> Self {
        HashMemoryEngine {
            tables: HashMap::new(),
        }
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
    fn open(_path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::new()))
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
        Ok(())
    }

    fn sync(&mut self) -> Result<()> {
        Ok(())
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
}

// Capability implementations for HashMemoryEngine
impl crate::engine::CanBatch for HashMemoryEngine {}
impl crate::engine::CanFts for HashMemoryEngine {}