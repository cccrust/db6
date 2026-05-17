//! Common utilities for msgq async modules
//!
//! Provides reusable components:
//! - ConcurrencyLimiter: Semaphore-based concurrency control
//! - GracefulShutdown: Broadcast-based graceful shutdown

mod limiter;
mod shutdown;

pub use limiter::ConcurrencyLimiter;
pub use shutdown::GracefulShutdown;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub const DEFAULT_CONCURRENCY_LIMIT: usize = 100;

pub fn new_atomic_usize(value: usize) -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(value))
}