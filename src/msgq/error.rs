//! 訊息佇列錯誤型別
//!
//! 使用 thiserror crate 定義訊息佇列特有的錯誤 MsgqError。
//! 與底層 db6 Error 分離，提供更清晰的錯誤語意。

use thiserror::Error;
use crate::error::Error as DbError;

/// 訊息佇列錯誤
#[derive(Error, Debug)]
pub enum MsgqError {
    /// 佇列不存在
    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    /// 佇列為空
    #[error("Queue is empty")]
    QueueEmpty,

    /// 訊息不存在
    #[error("Message not found: {0}")]
    MessageNotFound(String),

    /// 訊息正在處理中
    #[error("Message in flight, please wait")]
    MessageInFlight,

    /// 無效的訊息格式
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),

    /// 無效的引擎類型
    #[error("Invalid engine type: {0}")]
    InvalidEngine(String),

    /// 無效的操作
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// IO 錯誤
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化錯誤
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// 資料庫錯誤
    #[error("Database error: {0}")]
    Db(#[from] DbError),
}

/// Result 別名
pub type Result<T> = std::result::Result<T, MsgqError>;
