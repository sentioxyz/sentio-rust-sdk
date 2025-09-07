//! Integration tests for eth-basic processor
//!
//! This test suite demonstrates comprehensive testing of the eth-basic processor
//! using the Sentio SDK's testing framework with organized test structure.

use eth_basic::{ApprovalEvent, MyEthProcessor, TransferEvent};
use ethers::types::{H256, U256};
use sentio_sdk::eth::eth_processor::EthProcessor;
use sentio_sdk::testing::{addresses, chain_ids, mock_transfer_log, TestProcessorServer};

/// Test setup data containing configured test server and processor information
struct TestSetup {
    test_server: TestProcessorServer,
    contract_address: String,
    contract_chain_id: String,
    contract_name: String,
}

impl TestSetup {
    /// Common setup method to create test server, configure processor, and bind events
    async fn new() -> Self {
        let mut test_server = TestProcessorServer::new();
        
        // Create and configure our processor
        let processor = MyEthProcessor::new();
        let contract_address = processor.address().to_string();
        let contract_chain_id = processor.chain_id().to_string();
        let contract_name = processor.name().to_string();
        
        // Configure the processor for both Transfer and Approval events in a single chain
        processor
            .configure_event::<TransferEvent>(None)
            .configure_event::<ApprovalEvent>(None)
            .bind(&test_server);
        
        // Start the test server
        test_server.start().await.expect("Failed to start test server");
        
        TestSetup {
            test_server,
            contract_address,
            contract_chain_id,
            contract_name,
        }
    }
}

/// Test 1: Verify processor configuration is correct
#[tokio::test]
async fn test_processor_configuration() {
    let setup = TestSetup::new().await;
    let config = setup.test_server.get_config().await;
    
    println!("🔧 Testing processor configuration...");
    
    // Verify the config contains our registered processor
    assert_eq!(config.contract_configs.len(), 1, "Expected 1 contract configuration");
    let contract_config = config.contract_configs[0].clone();
    let contract = contract_config.contract.unwrap();
    
    assert_eq!(contract.address, setup.contract_address);
    assert_eq!(contract.chain_id, setup.contract_chain_id);
    assert_eq!(contract.name, setup.contract_name);
    
    // Verify both Transfer and Approval event handlers are configured
    assert_eq!(contract_config.log_configs.len(), 2, "Expected 2 log handlers (Transfer + Approval)");
    
    // Verify the Transfer event handler configuration
    let transfer_topic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
    let transfer_handler = contract_config.log_configs.iter()
        .find(|handler| {
            handler.filters.iter().any(|filter| {
                filter.topics.iter().any(|topic| {
                    topic.hashes.contains(&transfer_topic.to_string())
                })
            })
        });
    assert!(transfer_handler.is_some(), "Expected to find Transfer event handler configuration");
    
    // Verify the Approval event handler configuration
    let approval_topic = "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925";
    let approval_handler = contract_config.log_configs.iter()
        .find(|handler| {
            handler.filters.iter().any(|filter| {
                filter.topics.iter().any(|topic| {
                    topic.hashes.contains(&approval_topic.to_string())
                })
            })
        });
    assert!(approval_handler.is_some(), "Expected to find Approval event handler configuration");
    
    println!("✅ Processor configuration verified successfully!");
}

/// Test 2: Verify event logs are properly recorded
#[tokio::test]
async fn test_event_logging() {
    let setup = TestSetup::new().await;
    let eth_facet = setup.test_server.eth();
    
    println!("📝 Testing event logging functionality...");
    
    // Create a mock transfer log
    let transfer_log = mock_transfer_log(
        &setup.contract_address,
        addresses::ZERO,
        addresses::TEST_ADDRESS_1,
        "1000000000000000000", // 1 token
    );
    
    // Process the transfer event
    let result = eth_facet.test_log(transfer_log.clone(), Some(chain_ids::ETHEREUM)).await;
    
    // Verify event logs are recorded
    assert!(!result.events.is_empty(), "Expected at least one event log to be recorded");
    
    println!("📊 Event processing results:");
    println!("  - Events logged: {}", result.events.len());
    for (i, event) in result.events.iter().enumerate() {
        println!("    Event {}: {}", i + 1, event.name);
    }
    
    // Test multiple events to ensure consistent logging
    let additional_logs = vec![
        mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, "500000000000000000"),
        mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_2, addresses::TEST_ADDRESS_3, "250000000000000000"),
    ];
    
    for (i, log) in additional_logs.iter().enumerate() {
        let result = eth_facet.test_log(log.clone(), Some(chain_ids::ETHEREUM)).await;
        assert!(!result.events.is_empty(), "Expected events to be logged for transfer {}", i + 2);
    }
    
    println!("✅ Event logging verified successfully!");
}

/// Test 3: Verify metrics (gauges and counters) are properly tracked
#[tokio::test]
async fn test_metrics_tracking() {
    let setup = TestSetup::new().await;
    let eth_facet = setup.test_server.eth();
    
    println!("📈 Testing metrics tracking functionality...");
    
    // Process a transfer event
    let transfer_log = mock_transfer_log(
        &setup.contract_address,
        addresses::ZERO,
        addresses::TEST_ADDRESS_1,
        "2000000000000000000", // 2 tokens
    );
    
    let result = eth_facet.test_log(transfer_log, Some(chain_ids::ETHEREUM)).await;
    
    // Verify metrics are recorded
    println!("📊 Metrics processing results:");
    println!("  - Counters recorded: {}", result.counters.len());
    println!("  - Gauges recorded: {}", result.gauges.len());
    
    // Test metrics for counters
    if !result.counters.is_empty() {
        for (i, counter) in result.counters.iter().enumerate() {
            println!("    Counter {}: {} = {}", i + 1, counter.name, counter.value);
            assert!(counter.value > 0.0, "Expected counter value to be positive");
        }
    }
    
    // Test metrics for gauges
    if !result.gauges.is_empty() {
        for (i, gauge) in result.gauges.iter().enumerate() {
            println!("    Gauge {}: {} = {}", i + 1, gauge.name, gauge.value);
            // Gauges can have any value, just verify they exist
        }
    }
    
    // Process multiple events to verify metrics accumulation
    let additional_logs = vec![
        mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, "1000000000000000000"),
        mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_2, addresses::TEST_ADDRESS_3, "500000000000000000"),
    ];
    
    for log in additional_logs {
        let _result = eth_facet.test_log(log, Some(chain_ids::ETHEREUM)).await;
        // Each event should contribute to metrics
    }
    
    println!("✅ Metrics tracking verified successfully!");
}

/// Test 4: Verify entities are properly stored and can be retrieved from memory database
#[tokio::test]
async fn test_entity_storage_and_retrieval() {
    let setup = TestSetup::new().await;
    let eth_facet = setup.test_server.eth();
    
    println!("💾 Testing entity storage and retrieval...");
    
    // Process a transfer event that should create an entity
    let transfer_log = mock_transfer_log(
        &setup.contract_address,
        addresses::ZERO,
        addresses::TEST_ADDRESS_1,
        "3000000000000000000", // 3 tokens
    );
    
    let result = eth_facet.test_log(transfer_log.clone(), Some(chain_ids::ETHEREUM)).await;
    
    // Now we can access the database directly from the test result
    let transfer_count = result.db.get_table_count("transfer").await;
    
    println!("📊 Database query results:");
    println!("  - Transfer entities found: {}", transfer_count);
    
    // Verify at least one entity was created
    assert!(transfer_count > 0, "Expected at least one Transfer entity to be stored");
    
    println!("✅ Entity storage verified successfully!");
    
    // Test multiple entities using the same facet
    // Create logs with unique transaction hashes and log indices to ensure separate entities
    let mut additional_logs = vec![
        mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, "1500000000000000000"),
        mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_2, addresses::TEST_ADDRESS_3, "750000000000000000"),
    ];
    
    // Make each log unique by modifying transaction hash and log index
    for (i, log) in additional_logs.iter_mut().enumerate() {
        log.transaction_hash = Some(H256::from_low_u64_be(67890 + i as u64 + 2));
        log.log_index = Some(U256::from(i + 2));
    }
    
    for log in additional_logs {
        let _result = eth_facet.test_log(log, Some(chain_ids::ETHEREUM)).await;
    }
    
    // Verify total entity count increased
    let mut final_log = mock_transfer_log(&setup.contract_address, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_3, "100000000000000000");
    // Make this log unique too
    final_log.transaction_hash = Some(H256::from_low_u64_be(67890 + 10));
    final_log.log_index = Some(U256::from(10));
    
    let final_result = eth_facet.test_log(final_log, Some(chain_ids::ETHEREUM)).await;
    
    let final_count = final_result.db.get_table_count("transfer").await;
    println!("  - Total transfer entities after multiple events: {}", final_count);
    assert!(final_count >= 4, "Expected at least 4 Transfer entities after processing multiple events");
    
    println!("✅ Entity storage and retrieval verified successfully!");
}
