use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::EthEventHandler;
use sentio_sdk::core::Context;
use sentio_sdk::{async_trait, Server};


struct MyEthEventHandler {}

#[async_trait]
impl EthEventHandler for MyEthEventHandler {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!(
            "Processing event from contract: {:?} on chain: {}",
            event.log.address,
            ctx.chain_id()
        );
        
        println!(
            "Event details - Block: {}, Transaction: {:?}, Log Index: {}",
            event.log.block_number.unwrap_or_default(),
            event.log.transaction_hash,
            event.log.log_index.unwrap_or_default()
        );
        
        // Can access and modify context
        ctx.set_config_updated(true);
        
        println!("Event processing completed!");
    }
}




fn main() {
    let server = Server::new();

    // Create a processor with trait-based event handler and bind it to the server
    EthProcessor::new()
        .on_event(
            MyEthEventHandler {},
            Vec::new(), // No specific filters - process all events
            None,       // No special options
        )
        .bind(
            &server,
            EthBindOptions::new("0x1234567890123456789012345678901234567890")
                .with_name("My ETH Processor")
                .with_network("1"),
        );

    println!("Starting Ethereum processor with trait-based handler...");
    server.start();
}
