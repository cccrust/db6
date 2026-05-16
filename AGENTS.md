# db6 Agent Instructions

## Build Commands

```bash
cargo build    # Build library
cargo test     # Run tests
```

## Architecture (KV-focused for v0.1)

- `src/engine/` — Storage engines: `memory.rs`, `btree.rs`, `lsm.rs`
- `src/kv/` — `KvStore`, `Transactional`, `Persistent` traits
- `src/sql/` — SQL parser/planner/executor (basic, needs more work)
- `src/fts/` — FTS stub (minimal implementation)

## Entry Point

`src/lib.rs` exports:
```rust
pub use engine::{EngineStats, StorageEngine};
pub use kv::{KvStore, Transactional, Persistent};
```

## Key Design Notes

- StorageEngine trait uses `where Self: Sized` for `open`/`open_memory` to be dyn compatible
- Memory engine uses BTreeMap to support ORDER BY and range scans
- scan() uses `std::collections::Bound` for range queries

## Git Workflow

`git.sh` automates commit + push: `./git.sh <message> <branch>`

## Development Notes

- v0.1 focuses on KV layer only (SQL is basic stub)
- See `_doc/v0.1.md` for current implementation status
- See `_doc/plan.md` for full roadmap