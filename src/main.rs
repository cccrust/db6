//! db6 REPL - Interactive SQL command line

use db6::Executor;
use db6::engine::{HashMemoryEngine, BTreeMemoryEngine, BTreeEngine, LsmEngine, StorageEngine};
use std::io::{self, Write};

fn create_engine(engine_type: &str) -> Option<Box<dyn StorageEngine>> {
    match engine_type {
        "memory" | "memory-hash" => Some(Box::new(HashMemoryEngine::new())),
        "memory-btree" => Some(Box::new(BTreeMemoryEngine::new())),
        "btree" => Some(Box::new(BTreeEngine::new())),
        "lsm" => Some(Box::new(LsmEngine::new())),
        _ => None,
    }
}

fn main() {
    println!("db6 v2.5.0 - Interactive SQL REPL");
    println!("Type '.quit' to exit, '.help' for commands\n");

    let mut engine_type = "memory-btree".to_string();
    let engine = BTreeMemoryEngine::new();
    let mut executor = Executor::new(Box::new(engine));

    loop {
        print!("db6> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap() == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        if input.starts_with(".engine ") {
            let new_type = input.trim_start_matches(".engine ").trim();
            if let Some(engine) = create_engine(new_type) {
                engine_type = new_type.to_string();
                executor = Executor::new(engine);
                println!("Switched to {} engine", engine_type);
            } else {
                println!("Unknown engine: {}. Use: memory, memory-hash, memory-btree, btree, lsm", new_type);
            }
            continue;
        }

        match input {
            ".quit" | ".exit" => break,
            ".help" => {
                println!("Commands:");
                println!("  .quit, .exit  - Exit REPL");
                println!("  .help         - Show this help");
                println!("  .engine       - Show current engine");
                println!("  .engine <type> - Switch engine (memory-hash, memory-btree, btree, lsm)");
                println!("  .read <file>  - Execute SQL from file");
                println!("");
                println!("Engine types:");
                println!("  memory-hash   - Redis-like, fast KV (no ORDER BY/scan)");
                println!("  memory-btree  - SQLite-like, supports SQL (ORDER BY/scan)");
                println!("  btree         - BTree on disk, transactions");
                println!("  lsm           - LSM tree, high write throughput");
                println!("");
                println!("SQL Examples:");
                println!("  SELECT * FROM users");
                println!("  INSERT INTO t VALUES (1, 'hello')");
                println!("  UPDATE t SET value = 'new'");
                println!("  DELETE FROM t");
                println!("  SELECT * FROM t ORDER BY key DESC LIMIT 10");
                continue;
            }
            ".engine" => {
                println!("Engine: {}", engine_type);
                continue;
            }
            _ if input.starts_with(".read ") => {
                let path = input.trim_start_matches(".read ").trim();
                match std::fs::read_to_string(path) {
                    Ok(sql) => {
                        for stmt in sql.split(';') {
                            let stmt = stmt.trim();
                            if !stmt.is_empty() {
                                match executor.execute(stmt) {
                                    Ok(result) => {
                                        if result.rows.is_empty() {
                                            println!("OK ({} rows)", result.affected);
                                        } else {
                                            println!("{} rows:", result.rows.len());
                                            for row in &result.rows {
                                                println!("  {:?}", row);
                                            }
                                        }
                                    }
                                    Err(e) => println!("Error: {:?}", e),
                                }
                            }
                        }
                    }
                    Err(e) => println!("Error reading file: {}", e),
                }
                continue;
            }
            _ => {}
        }

        match executor.execute(input) {
            Ok(result) => {
                if result.rows.is_empty() {
                    println!("OK ({} rows affected)", result.affected);
                } else {
                    println!("{} rows:", result.rows.len());
                    for row in &result.rows {
                        println!("  {:?}", row);
                    }
                }
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }
    println!("\nGoodbye!");
}