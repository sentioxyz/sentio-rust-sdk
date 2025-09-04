use crate::eth::EthHandlerType;
use crate::core::RuntimeContext;
use crate::{DataBinding, HandlerType};
use crate::{Data, data};
use ethers::types::{Log, Block, Transaction, Address, H256};
use std::collections::HashMap;
use serde_json::Value;
use tokio::sync::{RwLock, mpsc};
use prost_types;
use crate::entity::store::backend::RemoteBackend;
use crate::timeseries_result::TimeseriesType;

/// Ethereum testing facet for simulating blockchain data
///
/// This facet provides utilities to test Ethereum processors by simulating
/// logs, blocks, transactions, and traces. It mirrors the TypeScript EthFacet
/// functionality while providing type-safe Rust interfaces.
pub struct EthTestFacet {
    server: crate::testing::TestProcessorServer,
}

impl EthTestFacet {
    pub fn new(server: crate::testing::TestProcessorServer) -> Self {
        Self {
            server,
        }
    }

    /// Test a single log event
    ///
    /// # Arguments
    ///
    /// * `log` - The log event to test
    /// * `chain_id` - Optional chain ID (defaults to Ethereum mainnet)
    ///
    /// # Example
    ///
    /// ```rust
    /// use sentio_sdk::testing::{EthTestFacet, mock_transfer_log};
    ///
    /// let facet = EthTestFacet::new();
    /// let result = facet.test_log(mock_transfer_log("0x123..."), Some(1)).await;
    /// ```
    pub async fn test_log(&self, log: Log, chain_id: Option<u64>) -> TestResult {
        self.test_logs(vec![log], chain_id).await
    }

    /// Test multiple log events
    pub async fn test_logs(&self, logs: Vec<Log>, chain_id: Option<u64>) -> TestResult {
        let chain_id = chain_id.unwrap_or(1); // Default to Ethereum mainnet
        let chain_id_str = chain_id.to_string();
        
        let mut test_result = TestResult::new();
        
        for log in logs {
            // 1. Convert log to DataBinding format
            let data_binding = self.create_log_data_binding(&log, &chain_id_str).await;
            
            // 2. Create a channel for collecting metrics and events
            let (tx, mut rx) = mpsc::channel(1024);
            let remote_backend = std::sync::Arc::new(RwLock::new(RemoteBackend::new()));
            // 3. Create RuntimeContext
            let runtime_context = RuntimeContext::new_with_empty_metadata(tx, 1, remote_backend);
            
            // 4. Process the binding using PluginManager from server
            let pm = self.server.plugin_manager.read().await;
            match pm.process(&data_binding, runtime_context).await {
                Ok(_process_result) => {
                    // Processing succeeded, collect any messages from the channel
                    while let Ok(msg) = rx.try_recv() {
                        if let Ok(response) = msg {
                            self.collect_results_from_channel_response(response, chain_id, &mut test_result);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error processing log: {}", e);
                    // Continue with other logs even if one fails
                }
            }
        }
        
        test_result
    }
    
    /// Create a DataBinding from a log, following the same logic as TypeScript buildLogBinding
    async fn create_log_data_binding(&self, log: &Log, chain_id: &str) -> DataBinding {
        // Serialize the log to JSON for the raw_log field
        let raw_log = serde_json::to_string(log).unwrap_or_default();
        
        let eth_log = data::EthLog {
            log: None, // Deprecated field
            timestamp: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            transaction: None, // Deprecated field
            transaction_receipt: None, // Deprecated field
            block: None, // Deprecated field
            raw_log,
            raw_transaction: None,
            raw_transaction_receipt: None,
            raw_block: None,
        };
        
        let data = Data {
            value: Some(data::Value::EthLog(eth_log)),
        };
        
        // Build handler_ids by matching log against contract configurations
        let handler_ids = self.build_handler_ids_for_log(log, chain_id).await;
        
        DataBinding {
            data: Some(data),
            handler_type: HandlerType::EthLog as i32,
            handler_ids,
            chain_id: chain_id.to_string(),
        }
    }
    
    /// Build handler IDs for a log by matching against contract configurations
    /// This follows the same logic as TypeScript buildLogBinding function
    async fn build_handler_ids_for_log(&self, log: &Log, chain_id: &str) -> Vec<i32> {
        let mut handler_ids = Vec::new();
        
        // Get the contract configurations from the server
        let config_response = self.server.get_config().await;
        
        // Now match the log against contract configurations
        for contract_config in &config_response.contract_configs {
            if let Some(contract) = &contract_config.contract {
                // Check if chain_id matches
                if contract.chain_id != chain_id {
                    continue;
                }
                
                // Check if address matches (convert to lowercase for comparison)
                let log_address = format!("{:?}", log.address).to_lowercase();
                let contract_address = contract.address.to_lowercase();
                
                if log_address != contract_address && contract_address != "*" {
                    continue;
                }
                
                // Check log handlers for topic matches
                for log_config in &contract_config.log_configs {
                    for filter in &log_config.filters {
                        // Check if topics match
                        let mut topic_match = true;
                        for (topic_idx, filter_topic) in filter.topics.iter().enumerate() {
                            if topic_idx >= log.topics.len() {
                                topic_match = false;
                                break;
                            }
                            
                            let log_topic = format!("{:?}", log.topics[topic_idx]).to_lowercase();
                            
                            // If filter topic has no hashes, it matches all
                            if filter_topic.hashes.is_empty() {
                                continue;
                            }
                            
                            // Check if any of the filter topic hashes match
                            let mut hash_match = false;
                            for hash in &filter_topic.hashes {
                                if hash.to_lowercase() == log_topic {
                                    hash_match = true;
                                    break;
                                }
                            }
                            
                            if !hash_match {
                                topic_match = false;
                                break;
                            }
                        }
                        
                        if topic_match {
                            handler_ids.push(log_config.handler_id);
                        }
                    }
                }
            }
        }
        
        handler_ids
    }
    
    /// Collect results from a single channel response
    fn collect_results_from_channel_response(
        &self, 
        response: crate::ProcessStreamResponseV2, 
        chain_id: u64, 
        test_result: &mut TestResult
    ) {
        if let Some(value) = response.value {
            match value {
                crate::processor::process_stream_response_v2::Value::TsRequest(ts_request) => {
                    // Process timeseries data (counters and gauges)
                    for ts_data in ts_request.data {
                        self.process_timeseries_result(ts_data, chain_id, test_result);
                    }
                }
                // TODO: Handle event logs and other request types when the correct protobuf types are identified
                _ => {
                    // Handle other request types if needed
                }
            }
        }
    }
    
    /// Process a single timeseries result (counter or gauge)
    fn process_timeseries_result(
        &self,
        ts_result: crate::TimeseriesResult,
        chain_id: u64,
        test_result: &mut TestResult
    ) {
        let metadata = TestMetadata {
            contract_name: ts_result.metadata.as_ref().map(|m| m.contract_name.clone()),
            block_number: ts_result.metadata.as_ref().map(|m| m.block_number as u64),
            handler_type: EthHandlerType::Event,
            chain_id,
        };
        
        let name = ts_result.metadata.as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_default();
            
        let labels = ts_result.metadata.as_ref()
            .map(|m| m.labels.clone())
            .unwrap_or_default();
        
        // Get the metric type from the `type` field
        let metric_type = ts_result.r#type();
        
        // Extract value from data field (which is a RichStruct)
        let value = if let Some(ref data) = ts_result.data {
            // Try to extract a numeric value from the RichStruct fields
            data.fields.get("value")
                .and_then(|v| match &v.value {
                    Some(value_type) => match value_type {
                        crate::common::rich_value::Value::FloatValue(f) => Some(*f),
                        crate::common::rich_value::Value::IntValue(i) => Some(*i as f64),
                        _ => None,
                    },
                    None => None,
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        
        match metric_type {
            TimeseriesType::Counter => {
                test_result.counters.push(CounterResult {
                    name,
                    value,
                    labels,
                    metadata,
                });
            }
            TimeseriesType::Gauge => {
                test_result.gauges.push(GaugeResult {
                    name,
                    value,
                    labels,
                    metadata,
                });
            }
            TimeseriesType::Event => {
                // Process event logs
                self.process_event_log(ts_result, chain_id, test_result);
            }
            _ => {
                // Handle other metric types if needed
            }
        }
    }
    
    /// Process an event log from TimeseriesResult
    fn process_event_log(
        &self,
        ts_result: crate::TimeseriesResult,
        chain_id: u64,
        test_result: &mut TestResult
    ) {
        let metadata = TestMetadata {
            contract_name: ts_result.metadata.as_ref().map(|m| m.contract_name.clone()),
            block_number: ts_result.metadata.as_ref().map(|m| m.block_number as u64),
            handler_type: EthHandlerType::Event,
            chain_id,
        };
        
        // Extract event name and attributes from the RichStruct data
        let (event_name, attributes) = if let Some(data) = &ts_result.data {
            let mut event_name = String::new();
            let mut attributes = HashMap::new();
            
            // Extract event name
            if let Some(name_value) = data.fields.get("event_name") {
                if let Some(value) = &name_value.value {
                    if let crate::common::rich_value::Value::StringValue(name) = value {
                        event_name = name.clone();
                    }
                }
            }
            
            // Convert all other fields to JSON values for attributes
            for (key, rich_value) in &data.fields {
                if key != "event_name" {  // Skip the event name field
                    let json_value = self.rich_value_to_json_value(rich_value);
                    attributes.insert(key.clone(), json_value);
                }
            }
            
            (event_name, attributes)
        } else {
            ("unknown_event".to_string(), HashMap::new())
        };
        
        test_result.events.push(EventResult {
            name: event_name,
            attributes,
            metadata,
        });
    }
    
    /// Convert RichValue to serde_json::Value for event attributes
    fn rich_value_to_json_value(&self, rich_value: &crate::common::RichValue) -> serde_json::Value {
        if let Some(value) = &rich_value.value {
            match value {
                crate::common::rich_value::Value::StringValue(s) => serde_json::Value::String(s.clone()),
                crate::common::rich_value::Value::FloatValue(f) => serde_json::Value::Number(
                    serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0))
                ),
                crate::common::rich_value::Value::IntValue(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
                crate::common::rich_value::Value::Int64Value(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
                crate::common::rich_value::Value::BoolValue(b) => serde_json::Value::Bool(*b),
                crate::common::rich_value::Value::ListValue(list) => {
                    let array: Vec<serde_json::Value> = list.values.iter()
                        .map(|v| self.rich_value_to_json_value(v))
                        .collect();
                    serde_json::Value::Array(array)
                },
                crate::common::rich_value::Value::StructValue(struct_val) => {
                    let mut map = serde_json::Map::new();
                    for (key, nested_value) in &struct_val.fields {
                        map.insert(key.clone(), self.rich_value_to_json_value(nested_value));
                    }
                    serde_json::Value::Object(map)
                },
                _ => serde_json::Value::Null,
            }
        } else {
            serde_json::Value::Null
        }
    }

    /// Test a single block
    pub async fn test_block(&self, block: Block<H256>, chain_id: Option<u64>) -> TestResult {
        self.test_blocks(vec![block], chain_id).await
    }

    /// Test multiple blocks
    pub async fn test_blocks(&self, blocks: Vec<Block<H256>>, chain_id: Option<u64>) -> TestResult {
        let chain_id = chain_id.unwrap_or(1);
        
        // TODO: Similar to test_logs but for block events
        
        TestResult::new()
    }

    /// Test a single transaction
    pub async fn test_transaction(&self, transaction: Transaction, chain_id: Option<u64>) -> TestResult {
        self.test_transactions(vec![transaction], chain_id).await
    }

    /// Test multiple transactions
    pub async fn test_transactions(&self, transactions: Vec<Transaction>, chain_id: Option<u64>) -> TestResult {
        let chain_id = chain_id.unwrap_or(1);
        
        // TODO: Process transaction events
        
        TestResult::new()
    }

    /// Test account-specific log (for account-level handlers)
    pub async fn test_account_log(&self, address: Address, log: Log, chain_id: Option<u64>) -> TestResult {
        self.test_account_logs(address, vec![log], chain_id).await
    }

    /// Test multiple account-specific logs
    pub async fn test_account_logs(&self, address: Address, logs: Vec<Log>, chain_id: Option<u64>) -> TestResult {
        let chain_id = chain_id.unwrap_or(1);
        
        // TODO: Process account-specific log events
        
        TestResult::new()
    }

    // TODO: Add trace testing methods when trace support is implemented
    // pub async fn test_trace(&self, trace: Trace, chain_id: Option<u64>) -> TestResult
    // pub async fn test_traces(&self, traces: Vec<Trace>, chain_id: Option<u64>) -> TestResult
}

/// Result of a test operation containing metrics and events
#[derive(Debug, Clone, Default)]
pub struct TestResult {
    pub counters: Vec<CounterResult>,
    pub gauges: Vec<GaugeResult>,
    pub events: Vec<EventResult>,
}

impl TestResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the first counter value by name
    pub fn first_counter_value(&self, name: &str) -> Option<f64> {
        self.counters
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.value)
    }

    /// Get the first gauge value by name
    pub fn first_gauge_value(&self, name: &str) -> Option<f64> {
        self.gauges
            .iter()
            .find(|g| g.name == name)
            .map(|g| g.value)
    }
    
    /// Get the first event by name
    pub fn first_event(&self, name: &str) -> Option<&EventResult> {
        self.events
            .iter()
            .find(|e| e.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct CounterResult {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub metadata: TestMetadata,
}

#[derive(Debug, Clone)]
pub struct GaugeResult {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub metadata: TestMetadata,
}

#[derive(Debug, Clone)]
pub struct EventResult {
    pub name: String,
    pub attributes: HashMap<String, Value>,
    pub metadata: TestMetadata,
}

#[derive(Debug, Clone)]
pub struct TestMetadata {
    pub contract_name: Option<String>,
    pub block_number: Option<u64>,
    pub handler_type: EthHandlerType,
    pub chain_id: u64,
}

impl Default for EthTestFacet {
    fn default() -> Self {
        Self::new(crate::testing::TestProcessorServer::new())
    }
}