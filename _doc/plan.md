# db6 開發計劃

## 1. 專案目標

db6 是一個統一的資料庫框架，同時支援三種儲存引擎：

| 引擎 | 特性 | 適用場景 |
|------|------|---------|
| **Memory** | BTreeMap，記憶體純 KV | 快速實驗、高效能快取 |
| **BTree** | 移植自 sql6 的 pager/btree | 全功能 SQL（SQLite 相容語法） |
| **LSM** | 移植自 lsm5 | 高寫入量、寫優化場景 |

SQL API 移植自 sql6，但會根據引擎能力限制功能：
- BTree：支援完整 SQLite 語法
- LSM：不支援 JOIN、複雜 ORDER BY、複合事務

---

## 2. 核心架構

```
db6/
├── src/
│   ├── lib.rs                      # 統一出口
│   ├── engine/
│   │   ├── mod.rs                  # StorageEngine trait（核心抽象）
│   │   ├── lsm.rs                  # LSM engine
│   │   ├── btree.rs                # BTree engine
│   │   └── memory.rs               # Memory engine (BTreeMap)
│   ├── kv/
│   │   └── mod.rs                  # KvStore trait impl for all engines
│   ├── sql/
│   │   ├── parser/                 # SQL parser（移植自 sql6）
│   │   │   ├── lexer.rs
│   │   │   ├── ast.rs
│   │   │   └── parser.rs
│   │   ├── planner/                # 查詢規劃（移植自 sql6）
│   │   │   ├── plan.rs
│   │   │   ├── planner.rs
│   │   │   └── constraints.rs
│   │   └── executor/               # 執行引擎（移植自 sql6）
│   │       ├── executor.rs
│   │       └── transaction.rs
│   ├── catalog/                    # 系統目錄（移植自 sql6）
│   │   ├── catalog.rs
│   │   └── meta.rs
│   ├── table/                      # table/row/schema（移植自 sql6）
│   │   ├── table.rs
│   │   ├── row.rs
│   │   └── schema.rs
│   └── fts/                        # FTS5 全文檢索（基於 KV 介面）
│       ├── tokenizer.rs             # CJK 分詞器
│       ├── index.rs                 # 倒排索引
│       ├── fts_table.rs             # FTS table wrapper
│       └── mod.rs                   # 統一出口
└── tests/
    ├── integration_tests.rs
    └── fts_tests.rs                 # FTS 專屬測試
```

---

## 3. 核心抽象：StorageEngine trait

所有引擎必須實作此 trait，作為 SQL 層與底層儲存的橋樑：

```rust
pub trait StorageEngine {
    /// 開啟或建立磁碟資料庫
    fn open(path: &Path) -> Result<Box<dyn StorageEngine>>;

    /// 建立記憶體模式資料庫
    fn open_memory() -> Box<dyn StorageEngine>;

    /// 引擎類型名稱
    fn engine_type(&self) -> &'static str;

    // ── 基本 KV 操作 ──────────────────────────────────────────

    /// 讀取一筆（table_id 用於多 table 隔離）
    fn get(&self, table_id: u32, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// 寫入或更新一筆
    fn put(&mut self, table_id: u32, key: &[u8], value: &[u8]) -> Result<()>;

    /// 刪除（tombstone）
    fn delete(&mut self, table_id: u32, key: &[u8]) -> Result<()>;

    /// 範圍掃描 [start, end)
    fn scan(&self, table_id: u32, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;

    // ── 持久化 ────────────────────────────────────────────────

    /// 將記憶體資料刷到磁碟
    fn flush(&mut self) -> Result<()>;

    /// fsync確保資料落盤
    fn sync(&mut self) -> Result<()>;

    // ── 交易 ─────────────────────────────────────────────────

    /// 開始交易（BTree 完整支援，LSM 有限支援，Memory 不支援）
    fn begin_transaction(&mut self) -> Result<()>;

    /// 提交交易
    fn commit_transaction(&mut self) -> Result<()>;

    /// 回滾交易
    fn rollback_transaction(&mut self) -> Result<()>;

    /// 目前是否有活躍交易
    fn has_transaction(&self) -> bool;

    // ── 可觀測性 ─────────────────────────────────────────────

    /// 取得統計資訊
    fn stats(&self) -> EngineStats;
}
```

### StorageEngine 與 SQL 層的對接

sql6 原有的 `pager/cache.rs` 是基於 btree 的。移植策略：

```
SQL 層 (parser → planner → executor)
    │
    └── calls pager API
            │
            └── pager calls StorageEngine trait ← 所有引擎共用此介面
                    ├── Memory engine
                    ├── BTree engine (移植 sql6 pager/btree)
                    └── LSM engine (移植 lsm5)
```

**不需要修改 parser、planner、executor**。只需實作 `StorageEngine` trait 並讓 pager 使用它。

---

## 4. 三種引擎實作規劃

### 4.1 Memory Engine（最早完成）

```rust
// 結構：BTreeMap<TableId, BTreeMap<Key, Value>>
pub struct MemoryEngine {
    tables: HashMap<u32, BTreeMap<Vec<u8>, Vec<u8>>>,
}
```

- **open_memory()** — 直接建立，無磁碟
- **所有操作** — O(log n) BTreeMap 查詢
- **交易** — 不支援（`Error::NotSupported`）
- **flush/sync** — no-op（純記憶體）
- **SQL 限制** — 無（全部支援）

### 4.2 BTree Engine（移植 sql6）

移植來源：`sql6/src/pager/` + `sql6/src/btree/`

- Page 大小：8KB
- 支援 MVCC、並發讀寫
- 支援完整交易
- StorageEngine adapter 層將 `pager` 包裝為 `StorageEngine`

### 4.3 LSM Engine（移植 lsm5）

移植來源：`lsm5/src/` 的核心模組

- WAL + MemTable + SSTable + Compaction
- 寫入路徑：WAL append → MemTable → flush to L0 → compaction to L1..LN
- 讀取路徑：MemTable → L0..LN SSTables
- **SQL 限制**：
  - 不支援 `JOIN`
  - 不支援 `ORDER BY`（代價極高）
  - 交易只支援單 table 操作

---

## 5. 移植 sql6 的模組

| sql6 模組 | 移植到 db6 | 說明 |
|-----------|-----------|------|
| `parser/lexer.rs` | `sql/parser/lexer.rs` | 直接複製 |
| `parser/ast.rs` | `sql/parser/ast.rs` | 直接複製 |
| `parser/parser.rs` | `sql/parser/parser.rs` | 直接複製 |
| `planner/plan.rs` | `sql/planner/plan.rs` | 直接複製 |
| `planner/planner.rs` | `sql/planner/planner.rs` | 直接複製 |
| `planner/constraints.rs` | `sql/planner/constraints.rs` | 直接複製 |
| `planner/executor.rs` | `sql/executor/executor.rs` | 需修改 pager 呼叫為 StorageEngine |
| `planner/transaction.rs` | `sql/executor/transaction.rs` | 直接複製 |
| `catalog/catalog.rs` | `catalog/catalog.rs` | 直接複製 |
| `catalog/meta.rs` | `catalog/meta.rs` | 直接複製 |
| `table/table.rs` | `table/table.rs` | 直接複製 |
| `table/row.rs` | `table/row.rs` | 直接複製 |
| `table/schema.rs` | `table/schema.rs` | 直接複製 |
| `pager/cache.rs` | `engine/adapter/pager_cache.rs` | 改為使用 StorageEngine |
| `pager/storage.rs` | `engine/btree/storage.rs` | 移植 |
| `btree/tree.rs` | `engine/btree/tree.rs` | 移植 |
| `fts/` | `fts/` | 直接複製 |

---

## 6. 版本里程碑

### v1.0 — 最小可用系統（目標）

```
✅ Memory engine 完成
✅ BTree engine 完成
✅ SQL parser + planner + executor 移植完成
✅ KV API 完成（KvStore trait for all engines）
✅ 統一測試：同一份 SQL 在三個引擎都能跑
✅ SELECT / INSERT / UPDATE / DELETE
```

### v1.1 — LSM Engine

```
✅ LSM engine 移植完成
✅ KvStore for LSM
✅ SQL limited mode（無 JOIN、複雜 ORDER BY）
✅ 跨引擎 benchmark
```

### v1.2 — 完整功能

```
✅ FTS 全文檢索（移植 sql6 fts/）
✅ 統一 catalog
✅ WAL 統一（WAL log replay）
✅ CLI / REPL
```

---

## 7. 實作順序

### Phase 1：框架搭建（db6 骨架）

```
1. 建立 src/lib.rs，export 所有模組
2. 建立 src/engine/mod.rs，定義 StorageEngine trait
3. 建立 src/sql/parser/ 移植（lexer, ast, parser）
4. 建立 src/sql/planner/ 移植（plan, planner, constraints）
5. 建立 src/sql/executor/ 移植（executor, transaction）
6. 建立測試：SQL 在 mock engine 上跑通
```

### Phase 2：Memory Engine

```
1. src/engine/memory.rs — MemoryEngine 實作 StorageEngine
2. 測試：Memory engine 完整跑通 SQL
3. src/kv/mod.rs — KvStore impl for Memory
```

### Phase 3：BTree Engine

```
1. 移植 sql6/src/pager/ → src/engine/btree/
2. 移植 sql6/src/btree/ → src/engine/btree/
3. src/engine/btree.rs — adapter 實作 StorageEngine
4. 測試：BTree engine 跑通 SQL
5. KvStore impl for BTree
```

### Phase 4：LSM Engine

```
1. 移植 lsm5/src/ → src/engine/lsm/
2. src/engine/lsm.rs — adapter 實作 StorageEngine
3. SQL limitation 標註（不支援的功能）
4. KvStore impl for LSM
```

### Phase 5：整合

```
1. catalog 移植
2. table/row/schema 移植
3. FTS 移植
4. 統一 CLI/REPL
5. 跨引擎整合測試
```

---

## 8. 關鍵設計決策

### 8.1 Engine trait 的 get 接受 table_id

為什麼？因為 BTree engine 的 pager 是多 table 隔離的，scan 需要針對特定 table。

### 8.2 LSM engine 的 SQL 限制在 Planner 層實作

在 `planner/planner.rs` 中，根據 engine type 判斷是否支援該 query，若不支援則回錯誤。

### 8.3 Memory engine 使用 BTreeMap 而非 HashMap

BTreeMap 的優點：
- 支援 `ORDER BY`（SQL 必要）
- 支援範圍查詢 `scan`
- 記憶體佔用合理

### 8.4 不修改 sql6 的 parser/planner/executor

sql6 的分層設計很好，只需要：
- 改動 `executor.rs` 中呼叫 pager 的部分，改為呼叫 `StorageEngine` trait
- 其他全部直接複製，不改動

---

## 9. 依賴

```toml
[dependencies]
thiserror = "2.0"           # Error enum
serde = { version = "1.0", features = ["derive"] }  # 序列化
zstd = "0.13"               # LSM 壓縮（可選，feature gate）

[dev-dependencies]
tempfile = "3.14"          # 測試用目錄
```

---

## 10. 測試策略

```rust
// 同一份測試在三個引擎都跑
fn test_sql_all_engines(sql: &str) {
    let sqls = [
        ("memory", || Box::new(db6::engine::Memory::open_memory()) as Box<dyn StorageEngine>),
        ("btree", || Box::new(db6::engine::BTree::open(path).unwrap())),
        ("lsm", || Box::new(db6::engine::Lsm::open(path).unwrap())),
    ];

    for (name, engine_fn) in sqls {
        let engine = engine_fn();
        let executor = Executor::new(engine);
        executor.execute(sql).unwrap();
    }
}
```

LSM 專屬測試（預期失敗）：
```rust
#[test]
#[should_panic(expected = "JOIN not supported with LSM engine")]
fn lsm_join_not_supported() { ... }
```

---

## 11. FTS5 全文檢索實作（基於 KV 介面）

### 11.1 設計原則

FTS 以 `StorageEngine` 的 KV 介面（`get/put/scan`）為基礎，**不依賴任何特定引擎**，可同時支援 Memory / BTree / LSM。

### 11.2 倒排索引 KV 佈局

```
FTS table id=3, term="hello", doc_ids=[1, 5, 9]:
  put(3, b"FTS:3:hello", varint_encode([1, 5, 9]))

FTS table id=3, term="world", doc_ids=[2, 5]:
  put(3, b"FTS:3:world", varint_encode([2, 5]))

掃描某個 FTS table 的所有 term:
  scan(3, b"FTS:3:", b"FTS:3:~")
```

### 11.3 支援的 SQL 語法（移植 sql6 FTS5）

```sql
-- 建立 FTS5 virtual table
CREATE VIRTUAL TABLE articles USING fts5(title, content, tokenize='cjk');

-- 全文檢索
SELECT * FROM articles WHERE articles MATCH 'hello AND world';

-- 短語匹配
SELECT * FROM articles WHERE articles MATCH '"資料庫系統"';

-- 前綴匹配
SELECT * FROM articles WHERE articles MATCH '資料*';

-- 布林組合
SELECT * FROM articles WHERE articles MATCH 'hello OR world NOT sql';

-- BM25 排序（Memory/BTree 支援，LSM 不支援）
SELECT * FROM articles WHERE articles MATCH 'database' ORDER BY rank;
```

### 11.4 各引擎 SQL 限制

| SQL 功能 | Memory | BTree | LSM |
|---------|--------|-------|-----|
| `MATCH` | ✅ | ✅ | ✅ |
| 前綴 `*` | ✅ | ✅ | ✅ |
| `AND / OR / NOT` | ✅ | ✅ | ✅ |
| `ORDER BY rank` | ✅ | ✅ | ❌ |
| `ORDER BY bm25()` | ✅ | ✅ | ❌ |
| `snippet()` | ✅ | ✅ | ✅ |

### 11.5 分詞策略

| 語言 | 方法 | 範例 |
|------|------|------|
| CJK（中/日/韓） | 雙元分詞（bigram）| "資料庫" → ["資料", "料庫"] |
| English | whitespace + lowercase | "Hello World" → ["hello", "world"] |
| Unicode61 | UAX#29 標準 | 符合 Unicode 標準 |

### 11.6 移植 sql6 FTS 的模組對照

| sql6 檔案 | db6 目的 | 移植策略 |
|-----------|---------|---------|
| `fts/tokenizer.rs` | CJK 分詞 + 自訂 tokenizer | 直接複製 |
| `fts/index.rs` | 倒排索引實作 | 已是 generic，幾乎不改 |
| `fts/fts_table.rs` | FTS table wrapper | 需改 StorageEngine adapter |
| `fts/mod.rs` | 統一出口 | 直接複製 |

### 11.7 FTS 實作順序（Phase 6）

```
1. 移植 fts/tokenizer.rs（保持不變）
2. 移植 fts/index.rs（FtsIndex 已是 generic，直接複製）
3. 移植 fts/fts_table.rs（整合 StorageEngine）
4. 修改 sql parser：支援 CREATE VIRTUAL TABLE ... USING fts5
5. 修改 sql planner：FtsMatch node
6. 修改 sql executor：呼叫 FtsTable MATCH
7. 測試：三個 engine 都能跑 FTS
8. 測試：CjkTokenizer 分詞正確性（"資料庫系統" → ["資料","料庫","庫系","系統"]）
9. 測試：AND/OR/NOT/前綴布林查詢
```

---

## 12. 版本里程碑（更新）

### v1.0 — 最小可用系統

```
✅ Memory engine 完成（StorageEngine impl）
✅ BTree engine 完成（移植 sql6 pager/btree）
✅ SQL parser + planner + executor 移植完成
✅ SELECT / INSERT / UPDATE / DELETE
✅ 統一測試：同一份 SQL 在三個引擎都能跑
```

### v1.1 — LSM Engine

```
✅ LSM engine 完成（移植 lsm5）
✅ KvStore for all engines
✅ SQL limited mode（LSM 不支援 JOIN、複雜 ORDER BY）
✅ 跨引擎 benchmark
```

### v1.2 — FTS + 完整功能

```
✅ FTS5 全文檢索（基於 KV 介面，三引擎通用）
✅ CJK 分詞（雙元分詞，支援中文/日文/韓文）
✅ BM25 排序（Memory/BTree）
✅ 統一 catalog
✅ CLI / REPL
```