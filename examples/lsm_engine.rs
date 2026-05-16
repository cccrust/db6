//! LSM Engine 範例
//! 
//! 展示如何使用 LsmEngine

use db6::engine::{StorageEngine, LsmEngine};

fn main() {
    println!("=== LSM Engine Example ===\n");

    // 建立 LSM 引擎
    let mut engine = LsmEngine::new();

    // 引擎資訊
    println!("Engine type: {}", engine.engine_type());

    // 基本操作 (LSM 只支援 table_id = 1)
    engine.put(1, b"key1", b"value1").unwrap();
    engine.put(1, b"key2", b"value2").unwrap();
    engine.put(1, b"key3", b"value3").unwrap();

    let val = engine.get(1, b"key1").unwrap();
    println!("get key1: {:?}", val);

    // 掃描
    let rows = engine.scan(1, b"", b"").unwrap();
    println!("\nscan all:");
    for (k, v) in rows {
        println!("  {} -> {}", String::from_utf8_lossy(&k), String::from_utf8_lossy(&v));
    }

    // 交易支援
    println!("\n--- Transaction Test ---");
    engine.begin_transaction().unwrap();
    println!("begin_transaction: OK");

    engine.put(1, b"key4", b"value4").unwrap();
    let val = engine.get(1, b"key4").unwrap();
    println!("during tx, get key4: {:?}", val);

    engine.commit_transaction().unwrap();
    println!("commit_transaction: OK");

    let val = engine.get(1, b"key4").unwrap();
    println!("after commit, get key4: {:?}", val);

    // 統計
    let stats = engine.stats();
    println!("\nStats:");
    println!("  key_count: {}", stats.key_count);
    println!("  in_transaction: {}", stats.in_transaction);

    // LSM 限制：multi-table 不支援
    println!("\n--- Multi-table Test ---");
    let result = engine.put(2, b"key", b"value");
    println!("put to table_id=2: {:?}", result);

    println!("\n=== Done ===");
}