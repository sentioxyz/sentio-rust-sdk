use crate::core::BaseProcessor;
use crate::eth::{EthEventHandler, EventMarker};
use crate::{AddressType, EthFetchConfig, EthPlugin, Server};
use chrono::prelude::*;
use ethers::abi::Log as DecodedLog;
use ethers::types::Log;
use std::future::Future;
use std::sync::Arc;

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
    pub decoded_log: Option<DecodedLog>,
}

#[derive(Clone)]
pub struct EventFilter {
    pub address: Option<String>,
    pub address_type: Option<AddressType>,
    pub topics: Vec<String>,
}

#[derive(Clone)]
pub struct OnEventOption {
    fetch_config: Option<EthFetchConfig>,
    decode_log: bool,
}

/// A configurable Ethereum processor that can register event handlers
pub struct ConfigurableEthProcessor<P: EthProcessor> {
    processor: P,
    event_handlers: Vec<EventHandler>,
}

impl<P: EthProcessor> ConfigurableEthProcessor<P> {
    /// Create a new configurable processor
    pub fn new(processor: P) -> Self {
        Self {
            processor,
            event_handlers: Vec::new(),
        }
    }

    /// Configure an event handler for a specific event type
    pub fn configure_event<T: EventMarker>(mut self, options: Option<OnEventOption>) -> Self
    where
        P: EthEventHandler<T> + Clone,
    {
        let filters = T::filter();

        // Create a cloned processor to use as the event handler
        let handler_processor = self.processor.clone();

        let type_erased: Arc<dyn TypeErasedEventHandler> =
            Arc::new((handler_processor, std::marker::PhantomData::<T>));

        let event_handler = EventHandler {
            handler: type_erased,
            filters: filters.clone(),
            options,
            name: None,
        };

        self.event_handlers.push(event_handler);
        self
    }

    /// Bind this configured processor to a server
    pub fn bind(self, server: &Server) {
        let processor_arc = Arc::new(self.processor);
        let mut processor_impl = EthProcessorImpl::new(processor_arc);
        processor_impl.event_handlers = self.event_handlers;

        server.register_processor::<EthProcessorImpl, EthPlugin>(processor_impl);
    }
}

/// Core trait that all Ethereum processors must implement
pub trait EthProcessor: Send + Sync + 'static {
    /// Get the contract address this processor handles
    fn address(&self) -> &str;

    /// Get the blockchain network/chain ID
    fn chain_id(&self) -> &str;

    /// Get the processor name
    fn name(&self) -> &str;

    fn configure_event<T: EventMarker>(
        mut self,
        options: Option<OnEventOption>,
    ) -> ConfigurableEthProcessor<Self>
    where
        Self: Sized,
        Self: EthEventHandler<T> + Clone,
    {
        let cfg = ConfigurableEthProcessor::new(self);
        cfg.configure_event::<T>(options)
    }
}

// Type-erased handler that can store any EthEventHandler<T: EventMarker>
pub trait TypeErasedEventHandler: Send + Sync {
    fn handle_event(
        &self,
        event: EthEvent,
        ctx: crate::eth::context::EthContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>>;
    fn get_filters(&self) -> Vec<EventFilter>;
}

// Implementation for handlers with EventMarker
impl<H, T> TypeErasedEventHandler for (H, std::marker::PhantomData<T>)
where
    H: EthEventHandler<T>,
    T: EventMarker,
{
    fn handle_event(
        &self,
        event: EthEvent,
        ctx: crate::eth::context::EthContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.0.on_event(event, ctx))
    }

    fn get_filters(&self) -> Vec<EventFilter> {
        T::filter()
    }
}


type AsyncEventHandler = Arc<dyn TypeErasedEventHandler>;

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

// Internal storage for processor data - used by the plugin system
#[derive(Clone)]
pub(crate) struct EthProcessorImpl {
    pub(crate) options: EthBindOptions,
    pub(crate) event_handlers: Vec<EventHandler>,
    pub(crate) processor: Arc<dyn EthProcessor>,
}

impl EthProcessorImpl {
    pub fn new(processor: Arc<dyn EthProcessor>) -> Self {
        let options = EthBindOptions::new(processor.address())
            .with_network(processor.chain_id().to_string())
            .with_name(processor.name().to_string());

        Self {
            options,
            event_handlers: Vec::new(),
            processor,
        }
    }


    /// Add an event handler for a specific event type
    #[cfg(test)]
    pub fn add_event_handler<T: EventMarker>(
        &mut self,
        handler: impl EthEventHandler<T>,
        options: Option<OnEventOption>,
    ) {
        let filters = T::filter();

        let type_erased: Arc<dyn TypeErasedEventHandler> =
            Arc::new((handler, std::marker::PhantomData::<T>));

        let event_handler = EventHandler {
            handler: type_erased,
            filters: filters.clone(),
            options,
            name: None,
        };

        self.event_handlers.push(event_handler);
    }

}

impl BaseProcessor for EthProcessorImpl {
    fn chain_id(&self) -> &str {
        self.options
            .network
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("1")
    }

    fn name(&self) -> &str {
        self.options
            .name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("eth-processor")
    }

    fn handler_count(&self) -> usize {
        self.event_handlers.len()
    }
}
