use ethers::types::{Log, Block, Transaction, Address, H256, U256, U64, Bytes, H64, OtherFields};
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
pub fn mock_transfer_log(
    contract_address: &str,
    from: &str, 
    to: &str,
    value: &str
) -> Log {
    let transfer_event_signature = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
    
    Log {
        address: Address::from_str(contract_address).expect("Invalid contract address"),
        topics: vec![
            H256::from_str(transfer_event_signature).expect("Invalid event signature"),
            H256::from_str(&format!("{:0>64}", from.trim_start_matches("0x"))).expect("Invalid from address"),
            H256::from_str(&format!("{:0>64}", to.trim_start_matches("0x"))).expect("Invalid to address"),
        ],
        data: {
            let value_u256 = U256::from_str(value).expect("Invalid value");
            let mut bytes = [0u8; 32];
            value_u256.to_big_endian(&mut bytes);
            Bytes::from(bytes)
        },
        block_hash: Some(H256::from_low_u64_be(12345)),
        block_number: Some(U64::from(14373295)),
        transaction_hash: Some(H256::from_low_u64_be(67890)),
        transaction_index: Some(U64::from(42)),
        log_index: Some(U256::from(1)),
        transaction_log_index: Some(U256::from(1)),
        log_type: None,
        removed: Some(false),
    }
}

/// Create a mock ERC20 Approval log
pub fn mock_approval_log(
    contract_address: &str,
    owner: &str,
    spender: &str, 
    value: &str
) -> Log {
    let approval_event_signature = "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925";
    
    Log {
        address: Address::from_str(contract_address).expect("Invalid contract address"),
        topics: vec![
            H256::from_str(approval_event_signature).expect("Invalid event signature"),
            H256::from_str(&format!("{:0>64}", owner.trim_start_matches("0x"))).expect("Invalid owner address"),
            H256::from_str(&format!("{:0>64}", spender.trim_start_matches("0x"))).expect("Invalid spender address"),
        ],
        data: {
            let value_u256 = U256::from_str(value).expect("Invalid value");
            let mut bytes = [0u8; 32];
            value_u256.to_big_endian(&mut bytes);
            Bytes::from(bytes)
        },
        block_hash: Some(H256::from_low_u64_be(12345)),
        block_number: Some(U64::from(14373295)),
        transaction_hash: Some(H256::from_low_u64_be(67890)),
        transaction_index: Some(U64::from(42)),
        log_index: Some(U256::from(2)),
        transaction_log_index: Some(U256::from(2)),
        log_type: None,
        removed: Some(false),
    }
}

/// Create a mock block for testing
pub fn mock_block(number: u64, timestamp: u64) -> Block<H256> {
    Block {
        number: Some(U64::from(number)),
        hash: Some(H256::from_low_u64_be(number)),
        parent_hash: H256::from_low_u64_be(number.saturating_sub(1)),
        nonce: Some(H64::from_low_u64_be(12345)),
        uncles_hash: H256::default(),
        logs_bloom: None,
        transactions_root: H256::default(),
        state_root: H256::default(),
        receipts_root: H256::default(),
        author: Some(Address::default()),
        difficulty: U256::from(1000000),
        total_difficulty: Some(U256::from(number * 1000000)),
        extra_data: Bytes::default(),
        size: Some(U256::from(1024)),
        gas_limit: U256::from(8000000),
        gas_used: U256::from(4000000),
        timestamp: U256::from(timestamp),
        transactions: vec![],
        uncles: vec![],
        base_fee_per_gas: Some(U256::from(20_000_000_000u64)), // 20 gwei
        mix_hash: Some(H256::default()),
        seal_fields: vec![],
        blob_gas_used: Some(U256::zero()),
        excess_blob_gas: Some(U256::zero()),
        parent_beacon_block_root: None,
        withdrawals_root: None,
        withdrawals: None,
        other: OtherFields::default(),
    }
}

/// Create a mock transaction for testing
pub fn mock_transaction(
    from: &str,
    to: Option<&str>,
    value: &str,
    data: Option<&str>
) -> Transaction {
    Transaction {
        hash: H256::from_low_u64_be(12345),
        nonce: U256::from(42),
        block_hash: Some(H256::from_low_u64_be(67890)),
        block_number: Some(U64::from(14373295)),
        transaction_index: Some(U64::from(1)),
        from: Address::from_str(from).expect("Invalid from address"),
        to: to.map(|addr| Address::from_str(addr).expect("Invalid to address")),
        value: U256::from_str(value).expect("Invalid value"),
        gas_price: Some(U256::from(20_000_000_000u64)), // 20 gwei
        gas: U256::from(21000),
        input: data.map(|d| Bytes::from_str(d).expect("Invalid data")).unwrap_or_default(),
        v: U64::from(27),
        r: U256::from(1),
        s: U256::from(1),
        transaction_type: Some(U64::from(0)), // Legacy transaction
        access_list: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        chain_id: Some(U256::from(1)), // Ethereum mainnet
        other: OtherFields::default(),
    }
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
        
        assert_eq!(log.address, Address::from_str(addresses::TEST_CONTRACT).unwrap());
        assert_eq!(log.topics.len(), 3);
        assert!(log.block_number.is_some());
    }

    #[test]
    fn test_mock_block() {
        let block = mock_block(123456, 1640995200); // Jan 1, 2022 timestamp
        
        assert_eq!(block.number, Some(U64::from(123456)));
        assert_eq!(block.timestamp, U256::from(1640995200));
        assert!(block.hash.is_some());
    }
    
    #[test]
    fn test_mock_transaction() {
        let tx = mock_transaction(
            addresses::TEST_ADDRESS_1,
            Some(addresses::TEST_ADDRESS_2),
            "1000000000000000000", // 1 ETH
            None
        );
        
        assert_eq!(tx.from, Address::from_str(addresses::TEST_ADDRESS_1).unwrap());
        assert_eq!(tx.to, Some(Address::from_str(addresses::TEST_ADDRESS_2).unwrap()));
        assert_eq!(tx.value, U256::from_str("1000000000000000000").unwrap());
    }
}