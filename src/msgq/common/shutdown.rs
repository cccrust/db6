//! GracefulShutdown — 基於 tokio broadcast 的優雅關閉機制
//!
//! 類似 mini-redis 的設計：協調多個非同步任務的關閉時機。
//! 當收到關閉訊號時，所有 subscribe 的任務都會收到通知。

use std::sync::Arc;
use tokio::sync::broadcast;

/// 優雅關閉協調器
pub struct GracefulShutdown {
    /// broadcast channel 的發送端
    shutdown_tx: broadcast::Sender<()>,
}

impl GracefulShutdown {
    /// 建立一個新的關閉協調器
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self { shutdown_tx: tx }
    }

    /// 訂閱關閉訊號
    ///
    /// 每個執行緒/任務需要各自的 Receiver。
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// 觸發關閉：發送訊號給所有訂閱者
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// 檢查是否已收到關閉訊號
    ///
    /// 注意：此方法目前回傳 false（需要額外狀態追蹤）。
    pub fn is_shutdown(&self) -> bool {
        false
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for GracefulShutdown {
    fn clone(&self) -> Self {
        Self {
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 測試關閉訊號的發送與接收
    #[tokio::test]
    async fn test_shutdown_signal() {
        let gs = GracefulShutdown::new();

        let mut rx1 = gs.subscribe();
        let mut rx2 = gs.subscribe();

        gs.shutdown();

        rx1.recv().await.unwrap();
        rx2.recv().await.unwrap();
    }

    /// 測試多個訂閱者同時接收關閉訊號
    #[tokio::test]
    async fn test_multiple_subscribers() {
        let gs = GracefulShutdown::new();

        let mut receivers = Vec::new();
        for _ in 0..5 {
            receivers.push(gs.subscribe());
        }

        gs.shutdown();

        for rx in receivers.iter_mut() {
            rx.recv().await.unwrap();
        }
    }
}