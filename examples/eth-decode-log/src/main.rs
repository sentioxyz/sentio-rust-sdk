use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::{Server};

mod abi_client;
mod processor;
mod generated;
use processor::*;

fn main()  {

    let server = Server::new();

    // Create a processor that listens to all events (no filters)
    LogDecoderProcessor::new()
        .configure_event::<AllEventsMarker>(None)
        .bind(&server);

    server.start();
}
