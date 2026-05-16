# db6 Agent Instructions

## Build Commands

```bash
cargo build    # Build library
cargo test     # Run tests
./test.sh      # Build + test with output
```

## Architecture

- `src/engine/` — Storage engines: `memory.rs`, `btree/`, `lsm.rs`
- `src/kv/` — `KvStore`, `Transactional`, `Persistent` traits
- `src/fts/` — FTS (CjkTokenizer, EnglishTokenizer, FtsIndex)
- `src/sql/` — SQL stub (minimal implementation)

## Entry Point

`src/lib.rs` exports:
```rust
pub use engine::{EngineStats, StorageEngine, MemoryEngine, BTreeEngine, LsmEngine};
pub use kv::{KvStore, Transactional, Persistent};
pub use fts::{FtsIndex, CjkTokenizer, FtsTokenizer};
pub use sql::{parse, Executor, ResultSet};
```

## Key Design Notes

- StorageEngine trait uses `where Self: Sized` for `open`/`open_memory` to be dyn compatible
- Memory engine uses BTreeMap to support ORDER BY and range scans
- BTreeEngine uses RwLock for thread safety
- scan() uses `std::collections::Bound` for range queries
- table_id parameter enables multi-table isolation

## Git Workflow

`./git.sh <message> <branch>` — commit + push

## Documentation

- [_doc/plan.md](_doc/plan.md) — Full roadmap
- [_doc/v0.1.md](_doc/v0.1.md) through [_doc/v0.3.md](_doc/v0.3.md) — Version history