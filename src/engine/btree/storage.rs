//! BTree 儲存層 — 頁面管理與持久化
//!
//! 提供基於頁面 (Page) 的儲存抽象，支援記憶體與檔案兩種實作。
//! 一個頁面的大小固定為 4096 bytes（與常見的作業系統頁面大小一致）。
//!
//! 檔案儲存格式：
//! ```text
//! [Page 0: Header] → [Page 1: Data] → [Page 2: Data] → ...
//! ```
//! Page 0 為檔案頭部 (BTreeHeader)，記錄根頁面 ID 與頁面總數。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 頁面大小：4096 bytes（4KB）
pub const PAGE_SIZE: usize = 4096;

/// BTree 檔案頭部，儲存在 Page 0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeHeader {
    /// 根頁面的 ID
    pub root_page: u64,
    /// 已配置的頁面總數
    pub page_count: u64,
}

impl Default for BTreeHeader {
    fn default() -> Self {
        Self {
            root_page: 0,
            page_count: 1, // Page 0 已被 Header 使用
        }
    }
}

/// 頁面結構：固定大小的資料區塊
#[derive(Debug, Clone)]
pub struct Page {
    /// 頁面唯一識別碼
    pub id: u64,
    /// 頁面內容（長度固定為 PAGE_SIZE）
    pub data: Vec<u8>,
}

impl Page {
    /// 建立一個以 0 填充的新頁面
    pub fn new(id: u64) -> Self {
        Self {
            id,
            data: vec![0; PAGE_SIZE],
        }
    }

    /// 從已有資料建立頁面
    pub fn from_data(id: u64, data: Vec<u8>) -> Self {
        Self { id, data }
    }
}

/// 儲存抽象層 trait，定義頁面層級的操作
pub trait Storage: Send + Sync {
    /// 讀取指定頁面的資料
    fn read_page(&mut self, page_id: u64) -> Option<Page>;
    /// 寫入一個頁面
    fn write_page(&mut self, page: &Page);
    /// 配置一個新頁面（回傳新的頁面 ID）
    fn alloc_page(&mut self) -> u64;
    /// 將所有改動寫入磁碟
    fn flush(&mut self) -> Result<()>;
    /// 關閉儲存
    fn close(&mut self);
    /// 讀取檔案頭部
    fn header(&self) -> Option<BTreeHeader>;
    /// 設定檔案頭部
    fn set_header(&mut self, header: BTreeHeader);
}

/// 檔案儲存實作
///
/// 將 BTree 頁面儲存在實體檔案中，每個頁面佔用固定的 PAGE_SIZE 空間。
/// 使用 `Arc<Mutex<Option<File>>>` 實現可選的共享檔案存取。
pub struct FileStorage {
    /// 底層檔案（使用 Option 支援關閉操作）
    file: Arc<Mutex<Option<File>>>,
    /// 檔案路徑
    path: std::path::PathBuf,
    /// 檔案頭部（執行緒安全）
    header: Mutex<BTreeHeader>,
}

impl FileStorage {
    /// 開啟或建立一個 BTree 檔案
    ///
    /// 如果檔案已存在且大小不小於一個頁面，會嘗試讀取檔案頭部。
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let metadata = file.metadata()?;
        let header = if metadata.len() >= PAGE_SIZE as u64 {
            let mut buffer = vec![0u8; PAGE_SIZE];
            let mut f = file.try_clone()?;
            f.seek(SeekFrom::Start(0))?;
            f.read_exact(&mut buffer)?;
            bincode::deserialize(&buffer).unwrap_or_default()
        } else {
            BTreeHeader::default()
        };

        Ok(Self {
            file: Arc::new(Mutex::new(Some(file))),
            path: path.to_path_buf(),
            header: Mutex::new(header),
        })
    }

    /// 取得檔案的可變參考（鎖定 Mutex）
    fn file_mut(&self) -> std::sync::MutexGuard<'_, Option<File>> {
        self.file.lock().unwrap()
    }

    /// 取得檔案大小（bytes）
    fn file_size(&self) -> u64 {
        self.file.lock().unwrap().as_ref()
            .map(|f| f.metadata().map(|m| m.len()).unwrap_or(0))
            .unwrap_or(0)
    }
}

impl Storage for FileStorage {
    /// 從檔案中讀取指定頁面
    ///
    /// 計算頁面偏移量 = page_id × PAGE_SIZE，然後讀取 PAGE_SIZE 個位元組。
    fn read_page(&mut self, page_id: u64) -> Option<Page> {
        let offset = page_id * PAGE_SIZE as u64;
        if offset >= self.file_size() {
            return None;
        }

        let mut buffer = vec![0u8; PAGE_SIZE];
        if let Some(ref mut f) = *self.file_mut() {
            if f.seek(SeekFrom::Start(offset)).is_err() {
                return None;
            }
            if f.read(&mut buffer).is_err() {
                return None;
            }
            Some(Page::from_data(page_id, buffer))
        } else {
            None
        }
    }

    /// 將頁面寫入檔案
    ///
    /// 如果偏移量超過檔案大小，會自動擴展檔案。
    fn write_page(&mut self, page: &Page) {
        let data = &page.data;
        let offset = page.id * PAGE_SIZE as u64;

        if offset + data.len() as u64 > self.file_size() {
            if let Some(ref mut f) = *self.file_mut() {
                let _ = f.set_len(offset + data.len() as u64);
            }
        }

        if let Some(ref mut f) = *self.file_mut() {
            let _ = f.seek(SeekFrom::Start(offset));
            let _ = f.write_all(data);
        }
    }

    /// 配置一個新頁面，回傳新的頁面 ID 並增加計數
    fn alloc_page(&mut self) -> u64 {
        let mut header = self.header.lock().unwrap();
        let id = header.page_count;
        header.page_count += 1;
        id
    }

    /// 將頭部寫回檔案並呼叫 fsync
    fn flush(&mut self) -> Result<()> {
        if let Some(ref mut f) = *self.file_mut() {
            f.flush()?;
        }
        let header = self.header.lock().unwrap();
        let data = bincode::serialize(&*header).unwrap_or_default();
        if let Some(ref mut f) = *self.file_mut() {
            f.seek(SeekFrom::Start(0))?;
            f.write_all(&data)?;
            f.flush()?;
        }
        Ok(())
    }

    /// 關閉檔案
    fn close(&mut self) {
        *self.file_mut() = None;
    }

    /// 讀取頭部
    fn header(&self) -> Option<BTreeHeader> {
        Some(self.header.lock().unwrap().clone())
    }

    /// 設定頭部
    fn set_header(&mut self, header: BTreeHeader) {
        *self.header.lock().unwrap() = header;
    }
}

/// 記憶體儲存實作（測試用途）
///
/// 頁面保存在 BTreeMap 中，flush 不做任何事。
pub struct MemoryStorage {
    pages: BTreeMap<u64, Page>,
    next_page_id: u64,
    header: BTreeHeader,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
            next_page_id: 1,
            header: BTreeHeader::default(),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for MemoryStorage {
    fn read_page(&mut self, page_id: u64) -> Option<Page> {
        self.pages.get(&page_id).cloned()
    }

    fn write_page(&mut self, page: &Page) {
        self.pages.insert(page.id, page.clone());
    }

    fn alloc_page(&mut self) -> u64 {
        let id = self.next_page_id;
        self.next_page_id += 1;
        id
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) {
        self.pages.clear();
        self.next_page_id = 1;
    }

    fn header(&self) -> Option<BTreeHeader> {
        Some(self.header.clone())
    }

    fn set_header(&mut self, header: BTreeHeader) {
        self.header = header;
    }
}