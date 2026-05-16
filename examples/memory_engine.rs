//! Memory Engine 範例
//! 
//! 展示兩種 Memory Engine 的差異

use db6::engine::{StorageEngine, HashMemoryEngine, BTreeMemoryEngine};

fn test_hash_engine() {
    println!("\n--- HashMemoryEngine (O(1) ops, no ORDER BY) ---");
    let mut engine = HashMemoryEngine::new();

    println!("Engine type: {}", engine.engine_type());

    engine.put(1, b"key1", b"value1").unwrap();
    engine.put(1, b"key2", b"value2").unwrap();

    let val = engine.get(1, b"key1").unwrap();
    println!("get key1: {:?}", val);

    // scan returns all keys (no range support)
    let rows = engine.scan(1, b"", b"").unwrap();
    println!("scan all: {} rows", rows.len());

    // Note: ORDER BY not supported
    println!("begin_transaction: {:?}", engine.begin_transaction());
}

fn test_btree_engine() {
    println!("\n--- BTreeMemoryEngine (O(log n), supports ORDER BY) ---");
    let mut engine = BTreeMemoryEngine::new();

    println!("Engine type: {}", engine.engine_type());

    engine.put(1, b"key1", b"value1").unwrap();
    engine.put(1, b"key2", b"value2").unwrap();
    engine.put(1, b"key3", b"value3").unwrap();

    let val = engine.get(1, b"key1").unwrap();
    println!("get key1: {:?}", val);

    // scan with range
    let rows = engine.scan(1, b"key1", b"key3").unwrap();
    println!("scan key1 to key3: {} rows", rows.len());

    // ORDER BY is automatic (BTreeMap is sorted)
    let all = engine.scan(1, b"", b"").unwrap();
    println!("scan all (ordered): {:?}", all.iter().map(|(k,_)| String::from_utf8_lossy(k)).collect::<Vec<_>>());

    // Note: No transaction support
    println!("begin_transaction: {:?}", engine.begin_transaction());
}

fn main() {
    println!("=== Memory Engine Example ===\n");
    println!("Two implementations:");
    println!("  - HashMemoryEngine: Redis-like, fast O(1), no ORDER BY/scan");
    println!("  - BTreeMemoryEngine: SQLite-like, supports SQL operations\n");

    test_hash_engine();
    test_btree_engine();

    println!("\n=== Done ===");
}