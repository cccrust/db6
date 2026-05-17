//! db6 — 統一資料庫系統，支援可插拔儲存引擎 (Memory/BTree/LSM) + KV + FTS5 + MSGQ
//!
//! # 架構圖
//!
//! - KV API: 統一的鍵值介面（所有儲存引擎實作此介面）
//! - SQL API: 解析器 → 規劃器 → 執行器（依賴於 KV 層）
//! - Storage Engines: Memory, BTree, LSM（實作 StorageEngine trait）
//! - FTS5: 基於 KV 介面的全文搜尋
//! - MSGQ: 基於 KV 介面的訊息佇列系統

// 允許未使用的程式碼與變數，避免開發階段警告干擾
#![allow(dead_code, unused)]

/// 儲存引擎模組：實作 StorageEngine trait
pub mod engine;
/// 錯誤型別模組：統一的 Error 與 Result
pub mod error;
/// SQL 子系統：解析器、規劃器、執行器
pub mod sql;
/// 全文搜尋模組：基於倒排索引的 FTS
pub mod fts;
/// 統一 KV API：工廠模式封裝多種引擎
pub mod kv;
/// 高階查詢模組：Fluent API 的 Db 結構
pub mod query;
/// 訊息佇列模組：同步/非同步佇列與 Pub/Sub
pub mod msgq;

// 公開 API：儲存引擎相關
pub use engine::{EngineStats, StorageEngine, KvStore, CanOrderBy, CanJoin, CanFts, CanTransaction, CanScan, CanBatch};
// 公開 API：KV 層
pub use kv::{KvStore as KvApi, KvEngine};
// 公開 API：高階查詢
pub use query::Db;
// 公開 API：錯誤型別
pub use error::{Error, Result};
// 公開 API：全文搜尋
pub use fts::{FtsIndex, FtsTokenizer, CjkTokenizer, EnglishTokenizer};
// 公開 API：SQL 子系統
pub use sql::{parse, Executor, ResultSet, SqlExecutor};
// 公開 API：訊息佇列系統
pub use msgq::{Msgq, SyncQueue, QueueMeta, SyncQueueMessage, QueueStats, SyncPubSub, SyncPubSubMessage, AsyncQueue, AsyncQueueMessage, AsyncMsgq, AsyncPubSub, AsyncPubSubMessage};