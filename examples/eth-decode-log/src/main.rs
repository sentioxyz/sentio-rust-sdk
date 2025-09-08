use anyhow::{anyhow, Result};
use ethers::abi::{Event, ParamType, RawLog, Token};
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::core::Context;
use sentio_sdk::{async_trait, Server};
use serde::Serialize;
use std::env;
use tracing::{debug, info, warn};

mod abi_client;
mod processor;
mod generated;
use processor::*;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let server = Server::new();

    // Create a processor that listens to all events (no filters)
    LogDecoderProcessor::new()
        .configure_event::<AllEventsMarker>(None)
        .bind(&server);

    info!("Starting Ethereum log decoder processor...");
    server.start();

    Ok(())
}
