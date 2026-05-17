//! Concurrency Limiter - Semaphore-based concurrency control
//!
//! Similar to mini-redis: limits concurrent async operations

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    limit: Arc<AtomicUsize>,
}

impl ConcurrencyLimiter {
    pub fn new(limit: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
            limit: Arc::new(AtomicUsize::new(limit)),
        }
    }

    pub fn with_limit(limit: usize) -> Self {
        Self::new(limit)
    }

    pub async fn acquire(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        self.semaphore.clone().acquire_owned().await.map_err(|e| e.to_string())
    }

    pub fn try_acquire(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.semaphore.clone().try_acquire_owned().ok()
    }

    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }

    pub fn limit(&self) -> usize {
        self.limit.load(Ordering::Relaxed)
    }

    pub fn set_limit(&self, new_limit: usize) {
        self.limit.store(new_limit, Ordering::Relaxed);
    }
}

impl Clone for ConcurrencyLimiter {
    fn clone(&self) -> Self {
        Self {
            semaphore: self.semaphore.clone(),
            limit: self.limit.clone(),
        }
    }
}