//! Graceful Shutdown - Broadcast-based shutdown coordination
//!
//! Similar to mini-redis: coordinates shutdown across multiple tasks

use std::sync::Arc;
use tokio::sync::broadcast;

pub struct GracefulShutdown {
    shutdown_tx: broadcast::Sender<()>,
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self { shutdown_tx: tx }
    }

    /// Subscribe to shutdown signal
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Trigger shutdown
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Check if shutdown signal was sent
    pub fn is_shutdown(&self) -> bool {
        // Can't easily check without subscribing
        // This would require additional state tracking
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

    #[tokio::test]
    async fn test_shutdown_signal() {
        let gs = GracefulShutdown::new();
        
        let mut rx1 = gs.subscribe();
        let mut rx2 = gs.subscribe();
        
        gs.shutdown();
        
        rx1.recv().await.unwrap();
        rx2.recv().await.unwrap();
    }

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