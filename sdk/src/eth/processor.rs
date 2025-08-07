use crate::eth::context::EthContext;
use crate::core::{BaseProcessor, PluginRegister};
use chrono::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use crate::{EthPlugin, Server};

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

pub enum TimeOrBlock {
    Block(u64),
    Time(DateTime<Utc>),
}

#[derive(Clone)]
pub struct RawEvent {
    // TODO: Add event data fields (address, topics, data, etc.)
}

#[derive(Clone)]
pub struct EventFilter {
    // TODO: Add filter criteria (topics, address ranges, etc.)
}

#[derive(Clone)]
pub struct OnEventOption {
    // TODO: Add event processing options
}

pub trait EthOnEvent {
    fn on_event<F>(
        &mut self,
        handler: fn(RawEvent, EthContext) -> F,
        filter: Option<EventFilter>,
        options: Option<OnEventOption>,
    ) -> &mut Self
    where
        F: Future<Output = ()> + Send + 'static;
}

type AsyncEventHandler = Arc<dyn Fn(RawEvent, EthContext) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

struct EventHandler {
    handler: AsyncEventHandler,
    filter: Option<EventFilter>,
    options: Option<OnEventOption>,
}

pub struct EthProcessor {
    options: EthBindOptions,
    event_handlers: Vec<EventHandler>,
}

impl EthProcessor {
    /// Create a new EthProcessor bound to a specific contract address
    pub fn bind(server: &mut Server, args: EthBindOptions) -> &mut Self {
        let processor = Self {
            options: args,
            event_handlers: Vec::new(),
        };
        
        // Register the processor with the EthPlugin via PluginManager
        server.plugin_manager.plugin::<EthPlugin>().register_processor(processor)
    }

    /// Convenience method to bind to an address directly
    pub fn bind_address(server: &mut Server, address: impl Into<String>) -> &mut Self {
        Self::bind(server, EthBindOptions::new(address))
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
    fn chain_id(&self) -> u64 {
        // Default to Ethereum mainnet (1) if no network specified
        match &self.options.network {
            Some(network) => match network.as_str() {
                "mainnet" | "ethereum" => 1,
                "goerli" => 5,
                "sepolia" => 11155111,
                "polygon" => 137,
                "bsc" => 56,
                _ => {
                    // Try to parse as number
                    network.parse().unwrap_or(1)
                }
            },
            None => 1, // Default to Ethereum mainnet
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
        &mut self,
        handler: fn(RawEvent, EthContext) -> F,
        filter: Option<EventFilter>,
        options: Option<OnEventOption>,
    ) -> &mut Self
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
            filter,
            options,
        };

        self.event_handlers.push(event_handler);
        self
    }
}

