//! WAL — 預寫式日誌 (Write-Ahead Log)
//!
//! WAL 是 LSM-Tree 的可靠性保證機制。根據 ARIES 演算法的原則，
//! 在資料寫入 MemTable 之前，必須先寫入 WAL。
//!
//! 若程式崩潰，重新啟動時可透過 WAL 復原尚未 flush 到 SSTable 的資料。
//!
//! ## 日誌格式
//!
//! 每筆日誌記錄以 TLV (Type-Length-Value) 格式儲存：
//! ```text
//! [key_len: u32][key: key_len bytes][value_len: u32][value: value_len bytes]
//! ```

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::Result;

/// WAL 結構
///
/// 每個 LSM 引擎實例對應一個 `wal.log` 檔案。
pub struct Wal {
    /// WAL 檔案路徑
    path: std::path::PathBuf,
    /// 檔案控制代碼
    file: File,
}

impl Wal {
    /// 建立一個新的 WAL（append 模式）
    pub fn create(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// 開啟一個已存在的 WAL
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// 寫入一筆日誌記錄
    ///
    /// 格式：`[key_len:4bytes][key][value_len:4bytes][value]`
    pub fn write(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.path)?;

        // 寫入鍵的長度與內容
        let len = key.len() as u32;
        file.write_all(&len.to_le_bytes())?;
        file.write_all(key)?;

        // 寫入值的長度與內容
        let len = value.len() as u32;
        file.write_all(&len.to_le_bytes())?;
        file.write_all(value)?;

        Ok(())
    }

    /// 從 WAL 復原所有未 flush 的資料
    ///
    /// 讀取 WAL 中所有記錄並回傳為鍵值對列表。
    pub fn recover(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut file = File::open(&self.path)?;
        let mut results = Vec::new();

        // 依序讀取每一組 [key_len][key][value_len][value]
        loop {
            let mut len_buf = [0u8; 4];
            match file.read_exact(&mut len_buf) {
                Ok(_) => {}
                Err(_) => break,
            }

            let key_len = u32::from_le_bytes(len_buf) as usize;
            let mut key = vec![0u8; key_len];
            file.read_exact(&mut key)?;

            let mut len_buf = [0u8; 4];
            file.read_exact(&mut len_buf)?;
            let val_len = u32::from_le_bytes(len_buf) as usize;
            let mut value = vec![0u8; val_len];
            file.read_exact(&mut value)?;

            results.push((key, value));
        }

        Ok(results)
    }

    /// 清空 WAL 內容（資料已 flush 到 SSTable 後呼叫）
    pub fn clear(&self) -> Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        drop(file);
        Ok(())
    }
}