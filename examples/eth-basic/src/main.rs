use sentio_sdk::Server;
use sentio_sdk::eth::eth_processor::*;

#[tokio::main]
async fn main() {
    let server = Server::new();

    // Create a processor bound to a specific contract address
    let processor = EthProcessor::bind(
        EthBindOptions::new("0x1234567890123456789012345678901234567890")
            .with_name("My ETH Processor")
            .with_network("1"),
        |processor| {
            processor.on_event(|_event, _ctx| async {
                println!("Processing event!");
            }, vec![], None);
        }
    );

    // Register the processor with the server
    processor.register_with_server(&server).await;

    server.start();
}