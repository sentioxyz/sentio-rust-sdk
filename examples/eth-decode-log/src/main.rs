use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::{ExecutionConfig, Server};

mod abi_client;
mod processor;
mod generated;
mod bench;
use processor::*;

fn main()  {

    let mut server = Server::new();

    // Provide global GraphQL schema to server
    server.set_gql_schema(generated::GQL_SCHEMA);
    // Set execution configuration
    let mut config = ExecutionConfig::default();
    config.process_binding_timeout = 10;
    server.set_execution_config(config);
    // Create a processor that listens to all events (no filters)
    LogDecoderProcessor::new()
        .configure_event::<AllEventsMarker>(None)
        .bind(&server);
    server.start();
}
