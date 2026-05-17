//! 統一的錯誤型別系統
//!
//! 定義整個 db6 專案共用的錯誤型別 `Error` 與 `Result`，讓所有模組使用一致的錯誤處理方式。
//! 使用 `thiserror` crate 自動產生 Display 與 Error 實作。

use thiserror::Error;

/// db6 全域錯誤型別
///
/// 涵蓋 IO、鍵值、交易、設定、引擎、SQL、FTS 等各層面的錯誤。
/// 每個變體透過 `#[error("...")]` 屬性定義人類可讀的錯誤訊息。
#[derive(Debug, Clone, Error)]
pub enum Error {
    /// IO 操作失敗，例如檔案讀寫錯誤
    #[error("IO error: {0}")]
    Io(String),

    /// 指定的鍵在 KV 儲存中不存在
    #[error("key not found")]
    KeyNotFound,

    /// 請求的操作未被支援（例如掃描不支援的引擎）
    #[error("operation not supported: {0}")]
    NotSupported(String),

    /// 資料損毀偵測（例如 BTree 節點格式錯誤、LSM SSTable 校驗失敗）
    #[error("data corruption: {0}")]
    Corruption(String),

    /// 交易相關錯誤（例如尚未開始交易）
    #[error("transaction error: {0}")]
    TransactionError(String),

    /// 交易操作失敗（如提交或回滾時出錯）
    #[error("transaction: {0}")]
    Transaction(String),

    /// 引擎或系統設定不正確
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// 不存在的儲存引擎類型
    #[error("invalid engine: {0}")]
    InvalidEngine(String),

    /// SQL 語法或執行錯誤
    #[error("SQL error: {0}")]
    Sql(String),

    /// 全文搜尋操作錯誤
    #[error("FTS error: {0}")]
    Fts(String),
}

/// 專案內部統一的 Result 別名
///
/// 簡化寫法：`Result<T>` 等同於 `std::result::Result<T, Error>`
pub type Result<T> = std::result::Result<T, Error>;

/// 將標準 IO 錯誤自動轉換為 db6 Error
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}