# db6

Unified database with pluggable storage engines (Memory/BTree/LSM) + KV + FTS5

## 快速開始

```bash
cargo build    # 編譯
cargo test     # 測試
```

## 版本策略

- **v0.x**: 專注 KV 層（StorageEngine trait + 三種引擎）
- **v1.0+**: 加入 SQL 層

## 儲存引擎

| 引擎 | 特性 | 適用場景 |
|------|------|---------|
| Memory | BTreeMap，記憶體純 KV | 快速實驗、高效能快取 |
| BTree | 磁碟 BTree，完整交易 | SQLite 相容 |
| LSM | LSM-tree，高寫入量 | 寫優化場景 |

## 核心 API

```rust
use db6::{StorageEngine, EngineStats};

let engine = MemoryEngine::open_memory();
engine.put(1, b"key", b"value")?;
let value = engine.get(1, b"key")?;
let rows = engine.scan(1, b"", b"")?;
```

## 目錄結構

```
src/
├── engine/       # 儲存引擎 (memory, btree, lsm)
├── kv/           # KvStore trait
├── sql/          # SQL 層 (v1.0 才加入)
└── fts/          # FTS5 (基於 KV 介面)
```

## 相關專案

- [sql6](https://github.com/ccc/sp6) — SQL 實作來源
- [lsm5](https://github.com/ccc/lsm5) — LSM 實作來源
- [btree6](https://github.com/ccc/btree6) — BTree 實作來源

## 開發計畫

See [_doc/plan.md](_doc/plan.md) for full roadmap.