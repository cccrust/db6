//! db6 — Unified database with pluggable storage engines (Memory/BTree/LSM) + KV + FTS5
//!
//! # Architecture
//!
//! - KV API: unified key-value interface (storage engines implement this)
//! - SQL API: parser -> planner -> executor (depends on KV)
//! - Storage Engines: Memory, BTree, LSM (implement StorageEngine trait)
//! - FTS5: full-text search on top of KV interface

#![allow(dead_code, unused)]

pub mod engine;
pub mod error;
pub mod kv;
pub mod sql;
pub mod fts;

pub use engine::{EngineStats, StorageEngine};
pub use error::{Error, Result};
pub use fts::{FtsIndex, FtsTokenizer, CjkTokenizer, EnglishTokenizer};
pub use kv::{KvStore, Transactional, Persistent};
pub use sql::{parse, Executor, ResultSet};