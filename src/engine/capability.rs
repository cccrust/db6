//! 能力標記系統 (Capability Markers)
//!
//! 由於 StorageEngine trait 加上 `where Self: Sized` 限制後無法用 `Box<dyn StorageEngine>`，
//! 改用能力標記 trait 在編譯期標記引擎支援哪些功能。
//!
//! 如果引擎不支援某功能（如 JOIN），執行器在執行期回傳錯誤，而非在編譯期禁止。

use crate::engine::StorageEngine;

/// 標記引擎支援 ORDER BY 排序
///
/// 需要引擎的 scan() 能按照鍵的順序遍歷（BTreeMap、BTree Engine 原生支援）。
pub trait CanOrderBy: StorageEngine {}

/// 標記引擎支援 JOIN 操作
///
/// JOIN 需要引擎能夠在同一個 table_id 內進行多次掃描並關聯結果。
pub trait CanJoin: StorageEngine {}

/// 標記引擎支援全文搜尋（FTS）
///
/// FTS 需要引擎支援前綴掃描（prefix scan）以遍歷倒排索引。
pub trait CanFts: StorageEngine {}

/// 標記引擎支援交易（Transaction）
///
/// 交易需要引擎支援 begin/commit/rollback 語意。
pub trait CanTransaction: StorageEngine {}

/// 標記引擎支援範圍掃描（Range Scan）
///
/// 所有引擎都應支援 scan()，此標記用於語意區分。
pub trait CanScan: StorageEngine {}

/// 標記引擎支援批次操作
///
/// batch_put 與 range_delete 的最佳化實作。
pub trait CanBatch: StorageEngine {}

/// 標記引擎支援 GROUP BY 與聚合函數
///
/// 聚合函數包括 COUNT, SUM, AVG, MIN, MAX 等。
pub trait CanGroupBy: StorageEngine {}

/// 快速實作能力標記的輔助巨集
///
/// 接受引擎型別與一或多個能力 trait，自動產生對應的 impl 區塊。
/// 例如 `impl_capabilities!(HashMemoryEngine, CanOrderBy, CanScan)` 會展開為：
/// `impl CanOrderBy for HashMemoryEngine {}` 和 `impl CanScan for HashMemoryEngine {}`。
#[macro_export]
macro_rules! impl_capabilities {
    ($engine:ident, $( $cap:ident ),*) => {
        $(
            impl $crate::engine::capability::$cap for $engine {}
        )*
    };
}
