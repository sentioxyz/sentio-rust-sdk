use eth_basic::{Erc20TransferProcessor, TransferEvent, CHAINS};
use sentio_sdk::eth::eth_processor::EthProcessor;
use sentio_sdk::Server;

fn main() {
    let server = Server::new();
    for (chain_id, start_block) in CHAINS {
        Erc20TransferProcessor::new(chain_id, *start_block)
            .configure_event::<TransferEvent>(None)
            .bind(&server);
    }
    server.start();
}
