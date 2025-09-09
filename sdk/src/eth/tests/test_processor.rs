//! Test processor for Ethereum event handling
//!
//! This module provides a sample processor that demonstrates how to use
//! the Ethereum event handlers and can be used for testing the framework.
 

#[cfg(test)]
/// Sample ERC20 processor for testing event handlers
#[derive(Clone)]
pub struct TestErc20Processor {
    address: String,
    chain_id: String,
    name: String,
}

#[cfg(test)]
impl TestErc20Processor {
    pub fn new(contract_address: &str, name: &str) -> Self {
        Self {
            address: contract_address.to_string(),
            chain_id: "1".to_string(), // Default to Ethereum mainnet
            name: name.to_string(),
        }
    }

    pub fn with_chain_id(mut self, chain_id: &str) -> Self {
        self.chain_id = chain_id.to_string();
        self
    }
}

#[cfg(test)]
impl EthProcessor for TestErc20Processor {
    fn address(&self) -> &str {
        &self.address
    }

    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn name(&self) -> &str {
        &self.name
    }
}


#[cfg(test)]
pub struct TransferEvent;
#[cfg(test)]
pub struct ApprovalEvent;

#[cfg(test)]
impl EventMarker for TransferEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            // Transfer event signature: Transfer(address indexed from, address indexed to, uint256 value)
            topics: vec!["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string()],
        }]
    }
}

#[cfg(test)]
impl EventMarker for ApprovalEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            // Approval event signature: Approval(address indexed owner, address indexed spender, uint256 value)
            topics: vec!["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925".to_string()],
        }]
    }
}

#[cfg(test)]
#[async_trait]
impl EthEventHandler<TransferEvent> for TestErc20Processor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("🔄 Processing TRANSFER event from contract: {:?} on chain: {}",
            event.log.address(), ctx.chain_id());

        println!("Transfer event details - Block: {:?}, Transaction: {:?}, Log Index: {:?}",
            event.log.block_number,
            event.log.transaction_hash,
            event.log.log_index
        );

        // Extract transfer data from topics and data
        // topics[0] = event signature (already filtered)
        // topics[1] = from address (indexed)
        // topics[2] = to address (indexed)
        // data = value (not indexed)

        if event.log.topics().len() >= 3 {
            let from = event.log.topics()[1];
            let to = event.log.topics()[2];

            println!("Transfer: {:?} -> {:?}", from, to);

            // Emit actual metrics and event logs for testing
            ctx.base_context().counter("transfers").add(1.0, None).await.ok();
            ctx.base_context().gauge("transfer_volume").record(1000.0, None).await.ok(); // Mock value

            // Emit event log with attributes
            use crate::core::event_logger::{Event, AttributeValue};
            let event = Event::name("transfer")
                .attr("from", AttributeValue::String(format!("{:?}", from)))
                .attr("to", AttributeValue::String(format!("{:?}", to)))
                .attr("value", AttributeValue::Number(1000.0));
            ctx.base_context().event_logger().emit(&event).await.ok();
        }

        ctx.set_config_updated(true);
        println!("Transfer event processing completed!");
    }
}

#[cfg(test)]
#[async_trait]
impl EthEventHandler<ApprovalEvent> for TestErc20Processor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("✅ Processing APPROVAL event from contract: {:?} on chain: {}",
            event.log.address(), ctx.chain_id());

        println!("Approval event details - Block: {:?}, Transaction: {:?}, Log Index: {:?}",
            event.log.block_number,
            event.log.transaction_hash,
            event.log.log_index
        );

        // Extract approval data from topics and data
        // topics[0] = event signature (already filtered)
        // topics[1] = owner address (indexed)
        // topics[2] = spender address (indexed)
        // data = value (not indexed)

        if event.log.topics().len() >= 3 {
            let owner = event.log.topics()[1];
            let spender = event.log.topics()[2];

            println!("Approval: {:?} -> {:?}", owner, spender);

            // Emit actual metrics and event logs for testing
            ctx.base_context().counter("approvals").add(1.0, None).await.ok();
            ctx.base_context().gauge("approval_amount").record(500.0, None).await.ok(); // Mock value

            // Emit event log with attributes
            use crate::core::event_logger::{Event, AttributeValue};
            let event = Event::name("approval")
                .attr("owner", AttributeValue::String(format!("{:?}", owner)))
                .attr("spender", AttributeValue::String(format!("{:?}", spender)))
                .attr("value", AttributeValue::Number(500.0));
            ctx.base_context().event_logger().emit(&event).await.ok();
        }

        ctx.set_config_updated(true);
        println!("Approval event processing completed!");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::addresses;



    #[tokio::test]
    async fn test_processor_creation() {
        let processor = TestErc20Processor::new(
            addresses::TEST_CONTRACT,
            "TestToken"
        );
        
        // Test using the EthProcessor trait methods
        assert_eq!(processor.name(), "TestToken");
        assert_eq!(processor.address(), addresses::TEST_CONTRACT);
        assert_eq!(processor.chain_id(), "1");
    }

    #[tokio::test]
    async fn test_event_filters() {
        // Test the event marker implementations
        let transfer_filters = TransferEvent::filter();
        assert_eq!(transfer_filters.len(), 1);
        assert_eq!(transfer_filters[0].topics.len(), 1);
        assert_eq!(transfer_filters[0].topics[0], "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");
        
        let approval_filters = ApprovalEvent::filter();
        assert_eq!(approval_filters.len(), 1);
        assert_eq!(approval_filters[0].topics.len(), 1);
        assert_eq!(approval_filters[0].topics[0], "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925");
    }

    #[tokio::test]
    async fn test_chain_id_customization() {
        let processor = TestErc20Processor::new(
            addresses::TEST_CONTRACT,
            "TestToken"
        );
        
        // Test default chain ID
        assert_eq!(processor.chain_id(), "1");
        
        // Test custom chain ID
        let processor_polygon = processor.with_chain_id("137");
        assert_eq!(processor_polygon.chain_id(), "137");
    }
}