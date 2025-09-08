use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::{Server};
use tracing::info;

mod abi_client;
mod processor;
mod generated;
use processor::*;

fn main()  {
    tracing_subscriber::fmt::init();

    let server = Server::new();

    // Create a processor that listens to all events (no filters)
    LogDecoderProcessor::new()
        .configure_event::<AllEventsMarker>(None)
        .bind(&server);

    info!("Starting Ethereum log decoder processor...");
    server.start();
}
