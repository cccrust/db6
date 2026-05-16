//! MemTable - in-memory write buffer

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub enum Value {
    Data(Vec<u8>),
    Tombstone,
}

impl Value {
    pub fn is_data(&self) -> bool {
        matches!(self, Value::Data(_))
    }

    pub fn get_data(&self) -> Option<&Vec<u8>> {
        match self {
            Value::Data(v) => Some(v),
            Value::Tombstone => None,
        }
    }
}

pub struct MemTable {
    map: BTreeMap<Vec<u8>, Value>,
}

impl MemTable {
    pub fn new() -> Self {
        Self { map: BTreeMap::new() }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.map.insert(key, Value::Data(value));
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.map.insert(key, Value::Tombstone);
    }

    pub fn get(&self, key: &[u8]) -> Option<&Value> {
        self.map.get(key)
    }

    pub fn scan(&self, start: &[u8], end: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let start = if start.is_empty() { None } else { Some(start.to_vec()) };
        let end = if end.is_empty() { None } else { Some(end.to_vec()) };

        match (start, end) {
            (None, None) => self.map.iter()
                .filter(|(_, v)| v.is_data())
                .map(|(k, v)| (k.clone(), v.get_data().unwrap().clone()))
                .collect(),
            (Some(s), None) => self.map.range(s..)
                .filter(|(_, v)| v.is_data())
                .map(|(k, v)| (k.clone(), v.get_data().unwrap().clone()))
                .collect(),
            (None, Some(e)) => self.map.range(..e)
                .filter(|(_, v)| v.is_data())
                .map(|(k, v)| (k.clone(), v.get_data().unwrap().clone()))
                .collect(),
            (Some(s), Some(e)) => self.map.range(s..e)
                .filter(|(_, v)| v.is_data())
                .map(|(k, v)| (k.clone(), v.get_data().unwrap().clone()))
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn all_data(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.map.iter()
            .filter(|(_, v)| v.is_data())
            .map(|(k, v)| (k.clone(), v.get_data().unwrap().clone()))
            .collect()
    }
}

impl Default for MemTable {
    fn default() -> Self {
        Self::new()
    }
}