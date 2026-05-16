//! LSM engine — 移植自 lsm5/

use crate::engine::{EngineStats, StorageEngine};
use crate::error::Result;

/// LSM engine（Placeholder）
pub struct LsmEngine {
    _placeholder: (),
}

impl LsmEngine {
    pub fn new() -> Self {
        LsmEngine { _placeholder: () }
    }
}

impl Default for LsmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageEngine for LsmEngine {
    fn open(_path: &std::path::Path) -> Result<Box<dyn StorageEngine>> {
        todo!("移植自 lsm5/")
    }

    fn open_memory() -> Box<dyn StorageEngine> {
        todo!("LSM engine 不支援記憶體模式")
    }

    fn engine_type(&self) -> &'static str {
        "lsm"
    }

    fn get(&self, _table_id: u32, _key: &[u8]) -> Result<Option<Vec<u8>>> {
        todo!()
    }

    fn put(&mut self, _table_id: u32, _key: &[u8], _value: &[u8]) -> Result<()> {
        todo!()
    }

    fn delete(&mut self, _table_id: u32, _key: &[u8]) -> Result<()> {
        todo!()
    }

    fn scan(&self, _table_id: u32, _start: &[u8], _end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        todo!()
    }

    fn flush(&mut self) -> Result<()> {
        todo!()
    }

    fn sync(&mut self) -> Result<()> {
        todo!()
    }

    fn begin_transaction(&mut self) -> Result<()> {
        todo!()
    }

    fn commit_transaction(&mut self) -> Result<()> {
        todo!()
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        todo!()
    }

    fn has_transaction(&self) -> bool {
        todo!()
    }

    fn stats(&self) -> EngineStats {
        EngineStats {
            key_count: 0,
            size_bytes: 0,
            cache_hit_rate: None,
            in_transaction: false,
            engine: "lsm",
        }
    }
}