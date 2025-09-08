use sentio_sdk::core::Context;
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::async_trait;

#[derive(Clone)]
pub struct {{PROJECT_CLASS_NAME}} {
    address: String,
    chain_id: String,
    name: String,
}

impl {{PROJECT_CLASS_NAME}} {
    pub fn new() -> Self {
        Self {
            address: "0x0000000000000000000000000000000000000000".to_string(), // TODO: Set your contract address
            chain_id: "1".to_string(), // TODO: Set your chain ID (1 for mainnet, 11155111 for sepolia, etc.)
            name: "{{PROJECT_NAME}} Processor".to_string(),
        }
    }
}

impl EthProcessor for {{PROJECT_CLASS_NAME}} {
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

// TODO: Define your event markers and handlers here
// Example:
/*
pub struct YourEvent;

impl EventMarker for YourEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            topics: vec!["0x...".to_string()], // Your event topic hash
        }]
    }
}

#[async_trait]
impl EthEventHandler<YourEvent> for {{PROJECT_CLASS_NAME}} {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("Processing event: {:?}", event);
        
        // TODO: Implement your event processing logic here
        // Examples:
        // - Extract data from event logs
        // - Create and store entities
        // - Emit metrics
        // - Log structured events
    }
}
*/