//! db6 — Unified database with pluggable storage engines and SQL + FTS5 support
//!
//! # Architecture
//!
//! ```
//! User API
//!     ├── KV API    (KvStore trait)     ← SQL 層依賴這個
//!     └── SQL API   (parser → planner → executor)
//!                         │
//!                         └── calls KvStore
//!                               │
//!                               └── impl for each engine
//!                                     ├── Memory engine
//!                                     ├── BTree engine
//!                                     └── LSM engine
//!                                           └── FTS5 (基於 KV 介面)
//! ```
//!
//! # Key Traits
//!
//! - [`KvStore`] — unified KV interface (SQL 層直接依賴這個)
//! - [`StorageEngine`] — low-level storage (engine 內部實作)
//! - All engines implement both traits

#![allow(dead_code, unused)]

pub mod engine;
pub mod error;
pub mod kv;
pub mod sql;
pub mod fts;

pub use engine::{EngineStats, StorageEngine};
pub use error::{Error, Result};
pub use fts::{FtsIndex, FtsTokenizer, CjkTokenizer};
pub use kv::{KvStore, Transactional, Persistent};
pub use sql::{parse, Executor, ResultSet};