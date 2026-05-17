//! 記憶體引擎模組
//!
//! 提供兩種記憶體引擎實作：
//! - `HashMemoryEngine`: 基於 HashMap，O(1) 操作，不支援 ORDER BY/scan
//! - `BTreeMemoryEngine`: 基於 BTreeMap，O(log n) 操作，支援 ORDER BY/scan
//!
//! 記憶體引擎適用於測試、快取、或不需要持久化的場景。
//! 資料在程式結束後即消失。

pub mod hash;
pub mod btree;

pub use hash::HashMemoryEngine;
pub use btree::BTreeMemoryEngine;