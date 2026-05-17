//! 非同步訊息佇列子模組
//!
//! 經過 v4.11 重構後從單一檔案拆分為多個關注點分離的子模組：
//!
//! - `config.rs`: 佇列設定與重試策略
//! - `queue.rs`: 核心佇列實作 (tokio Notify)
//! - `metrics.rs`: 監控指標與健康檢查
//! - `stream.rs`: tokio Stream 介面
//! - `exactly.rs`: 恰好一次傳遞 (Exactly-Once)
//! - `facade.rs`: 工廠入口 (AsyncMsgq)

pub mod config;
pub mod metrics;
pub mod queue;
pub mod stream;
pub mod facade;
pub mod exactly;

// 為向後相容，將 SyncQueueMessage 別名為 AsyncQueueMessage
pub use crate::msgq::SyncQueueMessage as AsyncQueueMessage;

pub use config::{AsyncQueueConfig, RetryConfig, with_retry};
pub use metrics::{QueueMetrics, HealthStatus, QueueHealth};
pub use queue::AsyncQueue;
pub use stream::AsyncQueueStream;
pub use facade::AsyncMsgq;
pub use exactly::ExactlyOnceQueue;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::KvEngine;
    use std::sync::Arc;

    fn get_engine() -> Arc<std::sync::RwLock<KvEngine>> {
        Arc::new(std::sync::RwLock::new(KvEngine::new("memory").unwrap()))
    }

    /// 測試非同步佇列的基本操作：enqueue → dequeue → ack
    #[tokio::test]
    async fn test_async_queue_basic() {
        let engine = get_engine();
        let mut q = AsyncQueue::new("test", engine);

        let id = q.enqueue(b"hello".to_vec(), 30).await.unwrap();
        assert!(!id.is_empty());

        assert_eq!(q.length().await.unwrap(), 1);

        let msg = q.dequeue(0).await.unwrap().unwrap();
        assert_eq!(msg.payload, b"hello");

        q.ack(&msg.id).await.unwrap();
        assert_eq!(q.length().await.unwrap(), 0);
    }

    /// 測試 Nack（拒絕處理）使訊息重新可見且 delivery_count 增加
    #[tokio::test]
    async fn test_async_queue_nack() {
        let engine = get_engine();
        let mut q = AsyncQueue::new("test", engine);

        q.enqueue(b"msg".to_vec(), 30).await.unwrap();

        let msg = q.dequeue(0).await.unwrap().unwrap();
        assert_eq!(msg.delivery_count, 1);

        q.nack(&msg.id).await.unwrap();

        let msg2 = q.dequeue(0).await.unwrap().unwrap();
        assert_eq!(msg2.delivery_count, 2);
    }

    /// 測試清空佇列
    #[tokio::test]
    async fn test_async_queue_purge() {
        let engine = get_engine();
        let mut q = AsyncQueue::new("test", engine);

        q.enqueue(b"msg1".to_vec(), 30).await.unwrap();
        q.enqueue(b"msg2".to_vec(), 30).await.unwrap();

        q.purge().await.unwrap();

        assert_eq!(q.length().await.unwrap(), 0);
    }
}
