//! SQL 佇列型別定義
//!
//! JobResult：SQL 執行結果的枚舉
//! ResultStore：結果儲存的抽象（記憶體/自訂）
//! SqlJob：SQL 工作的描述

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// SQL 執行結果枚舉
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobResult {
    /// SELECT 查詢結果
    Select {
        columns: Vec<String>,
        rows: Vec<Vec<serde_json::Value>>,
    },
    /// INSERT 結果
    Insert { affected: u64 },
    /// UPDATE 結果
    Update { affected: u64 },
    /// DELETE 結果
    Delete { affected: u64 },
    /// CREATE TABLE 結果
    Create { table_name: String },
    /// DROP TABLE 結果
    Drop { table_name: String },
    /// 錯誤結果
    Error { message: String },
}

impl JobResult {
    /// 將結果轉換為 JSON 值
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            JobResult::Select { columns, rows } => {
                serde_json::json!({
                    "type": "select",
                    "columns": columns,
                    "rows": rows
                })
            }
            JobResult::Insert { affected } => {
                serde_json::json!({
                    "type": "insert",
                    "affected": affected
                })
            }
            JobResult::Update { affected } => {
                serde_json::json!({
                    "type": "update",
                    "affected": affected
                })
            }
            JobResult::Delete { affected } => {
                serde_json::json!({
                    "type": "delete",
                    "affected": affected
                })
            }
            JobResult::Create { table_name } => {
                serde_json::json!({
                    "type": "create",
                    "table": table_name
                })
            }
            JobResult::Drop { table_name } => {
                serde_json::json!({
                    "type": "drop",
                    "table": table_name
                })
            }
            JobResult::Error { message } => {
                serde_json::json!({
                    "type": "error",
                    "message": message
                })
            }
        }
    }

    /// 判斷是否為 SELECT 結果
    pub fn is_select(&self) -> bool {
        matches!(self, JobResult::Select { .. })
    }

    /// 判斷是否為錯誤結果
    pub fn is_error(&self) -> bool {
        matches!(self, JobResult::Error { .. })
    }

    /// 取得錯誤訊息（如果是錯誤結果）
    pub fn error_message(&self) -> Option<String> {
        match self {
            JobResult::Error { message } => Some(message.clone()),
            _ => None,
        }
    }
}

/// 結果儲存
///
/// - Memory: 基於 HashMap 的記憶體儲存
/// - Custom: 自訂儲存（實作 SqlResultStore trait）
pub enum ResultStore {
    Memory(Arc<RwLock<HashMap<String, JobResult>>>),
    Custom(Box<dyn SqlResultStore>),
}

impl Clone for ResultStore {
    fn clone(&self) -> Self {
        match self {
            ResultStore::Memory(arc) => ResultStore::Memory(arc.clone()),
            ResultStore::Custom(_) => ResultStore::Memory(Arc::new(RwLock::new(HashMap::new()))),
        }
    }
}

impl ResultStore {
    /// 建立記憶體結果儲存
    pub fn memory() -> Self {
        Self::Memory(Arc::new(RwLock::new(HashMap::new())))
    }

    /// 儲存一個 SQL 執行結果
    pub async fn store(&self, job_id: &str, result: JobResult) -> Result<(), String> {
        match self {
            ResultStore::Memory(map) => {
                let mut guard = map.write().await;
                guard.insert(job_id.to_string(), result);
                Ok(())
            }
            ResultStore::Custom(store) => store.store(job_id, result),
        }
    }

    /// 取得一個 SQL 執行結果
    pub async fn get(&self, job_id: &str) -> Result<Option<JobResult>, String> {
        match self {
            ResultStore::Memory(map) => {
                let guard = map.read().await;
                Ok(guard.get(job_id).cloned())
            }
            ResultStore::Custom(store) => store.get(job_id),
        }
    }
}

/// 自訂結果儲存 trait
pub trait SqlResultStore: Send + Sync {
    fn store(&self, job_id: &str, result: JobResult) -> Result<(), String>;
    fn get(&self, job_id: &str) -> Result<Option<JobResult>, String>;
}

/// SQL 工作描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlJob {
    pub job_id: String,
    pub sql: String,
    pub submitted_at: u64,
}

impl SqlJob {
    /// 建立新的 SQL 工作
    ///
    /// 自動產生唯一的 job_id（時間戳 + 亂數）。
    pub fn new(sql: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let job_id = format!("{}:{:08x}", now, fastrand::u32(..));

        Self {
            job_id,
            sql,
            submitted_at: now,
        }
    }

    /// 序列化為 Vec<u8>
    pub fn serialize(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| e.to_string())
    }

    /// 反序列化
    pub fn deserialize(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data).map_err(|e| e.to_string())
    }
}