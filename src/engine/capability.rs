//! Engine capability system
//! 
//! Compile-time checks for engine features.
//! Use these traits to ensure the engine supports required operations.

use crate::engine::StorageEngine;

/// Marker trait for engines that support ORDER BY (range scans)
pub trait CanOrderBy: StorageEngine {}

/// Marker trait for engines that support JOIN operations
pub trait CanJoin: StorageEngine {}

/// Marker trait for engines that support FTS (Full-Text Search)
pub trait CanFts: StorageEngine {}

/// Marker trait for engines that support transactions
pub trait CanTransaction: StorageEngine {}

/// Marker trait for engines that support range scan (not just get)
pub trait CanScan: StorageEngine {}

/// Marker trait for engines that support batch operations
pub trait CanBatch: StorageEngine {}

/// Helper macro to implement capabilities for an engine
#[macro_export]
macro_rules! impl_capabilities {
    ($engine:ident, $( $cap:ident ),*) => {
        $(
            impl $crate::engine::capability::$cap for $engine {}
        )*
    };
}