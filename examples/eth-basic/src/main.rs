use sentio_sdk::Server;

mod processor;
mod entities;
use processor::*;
use sentio_sdk::eth::eth_processor::EthProcessor;

fn main() {
    let server = Server::new();

    let processor = MyEthProcessor::new();
    processor
        .configure_event::<TransferEvent>(None)
        .configure_event::<ApprovalEvent>(None)
        .bind(&server);
    
    server.start();
}