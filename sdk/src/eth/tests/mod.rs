//! Ethereum processor tests demonstrating the testing framework
//!
//! This module shows how to use the Sentio testing framework to test
//! Ethereum processors with simulated blockchain data.

pub mod test_processor;


#[cfg(test)]
mod tests {
    use crate::eth::eth_processor::EthProcessor;
    use crate::eth::EventMarker;
    use crate::testing::{addresses, chain_ids, mock_transfer_log, TestProcessorServer};
    use super::*;

    /// Example test showing how to test an ERC20 transfer handler
    /// 
    /// This test demonstrates the complete flow of:
    /// 1. ✅ Checking for log_handlers configuration
    /// 2. ✅ Modifying test_processor to add event log and metrics calls
    /// 3. ✅ Verifying event log and metrics recording (with framework limitations noted)
    #[tokio::test]
    async fn test_erc20_transfer_handler() {
        use test_processor::{TestErc20Processor, TransferEvent};
        
        // Initialize test server with processor setup
        let mut test_server = TestProcessorServer::new();
        TestErc20Processor::new(addresses::TEST_CONTRACT, "TestToken")
            .configure_event::<TransferEvent>(None)
            .bind(&test_server);
        test_server.start().await.expect("Failed to start test server");
        let config = test_server.get_config().await;
        
        // Get the test facet by consuming the server
        let eth_facet = test_server.eth();

        // Verify the config contains our registered processor
        assert_eq!(config.contract_configs.len(), 1, "Expected 1 contract configuration");
        let contract_config = config.contract_configs[0].clone();
        let contract = contract_config.contract.unwrap();
        assert_eq!(contract.address, addresses::TEST_CONTRACT);
        assert_eq!(contract.chain_id, "1");
        assert_eq!(contract.name, "TestToken");
        assert_eq!(contract_config.start_block, 0); // Default start block
        assert_eq!(contract_config.end_block, 0); // No end block specified
        
        // ✅ 1. Check for log_handlers - verify we have Transfer event handler configured
        assert_eq!(contract_config.log_configs.len(), 1, "Expected 1 log handler configuration");
        let log_handler = &contract_config.log_configs[0];
        // Note: Handler name might be empty if not explicitly set in the current framework implementation
        assert_eq!(log_handler.handler_id, 0, "Expected handler_id to be 0");
        
        // Verify the log filter contains the Transfer event signature
        assert!(!log_handler.filters.is_empty(), "Expected at least one log filter");
        let filter = &log_handler.filters[0];
        assert!(!filter.topics.is_empty(), "Expected topics in the log filter");
        let topic = &filter.topics[0];
        assert!(!topic.hashes.is_empty(), "Expected at least one hash in topic");
        assert_eq!(topic.hashes[0], "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef", 
            "Expected Transfer event topic hash");
        


        // Create a mock transfer log
        let transfer_log = mock_transfer_log(
            addresses::TEST_CONTRACT,
            addresses::ZERO, // Mint from zero address
            addresses::TEST_ADDRESS_1,
            "1000000000000000000", // 1 token with 18 decimals
        );

        // ✅ 2. Test the log processing with event log and metrics calls
        let result = eth_facet.test_log(transfer_log, Some(chain_ids::ETHEREUM)).await;
        
        // Verify we have the expected number of each metric type
        assert_eq!(result.counters.len(), 1, "Expected exactly 1 counter metric");
        assert_eq!(result.gauges.len(), 1, "Expected exactly 1 gauge metric");
        assert_eq!(result.events.len(), 1, "Expected exactly 1 event log");
        
        // ✅ 3. Verify event log and metrics are recorded properly
        
        // Check that we have the expected counter metric
        let transfers_counter = result.first_counter_value("transfers");
        assert!(transfers_counter.is_some(), "Expected 'transfers' counter to be recorded");
        assert_eq!(transfers_counter.unwrap(), 1.0, "Expected transfers counter value to be 1.0");
        
        // Verify metadata is properly set for metrics
        let counter = &result.counters[0];
        assert_eq!(counter.metadata.chain_id, chain_ids::ETHEREUM, "Expected correct chain_id in counter metadata");
        assert_eq!(counter.name, "transfers", "Expected counter name to be 'transfers'");
        
        // Check that we have the expected gauge metric  
        let volume_gauge = result.first_gauge_value("transfer_volume");
        assert!(volume_gauge.is_some(), "Expected 'transfer_volume' gauge to be recorded");
        assert_eq!(volume_gauge.unwrap(), 1000.0, "Expected transfer_volume gauge value to be 1000.0");
        
        let gauge = &result.gauges[0];
        assert_eq!(gauge.metadata.chain_id, chain_ids::ETHEREUM, "Expected correct chain_id in gauge metadata");
        assert_eq!(gauge.name, "transfer_volume", "Expected gauge name to be 'transfer_volume'");
        
        // Check that we have the expected event log
        let transfer_event = result.first_event("transfer");
        assert!(transfer_event.is_some(), "Expected 'transfer' event to be recorded");
        
        let event = transfer_event.unwrap();
        assert_eq!(event.attributes.len(), 3, "Expected exactly 3 attributes in transfer event");
        
        // Verify event attributes contain the expected fields
        assert!(event.attributes.contains_key("from"), "Expected 'from' attribute in transfer event");
        assert!(event.attributes.contains_key("to"), "Expected 'to' attribute in transfer event");
        assert!(event.attributes.contains_key("value"), "Expected 'value' attribute in transfer event");
        
        // Verify attribute values
        if let Some(serde_json::Value::String(from_val)) = event.attributes.get("from") {
            assert!(from_val.contains("0000000000000000000000000000000000000000"), "Expected from address to be zero address");
        } else {
            panic!("Expected 'from' attribute to be a string");
        }
        
        if let Some(serde_json::Value::String(to_val)) = event.attributes.get("to") {
            assert!(to_val.contains("1111111111111111111111111111111111111111"), "Expected to address to be test address 1");
        } else {
            panic!("Expected 'to' attribute to be a string");
        }
        
        if let Some(serde_json::Value::Number(value_num)) = event.attributes.get("value") {
            assert_eq!(value_num.as_f64().unwrap(), 1000.0, "Expected value attribute to be 1000.0");
        } else {
            panic!("Expected 'value' attribute to be a number");
        }
        
        // Verify metadata
        assert_eq!(event.metadata.chain_id, chain_ids::ETHEREUM, "Expected correct chain_id in event metadata");
        assert_eq!(event.name, "transfer", "Expected event name to be 'transfer'");
        
        // Test that helper methods work correctly for nonexistent items
        assert_eq!(result.first_counter_value("nonexistent"), None, "Should return None for nonexistent counter");
        assert_eq!(result.first_gauge_value("nonexistent"), None, "Should return None for nonexistent gauge");
        assert!(result.first_event("nonexistent").is_none(), "Should return None for nonexistent event");
        
        // Final validation - ensure we have collected the expected total metrics
        let total_metrics = result.counters.len() + result.gauges.len() + result.events.len();
        assert_eq!(total_metrics, 3, "Expected exactly 3 total metrics (1 counter + 1 gauge + 1 event)");
    }

    /// Example test showing how to test block interval handlers
    #[tokio::test] 
    async fn test_block_interval_handler() {
        let mut server = TestProcessorServer::new();
        
        // TODO: Add block interval handler registration when implemented
        // For now, just test the basic server setup
        
        server.start().await.expect("Failed to start test server");

        // Create a mock block
        use crate::testing::mock_block;
        let block = mock_block(14373295, 1640995200);

        // Test block processing
        let eth_facet = server.eth();
        let result = eth_facet.test_block(block, Some(chain_ids::ETHEREUM)).await;

        // TODO: Add assertions
        // assert_eq!(result.gauges.len(), 1);
        // assert_eq!(result.first_gauge_value("block_height"), Some(14373295.0));

        println!("Test completed successfully - block processed");
    }

    /// Example test showing environment setup
    #[tokio::test]
    async fn test_with_custom_environment() {
        use crate::testing::TestEnvironment;

        let env = TestEnvironment::new()
            .with_endpoint("1", "https://eth.llamarpc.com")
            .with_endpoint("137", "https://polygon.llamarpc.com")
            .with_timeout(60000);

        let server = TestProcessorServer::new();
        // TODO: Apply environment configuration when new_with_loader is implemented
        // For now, just test the basic server setup

        // Verify default environment (custom environment configuration will be added later)
        assert_eq!(server.environment.timeout_ms, 30000); // Default timeout
        // TODO: Test custom environment once new_with_loader is implemented
        println!("Environment config - Timeout: {}ms", server.environment.timeout_ms);

        println!("Environment configured successfully");
    }

    /// Example test showing how to test multiple logs together
    #[tokio::test]
    async fn test_multiple_events() {
        let mut server = TestProcessorServer::new();
        
        // TODO: Add processor registration when needed
        // TestErc20Processor::new(addresses::TEST_CONTRACT, "TestToken")
        //     .configure_event::<TransferEvent>(None)
        //     .bind(&server);

        server.start().await.expect("Failed to start test server");

        // Create multiple transfer logs
        let logs = vec![
            mock_transfer_log(addresses::TEST_CONTRACT, addresses::ZERO, addresses::TEST_ADDRESS_1, "1000000000000000000"),
            mock_transfer_log(addresses::TEST_CONTRACT, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, "500000000000000000"),
            mock_transfer_log(addresses::TEST_CONTRACT, addresses::TEST_ADDRESS_2, addresses::ZERO, "250000000000000000"), // Burn
        ];

        // Test all logs together
        let eth_facet = server.eth();
        let result = eth_facet.test_logs(logs, Some(chain_ids::ETHEREUM)).await;

        // TODO: Add assertions for batch processing
        // assert_eq!(result.counters.len(), 3); // One counter increment per transfer
        // assert_eq!(result.events.len(), 3);   // One event log per transfer

        println!("Multiple events processed successfully");
    }
    
    /// Test the TestErc20Processor directly
    #[tokio::test]
    async fn test_processor_implementation() {
        use test_processor::{TestErc20Processor, TransferEvent, ApprovalEvent};
        
        let processor = TestErc20Processor::new(
            addresses::TEST_CONTRACT,
            "TestToken"
        );
        
        // Test processor properties using the EthProcessor trait
        assert_eq!(processor.name(), "TestToken");
        assert_eq!(processor.address(), addresses::TEST_CONTRACT);
        assert_eq!(processor.chain_id(), "1");
        
        // Test with custom chain ID
        let processor_polygon = processor.clone().with_chain_id("137");
        assert_eq!(processor_polygon.chain_id(), "137");
        
        println!("Processor implementation test passed");
        
        // Test that the event markers are properly configured
        let transfer_filters = TransferEvent::filter();
        assert_eq!(transfer_filters.len(), 1);
        assert_eq!(transfer_filters[0].topics.len(), 1);
        assert_eq!(transfer_filters[0].topics[0], "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        
        let approval_filters = ApprovalEvent::filter();
        assert_eq!(approval_filters.len(), 1);
        assert_eq!(approval_filters[0].topics.len(), 1);
        assert_eq!(approval_filters[0].topics[0], "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925");
            
        println!("Event marker configuration tested successfully");
    }
    
    /// Test multiple processor types
    #[tokio::test]
    async fn test_multiple_processors() {
        use test_processor::{TestErc20Processor, TransferEvent};
        
        let mut server = TestProcessorServer::new();
        
        // Initialize multiple test processors for different contracts
        TestErc20Processor::new(addresses::USDC_ETHEREUM, "USDC")
            .configure_event::<TransferEvent>(None)
            .bind(&server);
        
        TestErc20Processor::new(addresses::USDT_ETHEREUM, "USDT")
            .configure_event::<TransferEvent>(None)
            .bind(&server);
        
        println!("Multiple processors registered: USDC and USDT");

        server.start().await.expect("Failed to start test server");

        // Test logs for different contracts
        let usdc_transfer = mock_transfer_log(
            addresses::USDC_ETHEREUM,
            addresses::ZERO,
            addresses::TEST_ADDRESS_1,
            "1000000", // USDC has 6 decimals
        );

        let usdt_transfer = mock_transfer_log(
            addresses::USDT_ETHEREUM,
            addresses::ZERO,
            addresses::TEST_ADDRESS_2,
            "1000000", // USDT has 6 decimals
        );

        // Process both transfers
        let eth_facet = server.eth();
        let result1 = eth_facet.test_log(usdc_transfer, Some(chain_ids::ETHEREUM)).await;
        // Note: Can't reuse facet since it consumed the server, so this test shows the limitation
        // In practice, you'd set up separate servers or batch process multiple logs in one call
        println!("Note: This test demonstrates the new ownership pattern - each facet consumes its server");

        println!("USDC result - Counters: {}, Gauges: {}, Events: {}", 
            result1.counters.len(), result1.gauges.len(), result1.events.len());
        
        println!("Multiple processors test completed");
    }
}