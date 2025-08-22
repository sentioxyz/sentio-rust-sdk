use crate::core::BaseProcessor;
use chrono::prelude::*;
use std::sync::Arc;
use ethers::types::Log;
use ethers::abi::Log as DecodedLog;
use crate::{AddressType, EthFetchConfig, EthPlugin, Server};
use crate::eth::EthEventHandler;

#[derive(Clone)]
pub struct EthBindOptions {
    pub address: String,
    /// Optional, if not set, then use eth mainnet
    pub network: Option<String>,
    /// Optional, override default contract name  
    pub name: Option<String>,
    pub start: Option<TimeOrBlock>,
    pub end: Option<TimeOrBlock>,
}

impl EthBindOptions {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            network: None,
            name: None,
            start: None,
            end: None,
        }
    }

    pub fn with_network(mut self, network: impl Into<String>) -> Self {
        self.network = Some(network.into());
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn from_block(mut self, block: u64) -> Self {
        self.start = Some(TimeOrBlock::Block(block));
        self
    }

    pub fn from_time(mut self, time: DateTime<Utc>) -> Self {
        self.start = Some(TimeOrBlock::Time(time));
        self
    }

    pub fn to_block(mut self, block: u64) -> Self {
        self.end = Some(TimeOrBlock::Block(block));
        self
    }

    pub fn to_time(mut self, time: DateTime<Utc>) -> Self {
        self.end = Some(TimeOrBlock::Time(time));
        self
    }
}

#[derive(Clone)]
pub enum TimeOrBlock {
    Block(u64),
    Time(DateTime<Utc>),
}

#[derive(Clone)]
pub struct EthEvent {
    pub log: Log,
    pub decoded_log: Option<DecodedLog>
}

#[derive(Clone)]
pub struct EventFilter {
    pub(crate) address : Option<String>,
    pub(crate) address_type: Option<AddressType>,
    pub(crate) topics: Vec<String>,
}

#[derive(Clone)]
pub struct OnEventOption {
    fetch_config: Option<EthFetchConfig>,
    decode_log: bool,
}



pub trait EthOnEvent {
    /// Register an event handler using a trait implementation (new simplified API)
    fn on_event(
        self,
        handler: impl EthEventHandler,
        filter: Vec<EventFilter>,
        options: Option<OnEventOption>,
    ) -> Self;
}

type AsyncEventHandler = Arc<dyn EthEventHandler>;

#[derive(Clone)]
pub(crate) struct EventHandler {
    pub(crate) handler: AsyncEventHandler,
    pub(crate) filters: Vec<EventFilter>,
    pub(crate) options: Option<OnEventOption>,
    pub(crate) name: Option<String>,
}

impl EventHandler {
    pub(crate) fn fetch_config(&self) -> Option<EthFetchConfig> {
        if let Some(options) = &self.options {
            if let Some(config) = &options.fetch_config {
                return Some(config.clone());
            }
        }
        None
    }

    pub(crate) fn need_decode_log(&self) -> bool {
        let opt = &self.options;
        opt.is_some() && opt.as_ref().unwrap().decode_log
    }
}

#[derive(Clone)]
pub struct EthProcessor {
    pub(crate) options: EthBindOptions,
    pub(crate) event_handlers: Vec<EventHandler>,
}


impl EthProcessor {
    /// Create a new EthProcessor for a specific contract address
    pub fn new() -> Self {
        Self {
            options: EthBindOptions::new(""), // Empty address to be set later in bind
            event_handlers: Vec::new(),
        }
    }

    /// Bind this processor to a server with the given options
    /// This consumes the processor and registers it with the server
    pub fn bind(mut self, server: &Server, options: EthBindOptions) {
        self.options = options;
        server.register_processor::<Self, EthPlugin>(self);
    }



    /// Get the number of registered event handlers
    pub fn handler_count(&self) -> usize {
        self.event_handlers.len()
    }

    /// Get a reference to the bind options
    pub fn options(&self) -> &EthBindOptions {
        &self.options
    }

    /// Get the contract address this processor is bound to
    pub fn address(&self) -> &str {
        &self.options.address
    }

    /// Check if this processor has any event handlers
    pub fn has_handlers(&self) -> bool {
        !self.event_handlers.is_empty()
    }
}

impl BaseProcessor for EthProcessor {
    fn chain_id(&self) -> &str {
        if let Some(network) = &self.options.network {
            network
        } else {
            "1"
        }
    }

    fn name(&self) -> &str {
        match &self.options.name {
            Some(name) => name,
            None => "eth-processor",
        }
    }

    fn handler_count(&self) -> usize {
        self.event_handlers.len()
    }
}



impl EthOnEvent for EthProcessor {
    
    fn on_event(
        mut self,
        handler: impl EthEventHandler,
        filters: Vec<EventFilter>,
        options: Option<OnEventOption>,
    ) -> Self
    {
        // Convert the trait implementation to our internal handler type
        let handler = Arc::new(handler);

        let event_handler = EventHandler {
            handler,
            filters,
            options,
            name: None,
        };

        self.event_handlers.push(event_handler);
        self
    }
}
