//! 多引擎切換範例
//! 
//! 展示如何在不同引擎間切換

use db6::engine::{StorageEngine, MemoryEngine, BTreeEngine, LsmEngine};

fn test_engine(name: &str, mut engine: Box<dyn StorageEngine>) {
    println!("\n--- Testing {} ---", name);
    
    // 基本操作
    engine.put(1, b"key1", b"value1").unwrap();
    let val = engine.get(1, b"key1").unwrap();
    println!("put/get: {:?}", val);
    
    // 統計
    let stats = engine.stats();
    println!("engine: {}, keys: {}", stats.engine, stats.key_count);
}

fn main() {
    println!("=== Multi Engine Example ===\n");

    // 使用 Memory Engine
    test_engine("Memory", MemoryEngine::open_memory());

    // 使用 BTree Engine
    test_engine("BTree", BTreeEngine::open_memory());

    // 使用 LSM Engine
    test_engine("LSM", LsmEngine::open_memory());

    // 動態建立引擎
    println!("\n--- Dynamic Engine Creation ---");
    let engine_types = vec!["memory", "btree", "lsm"];
    
    for etype in engine_types {
        let engine: Box<dyn StorageEngine> = match etype {
            "memory" => MemoryEngine::open_memory(),
            "btree" => BTreeEngine::open_memory(),
            "lsm" => LsmEngine::open_memory(),
            _ => continue,
        };
        println!("Created {}: {}", etype, engine.engine_type());
    }

    println!("\n=== Done ===");
}