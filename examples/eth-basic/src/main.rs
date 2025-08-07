use sentio_sdk::Server;
use sentio_sdk::eth::processor::*;

fn main() {
    let mut server = Server::new();

    // Create a processor bound to a specific contract address
    EthProcessor::bind(
        &mut server,
        EthBindOptions::new("0x1234567890123456789012345678901234567890")
            .with_name("My ETH Processor")
            .with_network("1")
    ).on_event(|_event, _ctx| async {
        println!("Processing event!");
    }, None, None);

    server.start();
}