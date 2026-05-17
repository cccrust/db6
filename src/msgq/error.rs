//! Message Queue Error Types

use thiserror::Error;
use crate::error::Error as DbError;

#[derive(Error, Debug)]
pub enum MsgqError {
    #[error("Queue not found: {0}")]
    QueueNotFound(String),

    #[error("Queue is empty")]
    QueueEmpty,

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Message in flight, please wait")]
    MessageInFlight,

    #[error("Invalid message format: {0}")]
    InvalidFormat(String),

    #[error("Invalid engine type: {0}")]
    InvalidEngine(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Db(#[from] DbError),
}

pub type Result<T> = std::result::Result<T, MsgqError>;