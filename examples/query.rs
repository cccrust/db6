//! Query Module Example - Fluent Interface
//! 
//! 展示如何使用 Db 進行 KV 和 SQL 操作

use db6::query::Db;

fn main() {
    println!("=== Query Module Example (Fluent Interface) ===\n");

    // 測試 1: Table operations (method chaining)
    println!("--- Table Operations (Method Chaining) ---");
    let mut db = Db::new("memory").unwrap();
    println!("Engine type: {}", db.engine_type());

    db.table("users")
        .put(b"k1", b"value1")
        .unwrap()
        .put(b"k2", b"value2")
        .unwrap()
        .put(b"k3", b"value3")
        .unwrap();

    let val = db.table("users").get(b"k1").unwrap();
    println!("get k1: {:?}", val);

    // Batch put
    db.table("users")
        .batch_put(vec![
            (b"k4".to_vec(), b"value4".to_vec()),
            (b"k5".to_vec(), b"value5".to_vec()),
        ])
        .unwrap();

    let rows = db.table("users").scan(b"", b"").unwrap();
    println!("scan all: {} rows", rows.len());

    // 測試 2: SELECT with method chaining
    println!("\n--- SELECT with Method Chaining ---");
    db.table("users").put(b"1", b"Alice").unwrap();
    db.table("users").put(b"2", b"Bob").unwrap();
    db.table("users").put(b"3", b"Charlie").unwrap();

    let result = db.select("*")
        .from("users")
        .limit(10)
        .execute()
        .unwrap();

    println!("SELECT * FROM users:");
    for row in &result.rows {
        println!("  {:?}", row);
    }

    // 測試 3: BTree with ORDER BY support
    println!("\n--- BTreeMemoryEngine with ORDER BY ---");
    let mut db = Db::new("btree").unwrap();
    
    db.table("users")
        .put(b"c", b"3")
        .unwrap()
        .put(b"a", b"1")
        .unwrap()
        .put(b"b", b"2")
        .unwrap();

    let result = db.select("*")
        .from("users")
        .order_by("key")
        .limit(10)
        .execute()
        .unwrap();

    println!("SELECT * FROM users ORDER BY key:");
    for row in &result.rows {
        println!("  {:?}", row);
    }

    // 測試 4: Persistence
    println!("\n--- Persistence Test ---");
    let temp_dir = std::env::temp_dir().join("db6_query_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    {
        let mut db = Db::open("btree", &temp_dir).unwrap();
        db.table("users").put(b"name", b"Alice").unwrap();
        db.table("users").flush().unwrap();
        println!("Wrote and flushed");
    }

    {
        let mut db = Db::open("btree", &temp_dir).unwrap();
        let name = db.table("users").get(b"name").unwrap();
        println!("After reopen: name={:?}", name);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    // 測試 5: Delete operations
    println!("\n--- Delete Operations ---");
    let mut db = Db::new("memory").unwrap();
    db.table("users")
        .put(b"k1", b"v1")
        .unwrap()
        .put(b"k2", b"v2")
        .unwrap();

    println!("Before delete: {} keys", db.table("users").scan(b"", b"").unwrap().len());

    db.table("users").delete(b"k1").unwrap();

    println!("After delete: {} keys", db.table("users").scan(b"", b"").unwrap().len());

    println!("\n=== Done ===");
}