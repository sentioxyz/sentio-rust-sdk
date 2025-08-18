use anyhow::Result;
use tonic::Status;
use tracing::debug;
use crate::{ProcessStreamResponseV2, processor::TimeseriesResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
// Re-export EventLogger trait from event_logger module
pub use crate::core::event_logger::EventLogger;
// Re-export metrics types
pub use crate::core::metrics::{Meter, Counter, Gauge, MetricOptions, NumberValue};

/// Labels type for metadata - equivalent to TypeScript Labels
pub type Labels = HashMap<String, String>;

/// Metadata structure that contains context information for events and metrics
#[derive(Debug, Clone)]
pub struct MetaData {
    pub address: String,
    pub contract_name: String,
    pub chain_id: String,
    pub block_number: u64,
    pub transaction_hash: String,
    pub transaction_index: i32,
    pub log_index: i32,
    pub base_labels: Labels,
}

/// Context trait that all user-facing contexts should implement
#[tonic::async_trait]
pub trait Context: Send + Sync {
    /// Get the event logger for this context
    fn event_logger(&self) -> &dyn EventLogger;


    fn get_metadata(&self) -> &RwLock<MetaData>;

    /// Get the meter for creating metrics (counters, gauges)
    fn meter(&self) -> &Meter;

    /// Get metadata for a given name and labels
    async fn address(&self) -> String {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.address.clone()
    }

    async fn contract_name(&self) -> String {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.contract_name.clone()
    }

    async fn chain_id(&self) -> String {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.chain_id.clone()
    }

    async fn block_number(&self) -> u64 {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.block_number
    }

    async fn transaction_hash(&self) -> String {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.transaction_hash.clone()
    }

    async fn transaction_index(&self) -> i32 {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.transaction_index
    }

    async fn log_index(&self) -> i32 {
        let lock = self.get_metadata();
        let metadata = lock.read().await;
        metadata.log_index
    }
}

/// Base context struct that provides common functionality for all contexts
/// This handles metadata management, caching, and event logging
pub struct BaseContext {
    /// Metadata wrapped in async RwLock for safe concurrent access
    pub metadata: Arc<RwLock<MetaData>>,
    /// Event logger instance
    pub event_logger: crate::core::event_logger::DefaultEventLogger,
    /// Meter for creating metrics (counters, gauges)
    pub meter: Meter,
}

impl Clone for BaseContext {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            event_logger: self.event_logger.clone(),
            meter: self.meter.clone(),
        }
    }
}

impl BaseContext {
    /// Create a new BaseContext
    pub fn new() -> Self {
        use crate::core::event_logger::DefaultEventLogger;
        
        let metadata = MetaData {
            address: String::new(),
            contract_name: String::new(),
            chain_id: String::new(),
            block_number: 0,
            transaction_hash: String::new(),
            transaction_index: 0,
            log_index: 0,
            base_labels: HashMap::new(),
        };
        
        let metadata_lock = Arc::new(RwLock::new(metadata));
        let event_logger = DefaultEventLogger::with_metadata_lock(metadata_lock.clone());
        
        Self {
            metadata: metadata_lock,
            event_logger,
            meter: Meter::new(),
        }
    }
    
    /// Create a new BaseContext with context information
    pub fn with_context(
        address: String,
        contract_name: String,
        chain_id: String,
        block_number: u64,
        transaction_hash: String,
        transaction_index: i32,
        log_index: i32,
    ) -> Self {
        use crate::core::event_logger::DefaultEventLogger;
        
        let metadata = MetaData {
            address,
            contract_name,
            chain_id,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
            base_labels: HashMap::new(),
        };
        
        let metadata_lock =  Arc::new(RwLock::new(metadata));
        let event_logger = DefaultEventLogger::with_metadata_lock(metadata_lock.clone());
        
        Self {
            metadata: metadata_lock,
            event_logger,
            meter: Meter::new(),
        }
    }
}



impl Default for BaseContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime context for processing requests with event logger support
#[derive(Clone)]
pub struct RuntimeContext {
    /// Channel sender for emitting event logs to the inbound stream
    pub tx: tokio::sync::mpsc::Sender<Result<ProcessStreamResponseV2, Status>>,
    /// Process ID for this runtime context
    pub process_id: i32,
}

impl RuntimeContext {
    /// Create a new RuntimeContext with the given event logger sender and process ID
    pub fn new(tx: tokio::sync::mpsc::Sender<Result<ProcessStreamResponseV2, Status>>, process_id: i32) -> Self {
        Self { tx, process_id }
    }

    /// Emit a TimeseriesResult through the stream
    pub async fn send_timeseries_result(&self, timeseries_result: TimeseriesResult) -> Result<()> {
        use crate::processor::TsRequest;
        
        let ts_request = TsRequest {
            data: vec![timeseries_result],
        };

        // Create ProcessStreamResponseV2 with TsRequest
        let response = ProcessStreamResponseV2 {
            process_id: self.process_id,
            value: Some(crate::processor::process_stream_response_v2::Value::TsRequest(ts_request)),
        };

        // Send through the channel
        self.tx.send(Ok(response)).await
            .map_err(|e| anyhow::anyhow!("Failed to send timeseries result: {}", e))?;

        debug!("Emitted TimeseriesResult");
        Ok(())
    }
}

tokio::task_local! {
    pub static RUNTIME_CONTEXT: RuntimeContext;
}