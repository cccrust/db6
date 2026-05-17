//! 磁碟 BTree 引擎模組
//!
//! 實作基於 BTree 結構的持久化儲存引擎，支援交易功能。
//!
//! 模組結構：
//! - `tree.rs`: BTree 資料結構（基於 BTreeMap）
//! - `storage.rs`: 檔案儲存與分頁管理
//! - `engine.rs`: StorageEngine trait 實作

mod storage;
mod tree;
mod engine;

pub use storage::{FileStorage, MemoryStorage, Page, Storage, PAGE_SIZE};
pub use tree::BTree;
pub use engine::BTreeEngine;