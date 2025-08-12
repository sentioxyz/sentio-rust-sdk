use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::Server;

fn main() {
    let server = Server::new();

    // Create a processor with event handlers and bind it to the server
    EthProcessor::new()
        .on_event(
            |_event, _ctx| async {
                println!("Processing event!");
            },
            Vec::new(),
            None,
        )
        .bind(
            &server,
            EthBindOptions::new("0x1234567890123456789012345678901234567890")
                .with_name("My ETH Processor")
                .with_network("1"),
        );

    server.start();
}
