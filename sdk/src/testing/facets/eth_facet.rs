use crate::core::AttributeValue;
use crate::eth::EthHandlerType;
use crate::{data, Data};
use crate::{DataBinding, HandlerType};
use alloy::primitives::Address;
use alloy::rpc::types::{Block, Log, Transaction};
use prost_types;
use std::collections::HashMap;
use std::sync::Arc;

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
            let data_binding = self.create_log_data_binding(&log, &chain_id_str).await;
            self.server.process_databinding(&data_binding, &mut test_result).await;
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
                let log_address = format!("{:?}", log.address()).to_lowercase();
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
                            if topic_idx >= log.topics().len() {
                                topic_match = false;
                                break;
                            }
                            
                            let log_topic = format!("{:?}", log.topics()[topic_idx]).to_lowercase();
                            
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

    /// Test a single block
    pub async fn test_block(&self, block: Block, chain_id: Option<u64>) -> TestResult {
        self.test_blocks(vec![block], chain_id).await
    }

    /// Test multiple blocks
    pub async fn test_blocks(&self, _blocks: Vec<Block>, chain_id: Option<u64>) -> TestResult {
        let _chain_id = chain_id.unwrap_or(1);
        
        // TODO: Similar to test_logs but for block events
        
        TestResult::new()
    }

    /// Test a single transaction
    pub async fn test_transaction(&self, transaction: Transaction, chain_id: Option<u64>) -> TestResult {
        self.test_transactions(vec![transaction], chain_id).await
    }

    /// Test multiple transactions
    pub async fn test_transactions(&self, _transactions: Vec<Transaction>, chain_id: Option<u64>) -> TestResult {
        let _chain_id = chain_id.unwrap_or(1);
        
        // TODO: Process transaction events
        
        TestResult::new()
    }

    /// Test account-specific log (for account-level handlers)
    pub async fn test_account_log(&self, address: Address, log: Log, chain_id: Option<u64>) -> TestResult {
        self.test_account_logs(address, vec![log], chain_id).await
    }

    /// Test multiple account-specific logs
    pub async fn test_account_logs(&self, _address: Address, _logs: Vec<Log>, chain_id: Option<u64>) -> TestResult {
        let _chain_id = chain_id.unwrap_or(1);
        
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
    pub db: Arc<crate::testing::MemoryDatabase>,
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
    pub attributes: HashMap<String, AttributeValue>,
    pub metadata: TestMetadata,
}

#[derive(Debug, Clone)]
pub struct TestMetadata {
    pub contract_name: Option<String>,
    pub block_number: Option<u64>,
    pub handler_type: EthHandlerType,
}

impl Default for EthTestFacet {
    fn default() -> Self {
        Self::new(crate::testing::TestProcessorServer::new())
    }
}