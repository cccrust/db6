//! db6 REPL - Interactive SQL command line

use db6::{parse, Executor};
use db6::engine::MemoryEngine;
use std::io::{self, Write};

fn main() {
    println!("db6 v2.1.0 - Interactive SQL REPL");
    println!("Type '.quit' to exit, '.help' for commands\n");

    let engine = MemoryEngine::new();
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

        match input {
            ".quit" | ".exit" => break,
            ".help" => {
                println!("Commands:");
                println!("  .quit, .exit  - Exit REPL");
                println!("  .help         - Show this help");
                println!("  .engine       - Show current engine");
                println!("");
                println!("SQL Examples:");
                println!("  SELECT * FROM users");
                println!("  INSERT INTO t VALUES (1, 'hello')");
                println!("  CREATE TABLE users (id INTEGER, name TEXT)");
                continue;
            }
            ".engine" => {
                println!("Engine: Memory");
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