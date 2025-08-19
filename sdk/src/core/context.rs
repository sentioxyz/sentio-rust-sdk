use crate::{processor::TimeseriesResult, ProcessStreamResponseV2};
use anyhow::Result;
use std::collections::HashMap;
use tonic::Status;
use tracing::debug;
// Re-export EventLogger trait from event_logger module
pub use crate::core::event_logger::EventLogger;
// Re-export metrics types
pub use crate::core::metrics::{Counter, Gauge, Meter, MetricOptions, NumberValue};

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
pub trait Context: Send + Sync {
    /// Get the base context for creating loggers and meters
    fn base_context(&self) -> &BaseContext;

    /// Get metadata access through the runtime context
    fn metadata(&self) -> MetaData {
        // Get from runtime context through task local storage
        RUNTIME_CONTEXT.with(|ctx| (*ctx.metadata).clone())
    }

    /// Get metadata for a given name and labels
    fn address(&self) -> String {
        self.metadata().address.clone()
    }

    fn contract_name(&self) -> String {
        self.metadata().contract_name.clone()
    }

    fn chain_id(&self) -> String {
        self.metadata().chain_id.clone()
    }

    fn block_number(&self) -> u64 {
        self.metadata().block_number
    }

    fn transaction_hash(&self) -> String {
        self.metadata().transaction_hash.clone()
    }

    fn transaction_index(&self) -> i32 {
        self.metadata().transaction_index
    }

    fn log_index(&self) -> i32 {
        self.metadata().log_index
    }
}

/// Base context struct that provides common functionality for all contexts
/// This handles creating pure event loggers and meters
pub struct BaseContext {
    // BaseContext is now simplified - no metadata storage
}

impl Clone for BaseContext {
    fn clone(&self) -> Self {
        Self {}
    }
}

impl BaseContext {
    /// Create a new BaseContext
    pub fn new() -> Self {
        Self {}
    }

    /// Create a new pure Event Logger
    pub fn event_logger(&self) -> crate::core::event_logger::EventLogger {
        crate::core::event_logger::EventLogger::new()
    }

    /// Create a new pure Meter
    pub fn meter(&self) -> Meter {
        Meter::new()
    }

    /// Create a new pure Counter
    pub fn counter(&self, name: &str) -> Counter {
        Counter::new(name)
    }

    /// Create a new pure Counter with options
    pub fn counter_with_options(&self, name: &str, options: MetricOptions) -> Counter {
        Counter::with_options(name, options)
    }

    /// Create a new pure Gauge
    pub fn gauge(&self, name: &str) -> Gauge {
        Gauge::new(name)
    }

    /// Create a new pure Gauge with options
    pub fn gauge_with_options(&self, name: &str, options: MetricOptions) -> Gauge {
        Gauge::with_options(name, options)
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
    /// Metadata for this runtime context (Arc for lightweight cloning)
    pub metadata: std::sync::Arc<MetaData>,
}

impl RuntimeContext {
    /// Create a new RuntimeContext with the given event logger sender, process ID, and metadata
    pub fn new(
        tx: tokio::sync::mpsc::Sender<Result<ProcessStreamResponseV2, Status>>,
        process_id: i32,
        metadata: MetaData,
    ) -> Self {
        Self {
            tx,
            process_id,
            metadata: std::sync::Arc::new(metadata),
        }
    }

    /// Create a new RuntimeContext with empty metadata
    pub fn new_with_empty_metadata(
        tx: tokio::sync::mpsc::Sender<Result<ProcessStreamResponseV2, Status>>,
        process_id: i32,
    ) -> Self {
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
        Self {
            tx,
            process_id,
            metadata: std::sync::Arc::new(metadata),
        }
    }

    /// Update the metadata in this runtime context
    pub fn with_metadata(mut self, metadata: MetaData) -> Self {
        self.metadata = std::sync::Arc::new(metadata);
        self
    }

    /// Get reference to metadata
    pub fn metadata(&self) -> &MetaData {
        &self.metadata
    }

    /// Convert metadata to protobuf RecordMetaData
    pub fn to_record_metadata(&self, name: &str) -> crate::processor::RecordMetaData {
        crate::processor::RecordMetaData {
            address: self.metadata.address.clone(),
            contract_name: self.metadata.contract_name.clone(),
            block_number: self.metadata.block_number,
            log_index: self.metadata.log_index,
            transaction_index: self.metadata.transaction_index,
            transaction_hash: self.metadata.transaction_hash.clone(),
            chain_id: self.metadata.chain_id.clone(),
            labels: self.metadata.base_labels.clone(),
            name: name.to_string(),
        }
    }

    /// Emit a TimeseriesResult through the stream
    pub async fn send_timeseries_result(
        &self,
        name: &str,
        mut timeseries_result: TimeseriesResult,
    ) -> Result<()> {
        use crate::processor::TsRequest;

        timeseries_result.metadata = Some(self.to_record_metadata(name));

        let ts_request = TsRequest {
            data: vec![timeseries_result],
        };

        // Create ProcessStreamResponseV2 with TsRequest
        let response = ProcessStreamResponseV2 {
            process_id: self.process_id,
            value: Some(crate::processor::process_stream_response_v2::Value::TsRequest(ts_request)),
        };

        // Send through the channel
        self.tx
            .send(Ok(response))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send timeseries result: {}", e))?;

        debug!("Emitted TimeseriesResult");
        Ok(())
    }
}

tokio::task_local! {
    pub static RUNTIME_CONTEXT: RuntimeContext;
}
