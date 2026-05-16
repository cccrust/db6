//! BTree Engine 範例
//! 
//! 展示如何使用 BTreeEngine

use db6::engine::{StorageEngine, BTreeEngine};

fn main() {
    println!("=== BTree Engine Example ===\n");

    // 建立 BTree 引擎 (記憶體模式)
    let mut engine = BTreeEngine::new();

    // 引擎資訊
    println!("Engine type: {}", engine.engine_type());

    // 基本操作
    engine.put(1, b"key1", b"value1").unwrap();
    engine.put(1, b"key2", b"value2").unwrap();
    engine.put(1, b"key3", b"value3").unwrap();

    let val = engine.get(1, b"key1").unwrap();
    println!("get key1: {:?}", val);

    // 掃描
    let rows = engine.scan(1, b"key1", b"key3").unwrap();
    println!("\nscan key1 to key3:");
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

    // 回滾測試
    println!("\n--- Rollback Test ---");
    engine.begin_transaction().unwrap();
    engine.put(1, b"key5", b"value5").unwrap();
    engine.rollback_transaction().unwrap();
    println!("rollback_transaction: OK");

    let val = engine.get(1, b"key5").unwrap();
    println!("after rollback, get key5: {:?}", val);

    // 統計
    let stats = engine.stats();
    println!("\nStats:");
    println!("  key_count: {}", stats.key_count);
    println!("  in_transaction: {}", stats.in_transaction);

    println!("\n=== Done ===");
}