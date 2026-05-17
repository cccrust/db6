//! Async SQL Executor - 基於 AsyncQueue + tokio

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::msgq::AsyncQueue;
use super::types::{JobResult, ResultStore, SqlJob};

pub struct AsyncSqlExecutor {
    queue: AsyncQueue,
    results: ResultStore,
}

impl AsyncSqlExecutor {
    pub fn new(result_store: ResultStore) -> Self {
        Self {
            queue: AsyncQueue::new("sql"),
            results: result_store,
        }
    }

    pub async fn execute(&mut self, sql: &str) -> Result<String, String> {
        let job = SqlJob::new(sql.to_string());
        let job_id = job.job_id.clone();
        let payload = job.serialize()?;

        self.queue.enqueue(payload, 30).await?;

        Ok(job_id)
    }

    pub async fn poll(&self, job_id: &str) -> Result<JobResult, String> {
        match self.results.get(job_id).await {
            Ok(Some(result)) => Ok(result),
            Ok(None) => Err("pending".to_string()),
            Err(e) => Err(e),
        }
    }

    pub async fn execute_and_wait(
        &mut self,
        sql: &str,
        timeout_ms: u64,
    ) -> Result<JobResult, String> {
        let job_id = self.execute(sql).await?;

        let start = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let deadline = start + timeout_ms;

        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            if now >= deadline {
                return Err("timeout".to_string());
            }

            match self.poll(&job_id).await {
                Ok(result) => return Ok(result),
                Err(e) if e == "pending" => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn spawn_workers(&self, count: usize) {
        for _ in 0..count {
            let queue = self.queue.clone();
            let results = self.results.clone();

            tokio::spawn(async move {
                worker_loop(queue, results).await;
            });
        }
    }

    pub async fn queue_length(&self) -> Result<usize, String> {
        self.queue.length().await
    }
}

impl Clone for AsyncSqlExecutor {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            results: self.results.clone(),
        }
    }
}

async fn worker_loop(mut queue: AsyncQueue, results: ResultStore) {
    loop {
        match queue.dequeue(0).await {
            Ok(Some(msg)) => {
                let job = match SqlJob::deserialize(&msg.payload) {
                    Ok(j) => j,
                    Err(e) => {
                        eprintln!("Failed to deserialize job: {}", e);
                        let _ = queue.nack(&msg.id).await;
                        continue;
                    }
                };

                let result = execute_sql(&job.sql).await;

                if let Err(e) = results.store(&job.job_id, result).await {
                    eprintln!("Failed to store result: {}", e);
                }

                let _ = queue.ack(&msg.id).await;
            }
            Ok(None) => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(e) => {
                eprintln!("Worker error: {}", e);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

async fn execute_sql(sql: &str) -> JobResult {
    // TODO: 整合現有的 SQL executor
    // 目前回傳模擬結果
    let sql_lower = sql.trim().to_lowercase();

    if sql_lower.starts_with("select") {
        JobResult::Select {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec![serde_json::json!(1), serde_json::json!("Alice")],
                vec![serde_json::json!(2), serde_json::json!("Bob")],
            ],
        }
    } else if sql_lower.starts_with("insert") {
        JobResult::Insert { affected: 1 }
    } else if sql_lower.starts_with("update") {
        JobResult::Update { affected: 1 }
    } else if sql_lower.starts_with("delete") {
        JobResult::Delete { affected: 1 }
    } else if sql_lower.starts_with("create") {
        let table = sql_lower
            .split_whitespace()
            .nth(2)
            .unwrap_or("unknown")
            .trim_end_matches(';');
        JobResult::Create { table_name: table.to_string() }
    } else if sql_lower.starts_with("drop") {
        let table = sql_lower
            .split_whitespace()
            .nth(2)
            .unwrap_or("unknown")
            .trim_end_matches(';');
        JobResult::Drop { table_name: table.to_string() }
    } else {
        JobResult::Error {
            message: "Unknown SQL type".to_string(),
        }
    }
}