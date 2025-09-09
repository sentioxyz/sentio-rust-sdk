use alloy::primitives::U256;
use alloy::rpc::types::{Block, Log, Transaction};
use std::str::FromStr;

/// Utility functions for creating mock blockchain data for testing

/// Create a mock ERC20 Transfer log
///
/// This is a common utility for testing ERC20 token processors.
///
/// # Arguments
///
/// * `contract_address` - The contract address that emitted the log
/// * `from` - The address tokens are transferred from
/// * `to` - The address tokens are transferred to  
/// * `value` - The amount of tokens transferred
///
/// # Example
///
/// ```rust
/// use sentio_sdk::testing::mock_transfer_log;
///
/// let log = mock_transfer_log(
///     "0x1E4EDE388cbc9F4b5c79681B7f94d36a11ABEBC9",
///     "0x0000000000000000000000000000000000000000", // mint from zero address
///     "0xB329e39Ebefd16f40d38f07643652cE17Ca5Bac1",
///     "1000000000000000000" // 1 token (18 decimals)
/// );
/// ```
/// Create a mock ERC20 Transfer log using JSON deserialization for compatibility  
pub fn mock_transfer_log(
    contract_address: &str,
    from: &str, 
    to: &str,
    value: &str
) -> Log {
    let transfer_event_signature = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
    let from_padded = format!("0x{:0>64}", from.trim_start_matches("0x"));
    let to_padded = format!("0x{:0>64}", to.trim_start_matches("0x"));
    
    // Convert value to hex-encoded 32-byte data
    let value_u256 = U256::from_str(value).expect("Invalid value");
    let value_hex = format!("0x{:064x}", value_u256);
    
    let log_json = format!(r#"{{
        "address": "{}",
        "topics": [
            "{}",
            "{}",
            "{}"
        ],
        "data": "{}",
        "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "blockNumber": "0xdb4c4f",
        "transactionHash": "0x1111111111111111111111111111111111111111111111111111111111111111",
        "transactionIndex": "0x2a",
        "logIndex": "0x1",
        "removed": false
    }}"#, contract_address, transfer_event_signature, from_padded, to_padded, value_hex);
    
    serde_json::from_str(&log_json).expect("Failed to create mock transfer log")
}

/// Create a mock ERC20 Approval log using JSON deserialization for compatibility
pub fn mock_approval_log(
    contract_address: &str,
    owner: &str,
    spender: &str, 
    value: &str
) -> Log {
    let approval_event_signature = "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925";
    let owner_padded = format!("0x{:0>64}", owner.trim_start_matches("0x"));
    let spender_padded = format!("0x{:0>64}", spender.trim_start_matches("0x"));
    
    // Convert value to hex-encoded 32-byte data
    let value_u256 = U256::from_str(value).expect("Invalid value");
    let value_hex = format!("0x{:064x}", value_u256);
    
    let log_json = format!(r#"{{
        "address": "{}",
        "topics": [
            "{}",
            "{}",
            "{}"
        ],
        "data": "{}",
        "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "blockNumber": "0xdb4c4f",
        "transactionHash": "0x1111111111111111111111111111111111111111111111111111111111111111",
        "transactionIndex": "0x2a",
        "logIndex": "0x2",
        "removed": false
    }}"#, contract_address, approval_event_signature, owner_padded, spender_padded, value_hex);
    
    serde_json::from_str(&log_json).expect("Failed to create mock approval log")
}

/// Create a mock block for testing
/// Note: This creates a simplified mock using JSON deserialization for compatibility
pub fn mock_block(number: u64, timestamp: u64) -> Block {
    let block_json = format!(r#"{{
        "hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "number": "0x{:x}",
        "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "timestamp": "0x{:x}",
        "gasLimit": "0x7a1200",
        "gasUsed": "0x3d0900",
        "miner": "0x0000000000000000000000000000000000000000",
        "difficulty": "0xf4240",
        "totalDifficulty": "0x{:x}",
        "nonce": "0x0000000000000000",
        "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
        "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
        "stateRoot": "0xd7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544",
        "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
        "extraData": "0x",
        "baseFeePerGas": "0x4a817c800",
        "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "transactions": [],
        "uncles": []
    }}"#, number, timestamp, number * 1000000);
    
    serde_json::from_str(&block_json).expect("Failed to create mock block")
}

/// Create a mock transaction for testing
/// Note: This creates a simplified mock using JSON deserialization for compatibility
pub fn mock_transaction(
    from: &str,
    to: Option<&str>,
    value: &str,
    data: Option<&str>
) -> Transaction {
    let to_field = match to {
        Some(addr) => format!(r#""to": "{}","#, addr),
        None => "".to_string(),
    };
    
    let data_field = match data {
        Some(d) => d.to_string(),
        None => "0x".to_string(),
    };
    
    let tx_json = format!(r#"{{
        "hash": "0x1111111111111111111111111111111111111111111111111111111111111111",
        "nonce": "0x2a",
        "blockHash": "0x2222222222222222222222222222222222222222222222222222222222222222",
        "blockNumber": "0xdb4c4f",
        "transactionIndex": "0x1",
        "from": "{}",
        {}
        "value": "{}",
        "gasPrice": "0x4a817c800",
        "gas": "0x5208",
        "input": "{}",
        "chainId": "0x1",
        "type": "0x0",
        "v": "0x1b",
        "r": "0x1",
        "s": "0x1"
    }}"#, from, to_field, value, data_field);
    
    serde_json::from_str(&tx_json).expect("Failed to create mock transaction")
}

/// Common chain IDs for testing
pub mod chain_ids {
    pub const ETHEREUM: u64 = 1;
    pub const GOERLI: u64 = 5;
    pub const SEPOLIA: u64 = 11155111;
    pub const POLYGON: u64 = 137;
    pub const BSC: u64 = 56;
    pub const ARBITRUM: u64 = 42161;
    pub const OPTIMISM: u64 = 10;
    pub const AVALANCHE: u64 = 43114;
}

/// Common contract addresses for testing
pub mod addresses {
    /// Zero address (often used as "from" in mint transactions)
    pub const ZERO: &str = "0x0000000000000000000000000000000000000000";
    
    /// Common test addresses
    pub const TEST_ADDRESS_1: &str = "0x1111111111111111111111111111111111111111";
    pub const TEST_ADDRESS_2: &str = "0x2222222222222222222222222222222222222222";
    pub const TEST_ADDRESS_3: &str = "0x3333333333333333333333333333333333333333"; 
    pub const TEST_CONTRACT: &str = "0x1E4EDE388cbc9F4b5c79681B7f94d36a11ABEBC9";
    
    /// Popular token addresses for testing
    pub const USDC_ETHEREUM: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    pub const USDT_ETHEREUM: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
    pub const WETH_ETHEREUM: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_transfer_log() {
        let log = mock_transfer_log(
            addresses::TEST_CONTRACT,
            addresses::ZERO,
            addresses::TEST_ADDRESS_1,
            "1000000000000000000"
        );
        
        assert_eq!(format!("{:?}", log.address()).to_lowercase(), addresses::TEST_CONTRACT.to_lowercase());
        assert_eq!(log.topics().len(), 3);
        assert!(log.block_number.is_some());
    }

    #[test]
    fn test_mock_block() {
        let block = mock_block(123456, 1640995200); // Jan 1, 2022 timestamp
        
        assert_eq!(block.header.number, 123456);
        assert_eq!(block.header.timestamp, 1640995200);
     }
    
    #[test]
    fn test_mock_transaction() {
        let tx = mock_transaction(
            addresses::TEST_ADDRESS_1,
            Some(addresses::TEST_ADDRESS_2),
            "1000000000000000000", // 1 ETH
            None
        );
        
        // Test basic transaction properties that are accessible
        assert_eq!(tx.block_number, Some(14371919));
        assert!(tx.transaction_index.is_some());
      }
}