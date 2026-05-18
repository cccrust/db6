use std::sync::{Arc, RwLock};
use db6::kv::KvEngine;
use db6::server::http::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Arc::new(RwLock::new(KvEngine::new("memory").unwrap()));
    let state = AppState { engine };

    let addr: std::net::SocketAddr = "127.0.0.1:50052".parse()?;
    println!("Starting db6 HTTP server on http://{}", addr);

    db6::server::http::start_http(&addr.to_string(), state).await
}