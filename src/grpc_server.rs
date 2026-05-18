use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use axum::{
    Router,
    routing::{get as axum_get, post},
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use db6::kv::KvEngine;
use db6::{KvApi, StorageEngine};

#[derive(Debug, Serialize, Deserialize)]
pub struct PutRequest {
    pub table_id: u32,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRequest {
    pub table_id: u32,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetResponse {
    pub value: Option<String>,
    pub found: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub table_id: u32,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResponse {
    pub pairs: Vec<KvPair>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KvPair {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub table_id: u32,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchPutRequest {
    pub table_id: u32,
    pub pairs: Vec<KvPair>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineStatsResponse {
    pub key_count: u64,
    pub size_bytes: u64,
    pub engine: String,
}

#[derive(Clone)]
struct AppState {
    engine: Arc<RwLock<KvEngine>>,
}

async fn put(State(state): State<AppState>, Json(req): Json<PutRequest>) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    KvApi::put(&mut *engine, req.table_id, req.key.as_bytes(), req.value.as_bytes()).unwrap();
    Json(StatusResponse { status: "ok".to_string() })
}

async fn get(State(state): State<AppState>, Json(req): Json<GetRequest>) -> Json<GetResponse> {
    let mut engine = state.engine.write().unwrap();
    let value = KvApi::get(&mut *engine, req.table_id, req.key.as_bytes()).unwrap();
    Json(GetResponse {
        found: value.is_some(),
        value: value.map(|v| String::from_utf8_lossy(&v).to_string()),
    })
}

async fn delete(State(state): State<AppState>, Json(req): Json<DeleteRequest>) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    KvApi::delete(&mut *engine, req.table_id, req.key.as_bytes()).unwrap();
    Json(StatusResponse { status: "ok".to_string() })
}

async fn scan(State(state): State<AppState>, Json(req): Json<ScanRequest>) -> Json<ScanResponse> {
    let mut engine = state.engine.write().unwrap();
    let pairs = KvApi::scan(&mut *engine, req.table_id, req.start.as_bytes(), req.end.as_bytes()).unwrap();
    Json(ScanResponse {
        pairs: pairs.into_iter().map(|(k, v)| KvPair {
            key: String::from_utf8_lossy(&k).to_string(),
            value: String::from_utf8_lossy(&v).to_string(),
        }).collect(),
    })
}

async fn batch_put(State(state): State<AppState>, Json(req): Json<BatchPutRequest>) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    let pairs: Vec<_> = req.pairs.into_iter()
        .map(|p| (p.key.into_bytes(), p.value.into_bytes()))
        .collect();
    KvApi::batch_put(&mut *engine, req.table_id, pairs).unwrap();
    Json(StatusResponse { status: "ok".to_string() })
}

async fn stats(State(state): State<AppState>) -> Json<EngineStatsResponse> {
    let engine = state.engine.read().unwrap();
    let s = StorageEngine::stats(&*engine);
    Json(EngineStatsResponse {
        key_count: s.key_count,
        size_bytes: s.size_bytes,
        engine: s.engine.to_string(),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Arc::new(RwLock::new(KvEngine::new("memory").unwrap()));
    let state = AppState { engine };

    let app = Router::new()
        .route("/kv/put", post(put))
        .route("/kv/get", post(get))
        .route("/kv/delete", post(delete))
        .route("/kv/scan", post(scan))
        .route("/kv/batch_put", post(batch_put))
        .route("/kv/stats", axum_get(stats))
        .with_state(state);

    let addr = "127.0.0.1:50051".parse::<SocketAddr>()?;
    println!("HTTP/JSON server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}