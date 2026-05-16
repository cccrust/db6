//! KvStore trait — unified key-value interface.
//!
//! Implemented by all engines (Memory, BTree, LSM).

use crate::error::Result;

/// Core KV operations.  All engines must implement this.
pub trait KvStore {
    /// Insert or update a key-value pair.
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Retrieve value for key.  Returns `Ok(Some(value))` if found.
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Delete a key.
    fn delete(&mut self, key: &[u8]) -> Result<()>;

    /// Scan keys in [start, end) range, ordered by key.
    /// Empty start (b"") = negative infinity.
    /// Empty end (b"") = positive infinity.
    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
}

/// Transaction support (optional).
pub trait Transactional {
    fn begin(&mut self);
    fn commit(&mut self) -> Result<()>;
    fn rollback(&mut self) -> Result<()>;
    fn has_transaction(&self) -> bool;
    fn tx_put(&mut self, key: &[u8], value: &[u8]) -> Result<()>;
    fn tx_delete(&mut self, key: &[u8]) -> Result<()>;
}

/// Persistence operations (optional).
pub trait Persistent {
    fn flush(&mut self) -> Result<()>;
    fn sync(&mut self) -> Result<()>;
}