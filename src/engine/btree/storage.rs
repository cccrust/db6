//! BTree storage - simplified version

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::Result;
use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeHeader {
    pub root_page: u64,
    pub page_count: u64,
}

impl Default for BTreeHeader {
    fn default() -> Self {
        Self {
            root_page: 0,
            page_count: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Page {
    pub id: u64,
    pub data: Vec<u8>,
}

impl Page {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            data: vec![0; PAGE_SIZE],
        }
    }

    pub fn from_data(id: u64, data: Vec<u8>) -> Self {
        Self { id, data }
    }
}

pub trait Storage: Send + Sync {
    fn read_page(&mut self, page_id: u64) -> Option<Page>;
    fn write_page(&mut self, page: &Page);
    fn alloc_page(&mut self) -> u64;
    fn flush(&mut self) -> Result<()>;
    fn close(&mut self);
    fn header(&self) -> Option<BTreeHeader>;
    fn set_header(&mut self, header: BTreeHeader);
}

pub struct FileStorage {
    file: Arc<Mutex<Option<File>>>,
    path: std::path::PathBuf,
    header: Mutex<BTreeHeader>,
}

impl FileStorage {
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

    fn file_mut(&self) -> std::sync::MutexGuard<Option<File>> {
        self.file.lock().unwrap()
    }

    fn file_size(&self) -> u64 {
        self.file.lock().unwrap().as_ref().map(|f| f.metadata().map(|m| m.len()).unwrap_or(0)).unwrap_or(0)
    }
}

impl Storage for FileStorage {
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

    fn alloc_page(&mut self) -> u64 {
        let mut header = self.header.lock().unwrap();
        let id = header.page_count;
        header.page_count += 1;
        id
    }

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

    fn close(&mut self) {
        *self.file_mut() = None;
    }

    fn header(&self) -> Option<BTreeHeader> {
        Some(self.header.lock().unwrap().clone())
    }

    fn set_header(&mut self, header: BTreeHeader) {
        *self.header.lock().unwrap() = header;
    }
}

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