use ethers::prelude::{Block, Log, Transaction, TransactionReceipt, H256};
use crate::data::EthLog;
use tracing::debug;
use serde_json;
use crate::core::MetaData;
use crate::eth::context::EthContext;
use crate::eth::eth_processor::{EthEvent, EventFilter};

/// Container for parsed Ethereum data structures
#[derive(Debug)]
pub struct ParsedEthData {
    pub log: Option<Log>,
    pub transaction: Option<Transaction>,
    pub receipt: Option<TransactionReceipt>,
    pub block: Option<Block<H256>>,
}

impl ParsedEthData {
    /// Extract metadata from all available Ethereum data sources
    pub fn extract_metadata(&self, chain_id: String, contract_name: String) -> MetaData {
        let mut metadata = MetaData {
            chain_id,
            contract_name,
            ..Default::default()
        };

        // Extract metadata from log data (highest priority for event-based processors)
        if let Some(ref log) = self.log {
            metadata.address = format!("{:?}", log.address);
            metadata.block_number = log.block_number.unwrap_or_default().as_u64();
            metadata.transaction_hash = format!("{:?}", log.transaction_hash.unwrap_or_default());
            metadata.transaction_index = log.transaction_index.unwrap_or_default().as_u32() as i32;
            metadata.log_index = log.log_index.unwrap_or_default().as_u32() as i32;
        }

        // Extract additional metadata from transaction data
        if let Some(ref transaction) = self.transaction {
            // Override with transaction data if log didn't provide it
            if metadata.transaction_hash.is_empty() || metadata.transaction_hash == "0x0000000000000000000000000000000000000000000000000000000000000000" {
                metadata.transaction_hash = format!("{:?}", transaction.hash);
            }
            if metadata.block_number == 0 {
                metadata.block_number = transaction.block_number.unwrap_or_default().as_u64();
            }
            if metadata.transaction_index == 0 {
                metadata.transaction_index = transaction.transaction_index.unwrap_or_default().as_u32() as i32;
            }
            if metadata.address.is_empty() {
                metadata.address = format!("{:?}", transaction.from);
            }
        }

        // Extract metadata from transaction receipt
        if let Some(ref receipt) = self.receipt {
            // Receipt can provide additional confirmation of transaction data
            if metadata.transaction_hash.is_empty() {
                metadata.transaction_hash = format!("{:?}", receipt.transaction_hash);
            }
            if metadata.block_number == 0 {
                metadata.block_number = receipt.block_number.unwrap_or_default().as_u64();
            }
            if metadata.transaction_index == 0 {
                metadata.transaction_index = receipt.transaction_index.as_u32() as i32;
            }
            // Use contract address from receipt if available
            if let Some(contract_address) = receipt.contract_address {
                if metadata.address.is_empty() {
                    metadata.address = format!("{:?}", contract_address);
                }
            }
        }

        // Extract metadata from block data
        if let Some(ref block) = self.block {
            if metadata.block_number == 0 {
                metadata.block_number = block.number.unwrap_or_default().as_u64();
            }
            metadata.block_timestamp = Some(block.timestamp.as_u64());
        }

        metadata
    }
}

impl From<&EthLog> for ParsedEthData {
    fn from(eth_log_data: &EthLog) -> Self {
        let mut parsed_data = ParsedEthData {
            log: None,
            transaction: None,
            receipt: None,
            block: None,
        };

        // Parse log data if available
        if !eth_log_data.raw_log.is_empty() {
            debug!("Parsing raw_log JSON: {}", eth_log_data.raw_log);
            match serde_json::from_str::<Log>(&eth_log_data.raw_log) {
                Ok(log) => parsed_data.log = Some(log),
                Err(e) => debug!("Failed to parse Log: {}", e),
            }
        }

        // Parse transaction data if available
        if let Some(raw_transaction) = &eth_log_data.raw_transaction {
            if !raw_transaction.is_empty() {
                debug!("Parsing raw_transaction JSON: {}", raw_transaction);
                match serde_json::from_str::<Transaction>(raw_transaction) {
                    Ok(tx) => parsed_data.transaction = Some(tx),
                    Err(e) => debug!("Failed to parse Transaction: {}", e),
                }
            }
        }

        // Parse transaction receipt data if available
        if let Some(raw_receipt) = &eth_log_data.raw_transaction_receipt {
            if !raw_receipt.is_empty() {
                debug!("Parsing raw_transaction_receipt JSON: {}", raw_receipt);
                match serde_json::from_str::<TransactionReceipt>(raw_receipt) {
                    Ok(receipt) => parsed_data.receipt = Some(receipt),
                    Err(e) => debug!("Failed to parse TransactionReceipt: {}", e),
                }
            }
        }

        // Parse block data if available
        if let Some(raw_block) = &eth_log_data.raw_block {
            if !raw_block.is_empty() {
                debug!("Parsing raw_block JSON: {}", raw_block);
                match serde_json::from_str::<Block<H256>>(raw_block) {
                    Ok(block) => parsed_data.block = Some(block),
                    Err(e) => debug!("Failed to parse Block: {}", e),
                }
            }
        }

        parsed_data
    }
}

/// Marker trait that defines filtering criteria for specific event types
/// This allows for type-safe event handling where each handler struct can define its own filter
pub trait EventMarker: Send + Sync + 'static {
    /// Returns the event filter criteria for this event type
    fn filter() -> Vec<EventFilter>;
}

#[crate::async_trait]
pub trait EthEventHandler<T: EventMarker>: Send + Sync + 'static {
    async fn on_event(&self, event: EthEvent, ctx: EthContext);
}