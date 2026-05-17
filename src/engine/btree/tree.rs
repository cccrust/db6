//! B+Tree 實作 — 使用 Rust 標準函式庫的 BTreeMap
//!
//! 這是一個簡化版的 B+Tree 實作，底層使用 `std::collections::BTreeMap`。
//! 真正的 B+Tree 實作需要自行管理節點分裂、頁面配置等，此版本利用 Rust
//! 內建的 BTreeMap 完成底層排序與範圍查詢，並透過序列化 (bincode) 實現持久化。
//!
//! 資料儲存在 `btree.dat` 檔案中，整個 BTreeMap 一次序列化寫入。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use crate::error::Result;

/// BTree 資料結構
///
/// - `data`: 核心資料，使用 BTreeMap 保存所有鍵值對（有序）
/// - `path`: 可選的磁碟路徑，用於持久化
pub struct BTree {
    data: BTreeMap<Vec<u8>, Vec<u8>>,
    path: Option<std::path::PathBuf>,
}

impl BTree {
    /// 建立一個空的記憶體 BTree
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            path: None,
        }
    }

    /// 從磁碟載入 BTree
    ///
    /// 從 `path/btree.dat` 讀取序列化資料。
    /// 如果檔案不存在，回傳空 BTree。
    pub fn load(path: &Path) -> Result<Self> {
        let data_path = path.join("btree.dat");

        if !data_path.exists() {
            return Ok(Self::new());
        }

        let mut file = File::open(&data_path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        // 如果反序列化失敗（格式不相容），回傳空 BTree
        let data: BTreeMap<Vec<u8>, Vec<u8>> = match bincode::deserialize(&contents) {
            Ok(d) => d,
            Err(_) => BTreeMap::new(),
        };

        Ok(Self {
            data,
            path: Some(path.to_path_buf()),
        })
    }

    /// 將 BTree 序列化寫入磁碟
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

    /// 設定持久化路徑
    pub fn set_path(&mut self, path: std::path::PathBuf) {
        self.path = Some(path);
    }

    /// 讀取指定鍵的值，O(log n)
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    /// 寫入或更新一筆鍵值
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    /// 刪除一筆鍵值，回傳 true 表示原本存在
    pub fn delete(&mut self, key: &[u8]) -> bool {
        self.data.remove(key).is_some()
    }

    /// 範圍掃描 [start, end)
    ///
    /// 支援四種邊界組合：
    /// - 無邊界：掃描全部
    /// - 僅起始：從 start 開始到結尾
    /// - 僅結束：從開頭到 end（不含）
    /// - 兩者：start（含）到 end（不含）
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

    /// 將資料 flush 到磁碟
    pub fn flush(&mut self) -> Result<()> {
        self.save()
    }
}

impl Default for BTree {
    fn default() -> Self {
        Self::new()
    }
}