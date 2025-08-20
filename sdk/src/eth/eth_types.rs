use ethers::prelude::{Block, Log, Transaction, TransactionReceipt, H256};
use crate::data::EthLog;
use tracing::debug;
use serde_json;

/// Container for parsed Ethereum data structures
#[derive(Debug)]
pub struct ParsedEthData {
    pub log: Option<Log>,
    pub transaction: Option<Transaction>,
    pub receipt: Option<TransactionReceipt>,
    pub block: Option<Block<H256>>,
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

 