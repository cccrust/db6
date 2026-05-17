//! db6 REPL — 互動式 SQL 命令列介面
//!
//! 提供類似 SQLite shell 的互動環境，支援引擎切換、SQL 執行、檔案載入等功能。
//! 使用者在 db6> 提示字元後輸入 SQL 或點指令 (.engine, .help, .quit, .read) 進行操作。

use db6::Executor;
use db6::engine::{HashMemoryEngine, BTreeMemoryEngine, BTreeEngine, LsmEngine, StorageEngine};
use std::io::{self, Write};

/// 依據字串名稱建立對應的儲存引擎實例
///
/// 支援的引擎：
/// - `"memory"` 或 `"memory-hash"`：HashMemoryEngine，類似 Redis，快速 KV
/// - `"memory-btree"`：BTreeMemoryEngine，支援 ORDER BY/scan
/// - `"btree"`：BTreeEngine，磁碟持久化，支援交易
/// - `"lsm"`：LsmEngine，高寫入吞吐量
fn create_engine(engine_type: &str) -> Option<Box<dyn StorageEngine>> {
    match engine_type {
        "memory" | "memory-hash" => Some(Box::new(HashMemoryEngine::new())),
        "memory-btree" => Some(Box::new(BTreeMemoryEngine::new())),
        "btree" => Some(Box::new(BTreeEngine::new())),
        "lsm" => Some(Box::new(LsmEngine::new())),
        _ => None,
    }
}

/// REPL 主程式：無窮迴圈讀取使用者輸入
fn main() {
    // 顯示初始歡迎訊息
    println!("db6 v2.5.0 - Interactive SQL REPL");
    println!("Type '.quit' to exit, '.help' for commands\n");

    // 預設使用 memory-btree 引擎，因為它支援最完整的 SQL 功能
    let mut engine_type = "memory-btree".to_string();
    let engine = BTreeMemoryEngine::new();
    let mut executor = Executor::new(Box::new(engine));

    // REPL 主迴圈
    loop {
        // 輸出提示字元
        print!("db6> ");
        io::stdout().flush().unwrap();

        // 讀取一行使用者輸入
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap() == 0 {
            break; // EOF (Ctrl+D) 結束
        }

        // 去除前後空白
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // 處理 .engine <type> 指令：切換引擎
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

        // 處理其他點指令
        match input {
            // .quit / .exit: 結束 REPL
            ".quit" | ".exit" => break,

            // .help: 顯示說明資訊
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

            // .engine: 顯示目前引擎
            ".engine" => {
                println!("Engine: {}", engine_type);
                continue;
            }

            // .read <file>: 從檔案讀取並執行 SQL
            _ if input.starts_with(".read ") => {
                let path = input.trim_start_matches(".read ").trim();
                match std::fs::read_to_string(path) {
                    Ok(sql) => {
                        // 以分號分隔逐句執行
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

        // 執行 SQL 語句
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
    // 結束訊息
    println!("\nGoodbye!");
}