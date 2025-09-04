use sentio_sdk::Server;

mod processor;
mod entities;
use processor::*;
use sentio_sdk::eth::eth_processor::EthProcessor;

fn main() {
    let server = Server::new();

    println!("🚀 Sentio ETH + Entity Framework Demo Starting...");
    println!("================================================");
    
    // Create processor instance
    let processor = MyEthProcessor::new();
    
    // Demonstrate entity framework capabilities
    processor.demo_entity_operations();

    // Create a processor with multiple event handlers for the same struct
    // Each handler uses a different EventMarker to define its filter
    let processor = MyEthProcessor::new();
    processor
        .configure_event::<TransferEvent>(None)
        .configure_event::<ApprovalEvent>(None)
        .bind(&server);
    
    server.start();
}