use crate::eth::context::EthContext;
use crate::core::BaseProcessor;
use chrono::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::{AddressType, EthFetchConfig, EthPlugin, Server};

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
pub struct RawEvent {
    pub address: String,
    pub data: String,
    pub topics: Vec<String>,
}

#[derive(Clone)]
pub struct EventFilter {
    pub(crate) address : Option<String>,
    pub(crate) address_type: Option<AddressType>,
    pub(crate) topics: Vec<String>,
}

#[derive(Clone)]
pub struct OnEventOption {
    fetch_config: Option<EthFetchConfig>
}

pub trait EthOnEvent {
    fn on_event<F>(
        self,
        handler: fn(RawEvent, EthContext) -> F,
        filter: Vec<EventFilter>,
        options: Option<OnEventOption>,
    ) -> Self
    where
        F: Future<Output = ()> + Send + 'static;
}

type AsyncEventHandler = Arc<dyn Fn(RawEvent, EthContext) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

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

    /// Process an event by calling all registered handlers
    pub async fn process_event(&self, event: RawEvent, context: EthContext) {
        use tracing::{error, debug};
        
        debug!("Processing event with {} handlers", self.event_handlers.len());
        
        for (idx, handler) in self.event_handlers.iter().enumerate() {
            // TODO: Apply filter logic here
            debug!("Calling handler {}", idx);
            
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (handler.handler)(event.clone(), context.clone())
            })) {
                Ok(future) => {
                    future.await;
                    debug!("Handler {} completed successfully", idx);
                }
                Err(_) => {
                    error!("Handler {} panicked during execution", idx);
                }
            }
        }
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
    fn on_event<F>(
        mut self,
        handler: fn(RawEvent, EthContext) -> F,
        filters: Vec<EventFilter>,
        options: Option<OnEventOption>,
    ) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // Wrap the handler function to match our type signature
        let async_handler: AsyncEventHandler = Arc::new(move |event, context| {
            let future = handler(event, context);
            Box::pin(future)
        });

        let event_handler = EventHandler {
            handler: async_handler,
            filters,
            options,
            name: None,
        };

        self.event_handlers.push(event_handler);
        self
    }
}
