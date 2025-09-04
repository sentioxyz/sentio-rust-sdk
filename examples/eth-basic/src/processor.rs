use sentio_sdk::core::Context;
use sentio_sdk::entity::{Store, StoreContext};
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::async_trait;

#[derive(Clone)]
pub(crate) struct MyEthProcessor {
    address: String,
    chain_id: String,
    name: String,
    store_context: Option<StoreContext>,
}

impl MyEthProcessor {
    pub fn new() -> Self {
        // Initialize with entity store for demonstration
        let store = Store::default();
        let store_context = StoreContext::new(store);

        Self {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            chain_id: "1".to_string(),
            name: "Sentio ETH + Entity Framework Demo".to_string(),
            store_context: Some(store_context),
        }
    }

    // Helper method to demonstrate entity operations
    pub(crate) fn demo_entity_operations(&self) {
        if let Some(ref store_ctx) = self.store_context {
            println!("🏪 Demonstrating Entity Framework capabilities:");

            // Example: Create a mock transfer entity (normally this would use generated entities)
            // let transfer = Transfer::new(
            //     "0x123...".to_string(),
            //     "transfer_123".to_string(),
            //     1000.into(),
            //     Utc::now(),
            //     // ... other fields
            // );

            // Example entity operations would go here:
            // transfer.save(store_ctx.store()).await;
            // let loaded = Transfer::load(&"transfer_123".to_string(), store_ctx.store()).await;

            println!("   📦 Generated entities available: Transfer, Approval, Account, TokenContract, DailyStats");
            println!("   🔧 Entity code generation happens at build time via build.rs");
            println!("   💾 In-memory store ready for entity persistence");
            println!("   🔍 Type-safe queries and relationships supported");
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
pub struct TransferEvent;
pub struct ApprovalEvent;

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



        // Example of using builder pattern with derive_builder
        // let transfer = TransferBuilder::default()
        //     .id(format!("{:?}-{}", event.log.transaction_hash, event.log.log_index.unwrap_or_default()))
        //     .transaction_hash(format!("{:?}", event.log.transaction_hash.unwrap_or_default()))
        //     .block_number(event.log.block_number.unwrap_or_default().into())
        //     .timestamp(Utc::now())
        //     .build()
        //     .expect("Failed to build transfer entity");
        // transfer.save(store_ctx.store()).await.unwrap();

     }
}

#[async_trait]
impl EthEventHandler<ApprovalEvent> for MyEthProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {

    }
}
