//! 引擎能力查詢系統 (Capability System)
//!
//! 透過 Marker Trait 在編譯期檢查引擎是否支援特定功能。
//! 例如：HashMemoryEngine 不實作 CanOrderBy，因此在記憶體 HashMap 引擎上執行
//! ORDER BY 查詢時，編譯器就會報錯，而不是在執行期才發現不支援。
//!
//! 這種設計稱作「編譯期能力宣告」(Compile-time Capability Declaration)，
//! 相較於執行期錯誤檢查，能在開發早期發現問題。

use crate::engine::StorageEngine;

/// 標記引擎支援 ORDER BY（即支援範圍掃描）
///
/// 實作此 trait 的引擎（BTreeMemoryEngine, BTreeEngine, LsmEngine）
/// 可以對查詢結果進行排序。
pub trait CanOrderBy: StorageEngine {}

/// 標記引擎支援 JOIN 操作
///
/// 目前僅 BTreeMemoryEngine 實作此功能。
pub trait CanJoin: StorageEngine {}

/// 標記引擎支援全文搜尋 (FTS)
pub trait CanFts: StorageEngine {}

/// 標記引擎支援交易 (BEGIN/COMMIT/ROLLBACK)
///
/// 磁碟引擎（BTreeEngine, LsmEngine）支援交易，
/// 記憶體引擎目前不支援。
pub trait CanTransaction: StorageEngine {}

/// 標記引擎支援範圍掃描（不只是單點查詢）
///
/// HashMemoryEngine 不實作此 trait，因為 HashMap 不支援範圍掃描。
pub trait CanScan: StorageEngine {}

/// 標記引擎支援批量操作 (batch_put, range_delete)
pub trait CanBatch: StorageEngine {}

/// 標記引擎支援 GROUP BY 與聚合函數
///
/// 聚合函數包括 COUNT, SUM, AVG, MIN, MAX 等。
pub trait CanGroupBy: StorageEngine {}

/// 快速實作能力標記的輔助巨集
///
/// 範例：
/// ```ignore
/// impl_capabilities!(BTreeMemoryEngine, CanOrderBy, CanScan, CanBatch);
/// ```
/// 上述巨集展開為：
/// ```ignore
/// impl CanOrderBy for BTreeMemoryEngine {}
/// impl CanScan for BTreeMemoryEngine {}
/// impl CanBatch for BTreeMemoryEngine {}
/// ```
#[macro_export]
macro_rules! impl_capabilities {
    ($engine:ident, $( $cap:ident ),*) => {
        $(
            impl $crate::engine::capability::$cap for $engine {}
        )*
    };
}