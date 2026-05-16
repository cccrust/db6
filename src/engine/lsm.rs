//! LSM engine — Simplified implementation based on lsm5

use std::collections::BTreeMap;
use std::sync::RwLock;

use crate::engine::{EngineStats, StorageEngine};
use crate::error::{Error, Result};

/// Value type in LSM - supports tombstones for deletions
#[derive(Clone, Debug)]
enum Value {
    Data(Vec<u8>),
    Tombstone,
}

/// MemTable - in-memory write buffer
struct MemTable {
    map: BTreeMap<Vec<u8>, Value>,
}

impl MemTable {
    fn new() -> Self {
        Self { map: BTreeMap::new() }
    }

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.map.insert(key, Value::Data(value));
    }

    fn delete(&mut self, key: Vec<u8>) {
        self.map.insert(key, Value::Tombstone);
    }

    fn get(&self, key: &[u8]) -> Option<&Value> {
        self.map.get(key)
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let start = if start.is_empty() { None } else { Some(start.to_vec()) };
        let end = if end.is_empty() { None } else { Some(end.to_vec()) };

        match (start, end) {
            (None, None) => self.map.iter()
                .filter(|(_, v)| !matches!(v, Value::Tombstone))
                .map(|(k, v)| match v {
                    Value::Data(d) => (k.clone(), d.clone()),
                    Value::Tombstone => (k.clone(), vec![]),
                })
                .collect(),
            (Some(s), None) => self.map.range(s..)
                .filter(|(_, v)| !matches!(v, Value::Tombstone))
                .map(|(k, v)| match v {
                    Value::Data(d) => (k.clone(), d.clone()),
                    Value::Tombstone => (k.clone(), vec![]),
                })
                .collect(),
            (None, Some(e)) => self.map.range(..e)
                .filter(|(_, v)| !matches!(v, Value::Tombstone))
                .map(|(k, v)| match v {
                    Value::Data(d) => (k.clone(), d.clone()),
                    Value::Tombstone => (k.clone(), vec![]),
                })
                .collect(),
            (Some(s), Some(e)) => self.map.range(s..e)
                .filter(|(_, v)| !matches!(v, Value::Tombstone))
                .map(|(k, v)| match v {
                    Value::Data(d) => (k.clone(), d.clone()),
                    Value::Tombstone => (k.clone(), vec![]),
                })
                .collect(),
        }
    }
}

/// LSM Engine - Simplified version
/// 
/// Architecture:
/// - MemTable: in-memory write buffer
/// - Data stored in memory (no disk persistence for v0.3)
/// - Limited transaction support (single table only)
pub struct LsmEngine {
    memtable: RwLock<MemTable>,
    in_transaction: RwLock<bool>,
    tx_buffer: RwLock<Option<BTreeMap<Vec<u8>, Option<Vec<u8>>>>>,
}

impl LsmEngine {
    pub fn new() -> Self {
        LsmEngine {
            memtable: RwLock::new(MemTable::new()),
            in_transaction: RwLock::new(false),
            tx_buffer: RwLock::new(None),
        }
    }

    pub fn open(_path: &std::path::Path) -> Result<Self> {
        Ok(Self::new())
    }
}

impl Default for LsmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageEngine for LsmEngine {
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    fn engine_type(&self) -> &'static str {
        "lsm"
    }

    fn get(&self, _table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // Check transaction buffer first
        if let Ok(tx) = self.tx_buffer.read() {
            if let Some(buffer) = tx.as_ref() {
                if let Some(value) = buffer.get(key) {
                    return match value {
                        Some(v) => Ok(Some(v.clone())),
                        None => Ok(None), // Deleted in transaction
                    };
                }
            }
        }

        // Check memtable
        let mem = self.memtable.read().unwrap();
        match mem.get(key) {
            Some(Value::Data(v)) => Ok(Some(v.clone())),
            Some(Value::Tombstone) => Ok(None),
            None => Ok(None),
        }
    }

    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        // LSM only supports single-table operations
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
                buffer.insert(key.to_vec(), None); // Tombstone
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
        Ok(())
    }

    fn sync(&mut self) -> Result<()> {
        Ok(())
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
            for (key, value) in buffer.into_iter() {
                match value {
                    Some(v) => self.memtable.write().unwrap().put(key, v),
                    None => self.memtable.write().unwrap().delete(key.clone()),
                }
            }
        }

        *self.in_transaction.write().unwrap() = false;
        Ok(())
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
        EngineStats {
            key_count: self.memtable.read().unwrap().map.len() as u64,
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
}