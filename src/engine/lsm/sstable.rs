//! SSTable — LSM-Tree 的磁碟排序字串表
//!
//! SSTable (Sorted String Table) 是 LSM-Tree 在磁碟上的儲存格式。
//! 當 MemTable 累積到一定大小時，其中的資料會被排序後寫入 SSTable。
//!
//! 每個 SSTable 儲存在 `.sst` 檔案中，使用 bincode 進行序列化。
//! 查詢時從最新的 SSTable 向舊的搜尋。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::{Error, Result};

/// SSTable 結構
///
/// 每個 SSTable 對應一個 `.sst` 檔案，包含一組有序的鍵值對。
pub struct SSTable {
    /// 檔案路徑
    path: std::path::PathBuf,
    /// 排序的鍵值資料
    data: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl SSTable {
    /// 建立一個新的 SSTable 並寫入磁碟
    ///
    /// 從 MemTable flush 時呼叫此方法。
    /// `data` 傳入時已是排序狀態（來自 BTreeMap）。
    pub fn create(path: &Path, data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Self> {
        let mut ss = Self {
            path: path.to_path_buf(),
            data: BTreeMap::new(),
        };

        for (k, v) in data {
            ss.data.insert(k, v);
        }

        ss.write_to_disk()?;
        Ok(ss)
    }

    /// 從磁碟載入一個 SSTable
    pub fn open(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path: path.to_path_buf(),
                data: BTreeMap::new(),
            });
        }

        let mut file = File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        let data: BTreeMap<Vec<u8>, Vec<u8>> = match bincode::deserialize(&contents) {
            Ok(d) => d,
            Err(_) => BTreeMap::new(),
        };

        Ok(Self {
            path: path.to_path_buf(),
            data,
        })
    }

    /// 將 BTreeMap 序列化並寫入 `.sst` 檔案
    fn write_to_disk(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;

        let data = bincode::serialize(&self.data)
            .map_err(|e| Error::Sql(format!("bincode: {:?}", e)))?;

        file.write_all(&data)?;
        Ok(())
    }

    /// 讀取指定鍵的值
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    /// 範圍掃描
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

    /// 傳回此 SSTable 的鍵數量
    pub fn len(&self) -> u64 {
        self.data.len() as u64
    }
}