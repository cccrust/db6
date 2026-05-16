//! Storage engine abstraction layer.
//!
//! 兩層介面：
//! - [`StorageEngine`] — 底層，table_id 隔離，flush/sync/transaction
//! - [`KvStore`] — SQL 層介面，executor 直接呼叫這個，engine 實作此介面
//!
//! 所有 engine 同時實作 `StorageEngine` + `KvStore`。

pub mod memory;
pub mod btree;
pub mod lsm;

pub use memory::MemoryEngine;
pub use btree::BTreeEngine;
pub use lsm::LsmEngine;

use crate::error::Result;

/// 引擎統計資訊。
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub key_count: u64,
    pub size_bytes: u64,
    pub cache_hit_rate: Option<f64>,
    pub in_transaction: bool,
    pub engine: &'static str,
}

/// 底層儲存引擎介面，支援多 table 隔離。
/// table_id 用於區分不同 table 的資料。
pub trait StorageEngine: Send + Sync {
    // ── 工廠 ────────────────────────────────────────────────────────────────

    /// 開啟或建立磁碟資料庫
    fn open(path: &std::path::Path) -> Result<Box<dyn StorageEngine>>
    where
        Self: Sized;

    /// 建立記憶體模式資料庫
    fn open_memory() -> Box<dyn StorageEngine>
    where
        Self: Sized;

    /// 引擎類型名稱
    fn engine_type(&self) -> &'static str;

    // ── 基本 KV 操作 ────────────────────────────────────────────────────────

    /// 讀取一筆（table_id 用於多 table 隔離）
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// 寫入或更新一筆
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;

    /// 刪除（tombstone）
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;

    /// 範圍掃描 [start, end)
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;

    /// 批量寫入（效能優化）
    fn batch_put(&mut self, table_id: u32, pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Result<()>;

    /// 範圍刪除
    fn range_delete(&mut self, table_id: u32, start: &[u8], end: &[u8]) -> Result<()>;

    // ── 持久化 ────────────────────────────────────────────────────────────

    /// 將記憶體資料刷到磁碟
    fn flush(&mut self) -> Result<()>;

    /// fsync 確保資料落盤
    fn sync(&mut self) -> Result<()>;

    // ── 交易 ─────────────────────────────────────────────────────────────

    /// 開始交易
    fn begin_transaction(&mut self) -> Result<()>;

    /// 提交交易
    fn commit_transaction(&mut self) -> Result<()>;

    /// 回滾交易
    fn rollback_transaction(&mut self) -> Result<()>;

    /// 目前是否有活躍交易
    fn has_transaction(&self) -> bool;

    // ── 可觀測性 ─────────────────────────────────────────────────────────

    /// 取得統計資訊
    fn stats(&self) -> EngineStats;
}

/// SQL 層直接呼叫的 KV 介面。
/// 所有 engine（Memory/BTree/LSM）都實作 `impl KvStore for XxxEngine`。
pub trait KvStore: Send + Sync {
    fn engine_type(&self) -> &'static str { "unknown" }
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;
    fn get(&mut self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
}