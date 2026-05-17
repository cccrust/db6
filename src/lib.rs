//! db6 — Unified database with pluggable storage engines (Memory/BTree/LSM) + KV + FTS5 + MSGQ
//!
//! # Architecture
//!
//! - KV API: unified key-value interface (storage engines implement this)
//! - SQL API: parser -> planner -> executor (depends on KV)
//! - Storage Engines: Memory, BTree, LSM (implement StorageEngine trait)
//! - FTS5: full-text search on top of KV interface
//! - MSGQ: message queue on top of KV interface

#![allow(dead_code, unused)]

pub mod engine;
pub mod error;
pub mod sql;
pub mod fts;
pub mod kv;
pub mod query;
pub mod msgq;

pub use engine::{EngineStats, StorageEngine, KvStore, CanOrderBy, CanJoin, CanFts, CanTransaction, CanScan, CanBatch};
pub use kv::{KvStore as KvApi, KvEngine};
pub use query::Db;
pub use error::{Error, Result};
pub use fts::{FtsIndex, FtsTokenizer, CjkTokenizer, EnglishTokenizer};
pub use sql::{parse, Executor, ResultSet, SqlExecutor};
pub use msgq::{Msgq, Queue, Message, QueueStats};