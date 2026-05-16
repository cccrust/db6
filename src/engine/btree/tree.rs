//! B+Tree implementation - simplified version using BTreeMap

use std::collections::BTreeMap;
use crate::error::Result;

pub struct BTree {
    data: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl BTree {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    pub fn delete(&mut self, key: &[u8]) -> bool {
        self.data.remove(key).is_some()
    }

    pub fn scan(&self, start: &[u8], end: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let start = if start.is_empty() { None } else { Some(start.to_vec()) };
        let end = if end.is_empty() { None } else { Some(end.to_vec()) };

        match (start, end) {
            (None, None) => self.data.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            (Some(s), None) => self.data.range(s..).map(|(k, v)| (k.clone(), v.clone())).collect(),
            (None, Some(e)) => self.data.range(..e).map(|(k, v)| (k.clone(), v.clone())).collect(),
            (Some(s), Some(e)) => self.data.range(s..e).map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Default for BTree {
    fn default() -> Self {
        Self::new()
    }
}