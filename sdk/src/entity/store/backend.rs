//! Storage backend abstraction for entity stores

use crate::core::{RUNTIME_CONTEXT, RuntimeContext};
use crate::db_request::{DbDelete, DbFilter, DbGet, DbList, DbUpsert, Op};
use crate::{DbRequest, db_request, db_response};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use tokio::sync::RwLock;
use tracing::warn;

/// Trait for storage backends
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Get a value by table and key
    async fn get(&mut self, table: &str, key: &str) -> Result<Option<db_response::Value>>;

    /// Delete a value by table and key
    async fn delete(&self, table: Vec<String>, ids: Vec<String>) -> Result<()>;

    /// List all values in a table (with optional filtering)
    async fn list(
        &mut self,
        table: &str,
        filters: Vec<DbFilter>,
        cursor: String,
        page_size: Option<u32>,
    ) -> Result<Option<db_response::Value>>;

    async fn upsert(&self, table: Vec<String>, id: Vec<String>, data: Vec<crate::common::RichStruct>) -> Result<()> ;
}

pub struct RemoteBackend {
    op_counter: AtomicU64,
    promises: HashMap<u64, async_promise::Resolve<Option<db_response::Value>>>,
}

impl RemoteBackend {
    pub fn new() -> Self {
        Self {
            op_counter: AtomicU64::new(0),
            promises: HashMap::new(),
        }
    }
}

impl Default for RemoteBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteBackend {
    pub(crate) fn receive_db_result(&mut self, db_result: crate::processor::DbResponse) {
        let op_id = db_result.op_id;
        if let Some(resolver) = self.promises.remove(&op_id) {
            resolver.into_resolve(db_result.value);
        } else {
            warn!("Received db result for unknown op id: {}", op_id);
        }
    }

    fn new_request(&self, op: Op) -> DbRequest {
        let op_id = self
            .op_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        DbRequest {
            op_id,
            op: Some(op),
        }
    }
    async fn send_async(&mut self, request: DbRequest) -> Result<Option<db_response::Value>> {
        let op_id = request.op_id;
        self.send(request).await?;

        let (resolve, promise) = async_promise::channel::<Option<db_response::Value>>();
        self.promises.insert(op_id, resolve);
        let result = promise.wait().await;

        if result.is_some() {
            let ret = result.unwrap();
            return Ok(ret.clone());
        }
        Ok(None)
    }

    async fn send(&self, req: DbRequest) -> Result<()> {
        let ctx = RUNTIME_CONTEXT.try_with(|ctx| ctx.clone())
            .map_err(|_| anyhow::anyhow!("Runtime context not available - make sure this is called within a processor handler"))?;
        ctx.send_db_request(req).await
    }
}

#[async_trait]
impl StorageBackend for RemoteBackend {
    async fn get(&mut self, table: &str, id: &str) -> Result<Option<db_response::Value>> {
        let op = Op::Get(DbGet {
            entity: table.to_string(),
            id: id.to_string(),
        });
        let req = self.new_request(op);
        self.send_async(req).await
    }

    async fn delete(&self, table: Vec<String>, ids: Vec<String>) -> Result<()> {
        let op = Op::Delete(DbDelete {
            entity: table,
            id: ids,
        });
        let req = self.new_request(op);
        self.send(req).await
    }

    async fn list(
        &mut self,
        table: &str,
        filters: Vec<DbFilter>,
        cursor: String,
        page_size: Option<u32>,
    ) -> Result<Option<db_response::Value>> {
        let op = Op::List(DbList {
            entity: table.to_string(),
            filters,
            cursor: cursor.to_string(),
            page_size,
        });
        let req = self.new_request(op);
        self.send_async(req).await
    }

    async fn upsert(&self, table: Vec<String>, id: Vec<String>, data: Vec<crate::common::RichStruct>) -> Result<()> {
        let op = Op::Upsert(DbUpsert {
            entity: table,
            id: id,
            data: vec![],
            entity_data: data,
        });
        let req = self.new_request(op);
        self.send(req).await
    }
}
