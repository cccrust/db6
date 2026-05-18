use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use crate::kv::KvEngine;
use crate::server::http::AppState;

pub struct Server {
    engine: Arc<RwLock<KvEngine>>,
    http_addr: SocketAddr,
}

impl Server {
    pub fn new(engine: Arc<RwLock<KvEngine>>) -> Self {
        Self {
            engine,
            http_addr: "127.0.0.1:50052".parse().unwrap(),
        }
    }

    pub fn http_addr(mut self, addr: SocketAddr) -> Self {
        self.http_addr = addr;
        self
    }

    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let state = AppState {
            engine: self.engine.clone(),
        };

        println!("Starting db6 server");
        println!("HTTP/REST: http://{}", self.http_addr);

        crate::server::http::start_http(&self.http_addr.to_string(), state).await
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new(Arc::new(RwLock::new(KvEngine::new("memory").unwrap())))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::default();
    server.serve().await
}