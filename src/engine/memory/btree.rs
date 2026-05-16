//! BTree Memory Engine - SQLite-like, supports SQL operations
//! 
//! Uses BTreeMap for ordered data, supports ORDER BY and range scans.

use std::collections::BTreeMap;
use crate::engine::{EngineStats, StorageEngine};
use crate::error::Result;

pub struct BTreeMemoryEngine {
    tables: std::collections::HashMap<u32, BTreeMap<Vec<u8>, Vec<u8>>>,
}

impl BTreeMemoryEngine {
    pub fn new() -> Self {
        BTreeMemoryEngine {
            tables: std::collections::HashMap::new(),
        }
    }

    fn table(&self, table_id: u32) -> &BTreeMap<Vec<u8>, Vec<u8>> {
        self.tables.get(&table_id).unwrap()
    }

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

impl StorageEngine for BTreeMemoryEngine {
    fn open(_path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        Ok(Box::new(Self::new()))
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        Box::new(Self::new())
    }

    fn engine_type(&self) -> &'static str {
        "memory-btree"
    }

    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.table(table_id).get(key).cloned())
    }

    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()> {
        self.table_mut(table_id).insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()> {
        self.table_mut(table_id).remove(key);
        Ok(())
    }

    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        use std::collections::Bound;
        let table = self.table(table_id);

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

    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()> {
        let table = self.table_mut(table_id);
        for (key, value) in pairs {
            table.insert(key, value);
        }
        Ok(())
    }

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

        let keys: Vec<Vec<u8>> = table.range((start_bound, end_bound)).map(|(k, _)| k.clone()).collect();
        for key in keys {
            table.remove(&key);
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
        Err(crate::error::Error::NotSupported("Memory engine 不支援交易".into()))
    }

    fn commit_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("Memory engine 不支援交易".into()))
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        Err(crate::error::Error::NotSupported("Memory engine 不支援交易".into()))
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
            engine: "memory-btree",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btree_basic() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"hello", b"world").unwrap();
        assert_eq!(engine.get(1, b"hello").unwrap(), Some(b"world".to_vec()));
        assert_eq!(engine.get(1, b"missing").unwrap(), None);
    }

    #[test]
    fn test_btree_scan() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"a", b"1").unwrap();
        engine.put(1, b"b", b"2").unwrap();
        engine.put(1, b"c", b"3").unwrap();

        let results = engine.scan(1, b"a", b"c").unwrap();
        assert_eq!(results.len(), 2); // a and b
    }

    #[test]
    fn test_btree_delete() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"k", b"v").unwrap();
        engine.delete(1, b"k").unwrap();
        assert_eq!(engine.get(1, b"k").unwrap(), None);
    }

    #[test]
    fn test_btree_multi_table() {
        let mut engine = BTreeMemoryEngine::new();
        engine.put(1, b"key", b"table1").unwrap();
        engine.put(2, b"key", b"table2").unwrap();
        assert_eq!(engine.get(1, b"key").unwrap(), Some(b"table1".to_vec()));
        assert_eq!(engine.get(2, b"key").unwrap(), Some(b"table2".to_vec()));
    }

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
}