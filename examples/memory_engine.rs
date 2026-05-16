//! Memory Engine 範例
//! 
//! 展示如何使用 MemoryEngine

use db6::engine::{StorageEngine, MemoryEngine};

fn main() {
    println!("=== Memory Engine Example ===\n");

    // 建立記憶體引擎
    let mut engine = MemoryEngine::new();

    // 引擎資訊
    println!("Engine type: {}", engine.engine_type());

    // 基本操作
    engine.put(1, b"key1", b"value1").unwrap();
    engine.put(1, b"key2", b"value2").unwrap();

    let val = engine.get(1, b"key1").unwrap();
    println!("get key1: {:?}", val);

    // 統計資訊
    let stats = engine.stats();
    println!("\nStats:");
    println!("  key_count: {}", stats.key_count);
    println!("  engine: {}", stats.engine);

    // 掃描
    let rows = engine.scan(1, b"", b"").unwrap();
    println!("\nAll rows:");
    for (k, v) in rows {
        println!("  {} -> {}", String::from_utf8_lossy(&k), String::from_utf8_lossy(&v));
    }

    // 注意：Memory engine 不支援交易
    let result = engine.begin_transaction();
    println!("\nbegin_transaction: {:?}", result);

    println!("\n=== Done ===");
}