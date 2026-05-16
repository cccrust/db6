//! BTreeEngine implementation

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

use crate::engine::{EngineStats, StorageEngine};
use crate::error::{Error, Result};

use super::tree::BTree;

pub struct BTreeEngine {
    tree: RwLock<BTree>,
    in_transaction: RwLock<bool>,
    tx_buffer: RwLock<BTreeMap<u32, BTreeMap<Vec<u8>, Option<Vec<u8>>>>>,
}

impl BTreeEngine {
    pub fn new() -> Self {
        BTreeEngine {
            tree: RwLock::new(BTree::new()),
            in_transaction: RwLock::new(false),
            tx_buffer: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn open(_path: &Path) -> Result<Self> {
        Ok(Self::new())
    }
}

impl Default for BTreeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageEngine for BTreeEngine {
    fn open(path: &Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::open(path)?))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    fn engine_type(&self) -> &'static str {
        "btree"
    }

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

    fn flush(&mut self) -> Result<()> {
        self.tree.write().unwrap().flush()
    }

    fn sync(&mut self) -> Result<()> {
        self.tree.write().unwrap().flush()
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

    fn rollback_transaction(&mut self) -> Result<()> {
        if !*self.in_transaction.read().unwrap() {
            return Err(Error::Transaction("No active transaction".into()));
        }
        self.tx_buffer.write().unwrap().clear();
        *self.in_transaction.write().unwrap() = false;
        Ok(())
    }

    fn has_transaction(&self) -> bool {
        *self.in_transaction.read().unwrap()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btree_basic() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    #[test]
    fn test_btree_scan() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.put(1, b"c", b"3").unwrap();

        let results = engine.scan(1, b"a", b"c").unwrap();
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_btree_delete() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"key", b"value").unwrap();
        engine.delete(1, b"key").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), None);
    }

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

    #[test]
    fn test_btree_transaction_rollback() {
        let mut engine = BTreeEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        
        engine.begin_transaction().unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.rollback_transaction().unwrap();
        
        assert_eq!(engine.get(1, b"b").unwrap(), None);
    }
}

// Capability implementations for BTreeEngine
impl crate::engine::CanOrderBy for BTreeEngine {}
impl crate::engine::CanScan for BTreeEngine {}
impl crate::engine::CanBatch for BTreeEngine {}
impl crate::engine::CanFts for BTreeEngine {}
impl crate::engine::CanTransaction for BTreeEngine {}