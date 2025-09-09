use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::{Server};

mod abi_client;
mod processor;
mod generated;
use processor::*;

fn main()  {

    let server = Server::new();

    // Provide global GraphQL schema to server
    server.set_gql_schema(generated::GQL_SCHEMA);

    // Create a processor that listens to all events (no filters)
    LogDecoderProcessor::new()
        .configure_event::<AllEventsMarker>(None)
        .bind(&server);
    server.start();
}
