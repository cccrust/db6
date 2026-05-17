//! 佇列監控指標與健康檢查

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 佇列監控指標
///
/// 所有指標使用 AtomicU64 確保執行緒安全。
/// - `enqueued_total`: 累計入隊總數
/// - `dequeued_total`: 累計出隊總數
/// - `acked_total`: 累計確認總數
/// - `nacked_total`: 累計拒絕總數
/// - `in_flight`: 正在處理中的訊息數
/// - `queue_depth`: 佇列深度
#[derive(Debug, Clone)]
pub struct QueueMetrics {
    pub enqueued_total: Arc<AtomicU64>,
    pub dequeued_total: Arc<AtomicU64>,
    pub acked_total: Arc<AtomicU64>,
    pub nacked_total: Arc<AtomicU64>,
    pub in_flight: Arc<AtomicU64>,
    pub queue_depth: Arc<AtomicU64>,
}

impl QueueMetrics {
    /// 建立新的指標實例
    pub fn new() -> Self {
        Self {
            enqueued_total: Arc::new(AtomicU64::new(0)),
            dequeued_total: Arc::new(AtomicU64::new(0)),
            acked_total: Arc::new(AtomicU64::new(0)),
            nacked_total: Arc::new(AtomicU64::new(0)),
            in_flight: Arc::new(AtomicU64::new(0)),
            queue_depth: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 入隊計數 +1
    pub fn enqueued_inc(&self) {
        self.enqueued_total.fetch_add(1, Ordering::Relaxed);
    }

    /// 出隊計數 +1
    pub fn dequeued_inc(&self) {
        self.dequeued_total.fetch_add(1, Ordering::Relaxed);
    }

    /// 確認計數 +1
    pub fn acked_inc(&self) {
        self.acked_total.fetch_add(1, Ordering::Relaxed);
    }

    /// 拒絕計數 +1
    pub fn nacked_inc(&self) {
        self.nacked_total.fetch_add(1, Ordering::Relaxed);
    }

    /// 設定飛行中訊息數
    pub fn in_flight_set(&self, count: u64) {
        self.in_flight.store(count, Ordering::Relaxed);
    }

    /// 設定佇列深度
    pub fn queue_depth_set(&self, depth: u64) {
        self.queue_depth.store(depth, Ordering::Relaxed);
    }
}

impl Default for QueueMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 健康狀態列舉
#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

pub struct QueueHealth {
    pub status: HealthStatus,
    pub details: std::collections::HashMap<String, String>,
}

impl QueueHealth {
    pub fn healthy() -> Self {
        Self {
            status: HealthStatus::Healthy,
            details: std::collections::HashMap::new(),
        }
    }

    pub fn degraded(reason: &str) -> Self {
        let mut details = std::collections::HashMap::new();
        details.insert("reason".to_string(), reason.to_string());
        Self {
            status: HealthStatus::Degraded(reason.to_string()),
            details,
        }
    }

    pub fn unhealthy(reason: &str) -> Self {
        let mut details = std::collections::HashMap::new();
        details.insert("reason".to_string(), reason.to_string());
        Self {
            status: HealthStatus::Unhealthy(reason.to_string()),
            details,
        }
    }
}
