//! B+Tree implementation - simplified version using BTreeMap

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use crate::error::Result;

pub struct BTree {
    data: BTreeMap<Vec<u8>, Vec<u8>>,
    path: Option<std::path::PathBuf>,
}

impl BTree {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            path: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let data_path = path.join("btree.dat");
        
        if !data_path.exists() {
            return Ok(Self::new());
        }
        
        let mut file = File::open(&data_path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        
        let data: BTreeMap<Vec<u8>, Vec<u8>> = match bincode::deserialize(&contents) {
            Ok(d) => d,
            Err(_) => BTreeMap::new(),
        };
        
        Ok(Self {
            data,
            path: Some(path.to_path_buf()),
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(ref path) = self.path {
            let data_path = path.join("btree.dat");
            
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&data_path)?;
            
            let data = bincode::serialize(&self.data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("bincode: {:?}", e)))?;
            
            file.write_all(&data)?;
        }
        Ok(())
    }

    pub fn set_path(&mut self, path: std::path::PathBuf) {
        self.path = Some(path);
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
        self.save()
    }
}

impl Default for BTree {
    fn default() -> Self {
        Self::new()
    }
}