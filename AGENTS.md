# db6 Agent Instructions

## Build Commands

```bash
cargo build    # Build library
cargo test     # Run tests
./test.sh      # Build + test with output
```

## Version Strategy

- **v0.x**: KV layer only (StorageEngine + 3 engines)
- **v1.0+**: Add SQL layer

## Architecture

- `src/engine/` — Storage engines: `memory.rs`, `btree/`, `lsm.rs`
- `src/kv/` — `KvStore`, `Transactional`, `Persistent` traits
- `src/sql/` — SQL stub (ignored until v1.0)
- `src/fts/` — FTS stub (ignored until v1.0)

## Status (v1.0)

| Component | Status |
|-----------|--------|
| StorageEngine trait | ✅ Done |
| MemoryEngine | ✅ Done |
| BTreeEngine | ✅ Done |
| LsmEngine | ✅ Done |
| KvStore trait | ✅ Done |
| FtsIndex (KV-based) | ✅ Done |
| CjkTokenizer | ✅ Done |
| EnglishTokenizer | ✅ Done |

## Entry Point

`src/lib.rs` exports:
```rust
pub use engine::{EngineStats, StorageEngine, MemoryEngine, BTreeEngine};
pub use kv::{KvStore, Transactional, Persistent};
```

## Key Design Notes

- StorageEngine trait uses `where Self: Sized` for `open`/`open_memory` to be dyn compatible
- Memory engine uses BTreeMap to support ORDER BY and range scans
- BTreeEngine uses RwLock for thread safety
- scan() uses `std::collections::Bound` for range queries
- table_id parameter enables multi-table isolation

## Git Workflow

`./git.sh <message> <branch>` — commit + push

## Testing

Run `./test.sh` or manually:
```bash
cargo build && cargo test
```

## Documentation

- [_doc/v0.1.md](_doc/v0.1.md) — Current implementation status (v0.2)
- [_doc/plan.md](_doc/plan.md) — Full roadmap