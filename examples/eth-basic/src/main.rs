use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::core::Context;
use sentio_sdk::{async_trait, Server};

#[derive(Clone)]
struct MyEthProcessor {
    address: String,
    chain_id: String,
    name: String,
}

impl MyEthProcessor {
    pub fn new() -> Self {
        Self {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            chain_id: "1".to_string(),
            name: "My ETH Processor".to_string(),
        }
    }
}

impl EthProcessor for MyEthProcessor {
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

// Define different event marker types for the same processor
struct TransferEvent;
struct ApprovalEvent;

impl EventMarker for TransferEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            topics: vec!["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string()], // Transfer event topic
        }]
    }
}

impl EventMarker for ApprovalEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            topics: vec!["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925".to_string()], // Approval event topic
        }]
    }
}

#[async_trait]
impl EthEventHandler<TransferEvent> for MyEthProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("🔄 Processing TRANSFER event from contract: {:?} on chain: {}", 
            event.log.address, ctx.chain_id());
        
        println!("Transfer event details - Block: {}, Transaction: {:?}, Log Index: {}",
            event.log.block_number.unwrap_or_default(),
            event.log.transaction_hash,
            event.log.log_index.unwrap_or_default()
        );
        
        ctx.set_config_updated(true);
        println!("Transfer event processing completed!");
    }
}

#[async_trait]
impl EthEventHandler<ApprovalEvent> for MyEthProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("✅ Processing APPROVAL event from contract: {:?} on chain: {}", 
            event.log.address, ctx.chain_id());
        
        println!("Approval event details - Block: {}, Transaction: {:?}, Log Index: {}",
            event.log.block_number.unwrap_or_default(),
            event.log.transaction_hash,
            event.log.log_index.unwrap_or_default()
        );
        
        ctx.set_config_updated(true);
        println!("Approval event processing completed!");
    }
}

fn main() {
    let server = Server::new();

    // Create a processor with multiple event handlers for the same struct
    // Each handler uses a different EventMarker to define its filter
    MyEthProcessor::new()
        .configure_event::<TransferEvent>(None)
        .configure_event::<ApprovalEvent>(None)
        .bind(&server);

    println!("Starting Ethereum processor with multiple event handlers for Transfer and Approval events...");
    server.start();
}