//! Memory engine module
//! 
//! Two implementations:
//! - HashMemoryEngine: HashMap-based, O(1) ops, no ORDER BY/scan
//! - BTreeMemoryEngine: BTreeMap-based, O(log n) ops, supports ORDER BY/scan

pub mod hash;
pub mod btree;

pub use hash::HashMemoryEngine;
pub use btree::BTreeMemoryEngine;