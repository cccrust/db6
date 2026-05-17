//! MemTable — LSM-Tree 的記憶體寫入緩衝區
//!
//! MemTable (Memory Table) 是 LSM-Tree 的第一層，所有寫入先進入這裡。
//! 內部使用 `BTreeMap` 確保資料有序，支援範圍掃描。
//!
//! 當 MemTable 達到一定大小時，會被 flush 到磁碟成為 SSTable。
//!
//! ## Value 枚舉
//!
//! - `Data(Vec<u8>)`: 正常資料
//! - `Tombstone`: 刪除標記（墓碑），表示該鍵已被刪除

use std::collections::BTreeMap;

/// 值類型：正常資料或刪除墓碑標記
#[derive(Clone, Debug)]
pub enum Value {
    /// 正常資料
    Data(Vec<u8>),
    /// 刪除標記（tombstone），表示此鍵已被刪除
    Tombstone,
}

impl Value {
    /// 是否為正常資料（非 Tombstone）
    pub fn is_data(&self) -> bool {
        matches!(self, Value::Data(_))
    }

    /// 取得資料內容（Tombstone 回傳 None）
    pub fn get_data(&self) -> Option<&Vec<u8>> {
        match self {
            Value::Data(v) => Some(v),
            Value::Tombstone => None,
        }
    }
}

/// MemTable 結構
///
/// 使用 `BTreeMap` 儲存有序的鍵值對，鍵為 `Vec<u8>`，值為 `Value`。
pub struct MemTable {
    map: BTreeMap<Vec<u8>, Value>,
}

impl MemTable {
    /// 建立一個空的 MemTable
    pub fn new() -> Self {
        Self { map: BTreeMap::new() }
    }

    /// 寫入一筆資料（插入或更新）
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.map.insert(key, Value::Data(value));
    }

    /// 刪除一筆資料（插入 Tombstone 標記）
    pub fn delete(&mut self, key: Vec<u8>) {
        self.map.insert(key, Value::Tombstone);
    }

    /// 讀取一筆資料
    pub fn get(&self, key: &[u8]) -> Option<&Value> {
        self.map.get(key)
    }

    /// 範圍掃描 [start, end)，只回傳正常資料（跳過 Tombstone）
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

    /// 傳回鍵的數量（含 Tombstone）
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 傳回所有正常資料（用於 flush 到 SSTable）
    pub fn all_data(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.map.iter()
            .filter(|(_, v)| v.is_data())
            .map(|(k, v)| (k.clone(), v.get_data().unwrap().clone()))
            .collect()
    }

    /// 清空 MemTable
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

impl Default for MemTable {
    fn default() -> Self {
        Self::new()
    }
}