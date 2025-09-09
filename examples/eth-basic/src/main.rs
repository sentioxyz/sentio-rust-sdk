use sentio_sdk::Server;
use eth_basic::*;
use sentio_sdk::eth::eth_processor::EthProcessor;

fn main() {
    let server = Server::new();

    // Provide global GraphQL schema to server
    server.set_gql_schema(generated::GQL_SCHEMA);

    let processor = MyEthProcessor::new();
    processor
        .configure_event::<TransferEvent>(None)
        .configure_event::<ApprovalEvent>(None)
        .bind(&server);
    
    server.start();
}
