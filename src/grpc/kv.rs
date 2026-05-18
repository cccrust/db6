use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use crate::kv::KvEngine;

#[derive(Debug, Serialize, Deserialize)]
pub struct PutRequest {
    pub table_id: u32,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PutResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRequest {
    pub table_id: u32,
    pub key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetResponse {
    pub value: Option<Vec<u8>>,
    pub found: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub table_id: u32,
    pub key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanRequest {
    pub table_id: u32,
    pub start: Vec<u8>,
    pub end: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResponse {
    pub pairs: Vec<KvPair>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KvPair {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchPutRequest {
    pub table_id: u32,
    pub pairs: Vec<KvPair>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchPutResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct RangeDeleteRequest {
    pub table_id: u32,
    pub start: Vec<u8>,
    pub end: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RangeDeleteResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlushRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlushResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineStatsRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineStatsResponse {
    pub engine_type: String,
    pub total_keys: u64,
    pub total_size: u64,
    pub hits: u64,
    pub misses: u64,
}

pub struct KvServiceImpl {
    engine: Arc<RwLock<KvEngine>>,
}

impl KvServiceImpl {
    pub fn new(engine: Arc<RwLock<KvEngine>>) -> Self {
        Self { engine }
    }
}

#[tonic::async_trait]
impl KvService for KvServiceImpl {
    async fn put(&self, request: PutRequest) -> Result<PutResponse, Status> {
        let mut engine = self.engine.write().map_err(|e| Status::internal(e.to_string()))?;
        engine.put(request.table_id, &request.key, &request.value)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(PutResponse {})
    }

    async fn get(&self, request: GetRequest) -> Result<GetResponse, Status> {
        let engine = self.engine.read().map_err(|e| Status::internal(e.to_string()))?;
        let value = engine.get(request.table_id, &request.key)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(GetResponse {
            found: value.is_some(),
            value,
        })
    }

    async fn delete(&self, request: DeleteRequest) -> Result<DeleteResponse, Status> {
        let mut engine = self.engine.write().map_err(|e| Status::internal(e.to_string()))?;
        engine.delete(request.table_id, &request.key)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(DeleteResponse {})
    }

    async fn scan(&self, request: ScanRequest) -> Result<ScanResponse, Status> {
        let engine = self.engine.read().map_err(|e| Status::internal(e.to_string()))?;
        let pairs = engine.scan(request.table_id, &request.start, &request.end)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(ScanResponse {
            pairs: pairs.into_iter().map(|(k, v)| KvPair { key: k, value: v }).collect(),
        })
    }

    async fn batch_put(&self, request: BatchPutRequest) -> Result<BatchPutResponse, Status> {
        let mut engine = self.engine.write().map_err(|e| Status::internal(e.to_string()))?;
        let pairs: Vec<_> = request.pairs.into_iter().map(|p| (p.key, p.value)).collect();
        engine.batch_put(request.table_id, pairs)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(BatchPutResponse {})
    }

    async fn range_delete(&self, request: RangeDeleteRequest) -> Result<RangeDeleteResponse, Status> {
        let mut engine = self.engine.write().map_err(|e| Status::internal(e.to_string()))?;
        engine.range_delete(request.table_id, &request.start, &request.end)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(RangeDeleteResponse {})
    }

    async fn flush(&self, _request: FlushRequest) -> Result<FlushResponse, Status> {
        let mut engine = self.engine.write().map_err(|e| Status::internal(e.to_string()))?;
        engine.flush().map_err(|e| Status::internal(e.to_string()))?;
        Ok(FlushResponse {})
    }

    async fn engine_stats(&self, _request: EngineStatsRequest) -> Result<EngineStatsResponse, Status> {
        use crate::engine::EngineStats;
        let engine = self.engine.read().map_err(|e| Status::internal(e.to_string()))?;
        let stats: EngineStats = engine.stats();
        Ok(EngineStatsResponse {
            engine_type: stats.engine_type,
            total_keys: stats.total_keys,
            total_size: stats.total_size,
            hits: stats.hits,
            misses: stats.misses,
        })
    }
}

#[tonic::async_trait]
pub trait KvService: Send + Sync {
    async fn put(&self, request: PutRequest) -> Result<PutResponse, Status>;
    async fn get(&self, request: GetRequest) -> Result<GetResponse, Status>;
    async fn delete(&self, request: DeleteRequest) -> Result<DeleteResponse, Status>;
    async fn scan(&self, request: ScanRequest) -> Result<ScanResponse, Status>;
    async fn batch_put(&self, request: BatchPutRequest) -> Result<BatchPutResponse, Status>;
    async fn range_delete(&self, request: RangeDeleteRequest) -> Result<RangeDeleteResponse, Status>;
    async fn flush(&self, request: FlushRequest) -> Result<FlushResponse, Status>;
    async fn engine_stats(&self, request: EngineStatsRequest) -> Result<EngineStatsResponse, Status>;
}

pub fn create_kv_service() -> KvServiceImpl {
    KvServiceImpl::new(Arc::new(RwLock::new(KvEngine::new("memory").unwrap())))
}