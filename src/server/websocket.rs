use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use axum::{
    extract::{
        State,
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
    Json,
};
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use crate::kv::KvEngine;
use crate::{KvApi, StorageEngine};
use crate::server::http::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsRequest {
    pub method: String,
    pub id: Option<u64>,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsResponse {
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            if let Message::Text(text) = msg {
                if let Ok(req) = serde_json::from_str::<WsRequest>(&text) {
                    let resp = handle_request(req, &state).await;
                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                        if socket.send(Message::Text(resp_json.into())).await.is_err() {
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

pub async fn handle_request(req: WsRequest, state: &AppState) -> WsResponse {
    match req.method.as_str() {
        "health" => WsResponse { id: req.id, result: Some("OK".into()), error: None },

        "kv_put" => {
            let table_id = req.params.get("table_id").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
            let key = req.params.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = req.params.get("value").and_then(|v| v.as_str()).unwrap_or("");

            let mut engine = state.engine.write().unwrap();
            match KvApi::put(&mut *engine, table_id, key.as_bytes(), value.as_bytes()) {
                Ok(_) => WsResponse { id: req.id, result: Some(serde_json::json!({"status": "ok"})), error: None },
                Err(e) => WsResponse { id: req.id, result: None, error: Some(e.to_string()) },
            }
        }

        "kv_get" => {
            let table_id = req.params.get("table_id").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
            let key = req.params.get("key").and_then(|v| v.as_str()).unwrap_or("");

            let mut engine = state.engine.write().unwrap();
            match KvApi::get(&mut *engine, table_id, key.as_bytes()) {
                Ok(value) => WsResponse {
                    id: req.id,
                    result: Some(serde_json::json!({
                        "found": value.is_some(),
                        "value": value.map(|v| String::from_utf8_lossy(&v).to_string())
                    })),
                    error: None,
                },
                Err(e) => WsResponse { id: req.id, result: None, error: Some(e.to_string()) },
            }
        }

        "kv_delete" => {
            let table_id = req.params.get("table_id").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
            let key = req.params.get("key").and_then(|v| v.as_str()).unwrap_or("");

            let mut engine = state.engine.write().unwrap();
            match KvApi::delete(&mut *engine, table_id, key.as_bytes()) {
                Ok(_) => WsResponse { id: req.id, result: Some(serde_json::json!({"status": "ok"})), error: None },
                Err(e) => WsResponse { id: req.id, result: None, error: Some(e.to_string()) },
            }
        }

        "kv_scan" => {
            let table_id = req.params.get("table_id").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
            let start = req.params.get("start").and_then(|v| v.as_str()).unwrap_or("");
            let end = req.params.get("end").and_then(|v| v.as_str()).unwrap_or("");

            let mut engine = state.engine.write().unwrap();
            match KvApi::scan(&mut *engine, table_id, start.as_bytes(), end.as_bytes()) {
                Ok(pairs) => {
                    let result: Vec<_> = pairs.into_iter().map(|(k, v)| {
                        serde_json::json!({
                            "key": String::from_utf8_lossy(&k),
                            "value": String::from_utf8_lossy(&v)
                        })
                    }).collect();
                    WsResponse { id: req.id, result: Some(serde_json::json!({"pairs": result})), error: None }
                }
                Err(e) => WsResponse { id: req.id, result: None, error: Some(e.to_string()) },
            }
        }

        "kv_batch_put" => {
            let table_id = req.params.get("table_id").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
            let pairs: Vec<_> = req.params.get("pairs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter().filter_map(|p| {
                        Some((
                            p.get("key")?.as_str()?.as_bytes().to_vec(),
                            p.get("value")?.as_str()?.as_bytes().to_vec(),
                        ))
                    }).collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let mut engine = state.engine.write().unwrap();
            match KvApi::batch_put(&mut *engine, table_id, pairs) {
                Ok(_) => WsResponse { id: req.id, result: Some(serde_json::json!({"status": "ok"})), error: None },
                Err(e) => WsResponse { id: req.id, result: None, error: Some(e.to_string()) },
            }
        }

        "kv_stats" => {
            let engine = state.engine.read().unwrap();
            let s = StorageEngine::stats(&*engine);
            WsResponse {
                id: req.id,
                result: Some(serde_json::json!({
                    "key_count": s.key_count,
                    "size_bytes": s.size_bytes,
                    "engine": s.engine
                })),
                error: None,
            }
        }

        _ => WsResponse {
            id: req.id,
            result: None,
            error: Some(format!("Unknown method: {}", req.method)),
        },
    }
}

pub fn create_ws_router(state: AppState) -> axum::Router {
    use axum::routing::get;

    axum::Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}