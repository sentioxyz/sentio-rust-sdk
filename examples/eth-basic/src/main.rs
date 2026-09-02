use eth_basic::chains_config::ChainsConfig;
use eth_basic::{Erc20TransferProcessor, TransferEvent, CHAINS};
use sentio_sdk::eth::eth_processor::EthProcessor;
use sentio_sdk::Server;

fn main() {
    let server = Server::new();
    server.set_gql_schema(eth_basic::generated::GQL_SCHEMA);

    // RPC endpoints arrive via `--chains-config=<path>` from the platform.
    let chains = ChainsConfig::from_args();
    for (chain_id, start_block) in CHAINS {
        let processor = Erc20TransferProcessor::new(chain_id, *start_block, &chains);
        if !processor.has_rpc() {
            eprintln!("no RPC endpoint for chain {}; token metadata will be recorded as 'unknown'", chain_id);
        }
        processor.configure_event::<TransferEvent>(None).bind(&server);
    }

    server.start();
}
