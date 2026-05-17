//! 儲存引擎抽象層
//!
//! 定義兩層介面：
//! - [`StorageEngine`] — 底層介面，提供 table_id 隔離、flush/sync/transaction 等方法
//! - [`KvStore`] — SQL 層直接呼叫的精簡介面，executor 使用此介面操作引擎
//!
//! 所有引擎 (Memory/BTree/LSM) 同時實作 `StorageEngine` + `KvStore`。
//! 引擎可互換，使用者可透過字串名稱動態選擇。

pub mod memory;
pub mod btree;
pub mod lsm;
pub mod capability;

pub use memory::{HashMemoryEngine, BTreeMemoryEngine};
pub use btree::BTreeEngine;
pub use lsm::LsmEngine;
pub use capability::{CanOrderBy, CanJoin, CanFts, CanTransaction, CanScan, CanBatch, CanGroupBy};

use crate::error::Result;

/// 引擎統計資訊，用於監控與除錯
///
/// - `key_count`: 目前儲存的鍵數量
/// - `size_bytes`: 估計的儲存大小
/// - `cache_hit_rate`: 快取命中率（僅部分引擎支援）
/// - `in_transaction`: 是否在交易中
/// - `engine`: 引擎類型名稱字串
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub key_count: u64,
    pub size_bytes: u64,
    pub cache_hit_rate: Option<f64>,
    pub in_transaction: bool,
    pub engine: &'static str,
}

/// 底層儲存引擎介面
///
/// 定義了所有儲存引擎必須實作的操作，包括：
/// - 基本 KV 操作 (get/put/delete/scan)
/// - 批量操作 (batch_put/range_delete)
/// - 持久化 (flush/sync)
/// - 交易支援 (begin/commit/rollback)
/// - 可觀測性 (stats)
///
/// `table_id` 參數用於多 table 隔離，不同 table 的資料共用同一引擎但互不干擾。
pub trait StorageEngine: Send + Sync {
    // ── 工廠方法 ────────────────────────────────────────────────────────────

    /// 開啟或建立磁碟資料庫（僅 BTree/LSM 實作）
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>>
    where
        Self: Sized;

    /// 建立記憶體模式資料庫
    fn open_memory() -> Box<dyn StorageEngine>
    where
        Self: Sized;

    /// 回傳引擎類型名稱字串
    fn engine_type(&self) -> &'static str;

    // ── 基本 KV 操作 ────────────────────────────────────────────────────────

    /// 讀取指定 table 中一筆鍵值資料
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// 寫入或更新指定 table 中一筆鍵值資料
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;

    /// 刪除指定 table 中一筆鍵值資料（使用 tombstone 標記）
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;

    /// 範圍掃描 [start, end)，回傳該範圍內所有鍵值對
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;

    /// 批量寫入多筆鍵值對（效能優化，減少鎖定/日誌開銷）
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()>;

    /// 範圍刪除 [start, end)，刪除該範圍內所有鍵
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()>;

    // ── FTS (Full-Text Search) 注意 ────────────────────────────────────────
    // FTS 並非透過 StorageEngine 直接操作，而是透過 FtsIndex 物件使用。
    // 請參考 examples/fts_kv_api.rs 了解使用方式:
    //   let mut fts = FtsIndex::new(engine);
    //   fts.insert(doc_id, text)?;
    //   let results = fts.search(query)?;

    // ── 持久化操作 ──────────────────────────────────────────────────────────

    /// 將記憶體中的髒資料寫回磁碟
    fn flush(&mut self) -> Result<()>;

    /// 執行 fsync 確保資料確實落盤（不遺失）
    fn sync(&mut self) -> Result<()>;

    // ── 交易支援 ────────────────────────────────────────────────────────────

    /// 開始一個新交易
    fn begin_transaction(&mut self) -> Result<()>;

    /// 提交目前交易，使所有修改永久生效
    fn commit_transaction(&mut self) -> Result<()>;

    /// 回滾目前交易，取消所有未提交的修改
    fn rollback_transaction(&mut self) -> Result<()>;

    /// 檢查目前是否有活躍交易
    fn has_transaction(&self) -> bool;

    // ── 可觀測性 ───────────────────────────────────────────────────────────

    /// 取得引擎目前的統計資訊
    fn stats(&self) -> EngineStats;
}

/// SQL 執行器直接使用的 KV 精簡介面
///
/// 與 StorageEngine 不同，此 trait 不需要 flush/sync/transaction，
/// 因為這些操作由 SQL Executor 在更高層次管理。
///
/// 所有引擎（Memory/BTree/LSM）都實作 `impl KvStore for XxxEngine`。
pub trait KvStore: Send + Sync {
    /// 回傳引擎類型名稱（預設為 "unknown"）
    fn engine_type(&self) -> &'static str { "unknown" }

    /// 寫入一筆鍵值資料
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;

    /// 讀取一筆鍵值資料
    fn get(&mut self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// 刪除一筆鍵值資料
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;

    /// 範圍掃描
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
}