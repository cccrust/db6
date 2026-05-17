//! LSM-Tree 儲存引擎模組
//!
//! 實作基於 LSM-Tree (Log-Structured Merge-Tree) 的持久化儲存引擎，
//! 特別適合高寫入吞吐量的場景。
//!
//! 架構：
//! - MemTable: 記憶體寫入緩衝區 (可寫/不可寫)
//! - SSTable: 磁碟上的排序字串表 (Sorted String Table)
//! - WAL: 預寫式日誌，用於資料復原
//! - Bloom Filter: 布隆過濾器，加速不存在的鍵查詢

pub mod engine;
pub mod memtable;
pub mod sstable;
pub mod wal;
pub mod bloom;

pub use engine::LsmEngine;