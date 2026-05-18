use axum::{
    Router,
    routing::{get, post},
    extract::{State, WebSocketUpgrade},
    Json,
    response::IntoResponse,
};
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use crate::kv::KvEngine;
use crate::{parse, KvApi, StorageEngine, Executor, SqlExecutor};

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
pub struct DeleteRequest {
    pub table_id: u32,
    pub key: String,
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
pub struct BatchPutRequest {
    pub table_id: u32,
    pub pairs: Vec<KvPair>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RangeDeleteRequest {
    pub table_id: u32,
    pub start: String,
    pub end: String,
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
pub struct AppState {
    pub engine: Arc<RwLock<KvEngine>>,
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/kv/put", post(kv_put))
        .route("/kv/get", post(kv_get))
        .route("/kv/delete", post(kv_delete))
        .route("/kv/scan", post(kv_scan))
        .route("/kv/batch_put", post(kv_batch_put))
        .route("/kv/range_delete", post(kv_range_delete))
        .route("/kv/stats", get(kv_stats))
        .route("/sql/execute", post(sql_execute))
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn health() -> &'static str {
    "OK"
}

async fn kv_put(
    State(state): State<AppState>,
    Json(req): Json<PutRequest>,
) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    KvApi::put(&mut *engine, req.table_id, req.key.as_bytes(), req.value.as_bytes())
        .expect("put failed");
    Json(StatusResponse { status: "ok".to_string() })
}

async fn kv_get(
    State(state): State<AppState>,
    Json(req): Json<GetRequest>,
) -> Json<GetResponse> {
    let mut engine = state.engine.write().unwrap();
    let value = KvApi::get(&mut *engine, req.table_id, req.key.as_bytes())
        .expect("get failed");
    Json(GetResponse {
        found: value.is_some(),
        value: value.map(|v| String::from_utf8_lossy(&v).to_string()),
    })
}

async fn kv_delete(
    State(state): State<AppState>,
    Json(req): Json<DeleteRequest>,
) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    KvApi::delete(&mut *engine, req.table_id, req.key.as_bytes())
        .expect("delete failed");
    Json(StatusResponse { status: "ok".to_string() })
}

async fn kv_scan(
    State(state): State<AppState>,
    Json(req): Json<ScanRequest>,
) -> Json<ScanResponse> {
    let mut engine = state.engine.write().unwrap();
    let pairs = KvApi::scan(&mut *engine, req.table_id, req.start.as_bytes(), req.end.as_bytes())
        .expect("scan failed");
    Json(ScanResponse {
        pairs: pairs
            .into_iter()
            .map(|(k, v)| KvPair {
                key: String::from_utf8_lossy(&k).to_string(),
                value: String::from_utf8_lossy(&v).to_string(),
            })
            .collect(),
    })
}

async fn kv_batch_put(
    State(state): State<AppState>,
    Json(req): Json<BatchPutRequest>,
) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    let pairs: Vec<_> = req
        .pairs
        .into_iter()
        .map(|p| (p.key.into_bytes(), p.value.into_bytes()))
        .collect();
    KvApi::batch_put(&mut *engine, req.table_id, pairs).expect("batch_put failed");
    Json(StatusResponse { status: "ok".to_string() })
}

async fn kv_range_delete(
    State(state): State<AppState>,
    Json(req): Json<RangeDeleteRequest>,
) -> Json<StatusResponse> {
    let mut engine = state.engine.write().unwrap();
    KvApi::range_delete(&mut *engine, req.table_id, req.start.as_bytes(), req.end.as_bytes())
        .expect("range_delete failed");
    Json(StatusResponse { status: "ok".to_string() })
}

async fn kv_stats(State(state): State<AppState>) -> Json<EngineStatsResponse> {
    let engine = state.engine.read().unwrap();
    let s = StorageEngine::stats(&*engine);
    Json(EngineStatsResponse {
        key_count: s.key_count,
        size_bytes: s.size_bytes,
        engine: s.engine.to_string(),
    })
}

async fn sql_execute(
    State(_state): State<AppState>,
    Json(req): Json<SqlRequest>,
) -> Json<SqlResponse> {
    use crate::engine::HashMemoryEngine;
    use std::boxed::Box;

    let mut exec = Executor::new(Box::new(HashMemoryEngine::new()));

    match exec.execute(&req.sql) {
        Ok(result) => Json(SqlResponse {
            columns: result.columns,
            rows: result.rows,
            affected: result.affected,
            error: None,
        }),
        Err(e) => Json(SqlResponse {
            columns: vec![],
            rows: vec![],
            affected: 0,
            error: Some(e.to_string()),
        }),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SqlRequest {
    pub sql: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SqlResponse {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub affected: usize,
    pub error: Option<String>,
}

pub async fn start_http(addr: &str, state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("HTTP/WS server listening on http://{}", addr);
    axum::serve(listener, create_app(state)).await?;
    Ok(())
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_socket(socket, state))
}

async fn handle_ws_socket(socket: axum::extract::ws::WebSocket, state: AppState) {
    use futures_util::{StreamExt, SinkExt};

    let (mut sender, mut receiver) = socket.split();
    let state = state.clone();

    while let Some(msg) = receiver.next().await {
        if let Ok(msg) = msg {
            if let axum::extract::ws::Message::Text(text) = msg {
                if let Ok(req) = serde_json::from_str::<crate::server::websocket::WsRequest>(&text) {
                    let resp = crate::server::websocket::handle_request(req, &state).await;
                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                        if sender.send(axum::extract::ws::Message::Text(resp_json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        } else {
            break;
        }
    }
}