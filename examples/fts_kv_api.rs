//! FTS KV API example
//! 
//! 展示如何在 KV API 中使用 FTS 功能

use db6::engine::{StorageEngine, HashMemoryEngine, BTreeMemoryEngine, BTreeEngine, LsmEngine};
use db6::FtsIndex;

fn test_fts_with_engine<E: StorageEngine>(name: &str, engine: E) {
    println!("\n--- Testing FTS with {} ---", name);
    
    // 建立 FtsIndex
    let mut fts = FtsIndex::new(engine);
    
    // 加入文件
    fts.insert(1, "Hello world").unwrap();
    fts.insert(2, "資料庫系統").unwrap();
    fts.insert(3, "Rust 程式設計").unwrap();
    fts.insert(4, "Database system").unwrap();
    fts.insert(5, "Hello Rust").unwrap();
    
    // 搜尋
    println!("Search 'world':");
    let results = fts.search("world").unwrap();
    println!("  doc_ids: {:?}", results);
    
    println!("Search 'Hello':");
    let results = fts.search("Hello").unwrap();
    println!("  doc_ids: {:?}", results);
    
    println!("Search '資料庫':");
    let results = fts.search("資料庫").unwrap();
    println!("  doc_ids: {:?}", results);
    
    // BM25 搜尋
    println!("\nBM25 Search 'database':");
    let results = fts.search_bm25("database").unwrap();
    for (doc_id, score) in results {
        println!("  doc_id: {}, score: {:.2}", doc_id, score);
    }
}

fn main() {
    println!("=== FTS KV API Example ===\n");
    
    // 測試各引擎
    test_fts_with_engine("HashMemoryEngine", HashMemoryEngine::new());
    test_fts_with_engine("BTreeMemoryEngine", BTreeMemoryEngine::new());
    test_fts_with_engine("BTreeEngine", BTreeEngine::new());
    
    // LsmEngine 不支援 FTS
    println!("\n--- LsmEngine FTS Test (預期失敗) ---");
    let result = std::panic::catch_unwind(|| {
        let engine = LsmEngine::new();
        let mut fts = FtsIndex::new(engine);
        fts.insert(1, "test").unwrap();
    });
    if result.is_err() {
        println!("LsmEngine: FTS not supported (as expected)");
    }
    
    println!("\n=== Done ===");
    println!("\nUsage:");
    println!("  use db6::{{FtsIndex, StorageEngine}};");
    println!("  let mut fts = FtsIndex::new(engine);");
    println!("  fts.insert(doc_id, text).unwrap();");
    println!("  let results = fts.search(query).unwrap();");
}