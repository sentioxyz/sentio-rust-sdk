use crate::core::BaseProcessor;
use crate::eth::{EthEventHandler, EventMarker};
use crate::{AddressType, EthFetchConfig, EthPlugin};
use alloy::dyn_abi::{DecodedEvent, DynSolEvent};
use alloy::json_abi::Event as JsonEvent;
use alloy::rpc::types::Log;
use anyhow::Result;
use chrono::prelude::*;
use derive_builder::Builder;
use std::future::Future;
use std::sync::Arc;

#[derive(Clone, Builder)]
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
    pub decoded: Option<DecodedEvent>,
}

impl EthEvent {
    /// Decode the log using an ABI string and populate decoded_log field
    pub fn decode_from_abi_str(&self, abi_str: &str) -> Result<EthEvent> {
        let json_event: JsonEvent = serde_json::from_str(abi_str)?;
        self.decode(&json_event)
    }

    /// Decode the log using a pre-parsed `JsonEvent` and populate `decoded` field
    pub fn decode(&self, json_event: &JsonEvent) -> Result<EthEvent> {
        // Catch panics from alloy decode operations and convert to errors
        let decoded_data = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.parse_log_with_json(json_event)
        })) {
            Ok(result) => result?,
            Err(panic_payload) => {
                let panic_message = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic occurred during log decoding".to_string()
                };
                return Err(anyhow::anyhow!("Log decode failed due to panic: {}", panic_message));
            }
        };

        Ok(EthEvent {
            log: self.log.clone(),
            decoded: Some(decoded_data),
        })
    }


    /// Internal method to decode a log using a pre-parsed `JsonEvent`
    fn parse_log_with_json(&self, json_event: &JsonEvent) -> Result<DecodedEvent> {
        // Convert JsonEvent inputs to DynSolTypes for dynamic decoding
        let mut indexed_params: Vec<alloy::dyn_abi::DynSolType> = Vec::new();
        let mut non_indexed_params: Vec<alloy::dyn_abi::DynSolType> = Vec::new();

        for param in &json_event.inputs {
            let dyn_type = param
                .ty
                .to_string()
                .parse::<alloy::dyn_abi::DynSolType>()
                .map_err(|e| anyhow::anyhow!("Failed to parse type '{}': {}", param.ty, e))?;

            if param.indexed {
                indexed_params.push(dyn_type);
            } else {
                non_indexed_params.push(dyn_type);
            }
        }

        // Create the body type as a tuple of non-indexed parameters
        // Always use a tuple, even for a single parameter, to match decoder expectations
        let body_type = alloy::dyn_abi::DynSolType::Tuple(non_indexed_params);

        // Create DynSolEvent with proper parameters (topic_0, indexed_types, body_type)
        let dyn_event = DynSolEvent::new_unchecked(
            self.log.topics().first().copied(),
            indexed_params,
            body_type,
        );

        let log_data = &self.log.inner.data;
        // Decode the log using alloy's dynamic ABI decoding
        dyn_event
            .decode_log_data(log_data)
            .map_err(|e| anyhow::anyhow!("Failed to decode log: {}", e))
    }
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
    processor: Arc<P>,
    event_handlers: Vec<EventHandler>,
}

impl<P: EthProcessor> ConfigurableEthProcessor<P> {
    /// Create a new configurable processor
    pub fn new(processor: P) -> Self {
        Self {
            processor: Arc::new(processor),
            event_handlers: Vec::new(),
        }
    }

    /// Configure an event handler for a specific event type
    pub fn configure_event<T: EventMarker>(mut self, options: Option<OnEventOption>) -> Self
    where
        P: EthEventHandler<T> ,
    {
        let filters = T::filter();

        // Use the existing Arc reference instead of cloning
        let type_erased: Arc<dyn TypeErasedEventHandler> =
            Arc::new((Arc::clone(&self.processor), std::marker::PhantomData::<T>));

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
    pub fn bind<S: crate::BindableServer>(self, server: &S) {
        let processor_arc = self.processor.clone();
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
        self,
        options: Option<OnEventOption>,
    ) -> ConfigurableEthProcessor<Self>
    where
        Self: Sized,
        Self: EthEventHandler<T> ,
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
        if let Some(options) = &self.options
            && let Some(config) = &options.fetch_config {
                return Some(*config);
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
    pub(crate) _processor: Arc<dyn EthProcessor>,
}

impl EthProcessorImpl {
    pub fn new(processor: Arc<dyn EthProcessor>) -> Self {
        let options = EthBindOptions::new(processor.address())
            .with_network(processor.chain_id().to_string())
            .with_name(processor.name().to_string());

        Self {
            options,
            event_handlers: Vec::new(),
            _processor: processor,
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
            .network.as_deref()
            .unwrap_or("1")
    }

    fn name(&self) -> &str {
        self.options
            .name.as_deref()
            .unwrap_or("eth-processor")
    }

    fn handler_count(&self) -> usize {
        self.event_handlers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::consensus::private::alloy_primitives;
    use alloy::dyn_abi::DynSolValue;
    use alloy::primitives::hex::FromHex;
    use alloy::primitives::{Address, Bytes, U256};
    use alloy::primitives::{LogData, B256};
    use std::str::FromStr;

    #[test]
    fn test_decode_from_abi_str() {
        let from = "0x742d35cc6834b8532d5f7c6aa25a6b5f9a2a2b6f";
        let to = "0x742d35cc6834b8532d5f7c6aa25a6b5f9a2a2c7f";
        let amount = "1000000000000000000000";

        // Build proper topics: [signature, from(indexed), to(indexed)]
        let sig = B256::from_str("0x16cdf1707799c6655baac6e210f52b94b7cec08adcaf9ede7dfe8649da926146").unwrap();
        let from_addr = Address::from_str(from).unwrap();
        let to_addr = Address::from_str(to).unwrap();
        let mut from_topic_bytes = [0u8; 32];
        from_topic_bytes[12..].copy_from_slice(from_addr.as_slice());
        let mut to_topic_bytes = [0u8; 32];
        to_topic_bytes[12..].copy_from_slice(to_addr.as_slice());
        let topics = vec![sig, B256::from(from_topic_bytes), B256::from(to_topic_bytes)];

        // Data contains only the non-indexed amount (as 32-byte big-endian)
        let amount_u256 = U256::from_str(amount).unwrap();
        let amount_hex = format!("0x{:064x}", amount_u256);

        let log = Log {
            inner: alloy_primitives::Log {
                address: Address::from_str("0xbd0edfbac386c9964f8f013d65d7dad5382d9cd7").unwrap(),
                data: LogData::new_unchecked(
                    topics,
                    Bytes::from_hex(amount_hex).unwrap(),
                ),
            },
            block_hash: Some(B256::with_last_byte(0x69)),
            block_number: Some(0x69),
            block_timestamp: None,
            transaction_hash: Some(B256::with_last_byte(0x69)),
            transaction_index: Some(0x69),
            log_index: Some(0x69),
            removed: false,
        };
        let eth_event = EthEvent { log, decoded: None };

        let transfer_abi = r#"{
            "type": "event",
            "name": "CoinTransfer",
            "anonymous": false,
            "inputs": [
                {"name": "sender", "type": "address", "indexed": true},
                {"name": "receiver", "type": "address", "indexed": true},
                {"name": "amount", "type": "uint256", "indexed": false}
            ]
        }"#;

        let decoded_event = eth_event
            .decode_from_abi_str(transfer_abi)
            .expect("expected successful decode for valid log + ABI");

        let decoded = decoded_event
            .decoded
            .as_ref()
            .expect("decoded event payload should be present");

        // Verify indexed topics (from, to)
        assert_eq!(decoded.indexed.len(), 2);
        match &decoded.indexed[0] {
            DynSolValue::Address(addr) => {
                assert_eq!(format!("0x{:x}", addr), from);
            }
            other => panic!("unexpected type for indexed[0]: {:?}", other),
        }
        match &decoded.indexed[1] {
            DynSolValue::Address(addr) => {
                assert_eq!(format!("0x{:x}", addr), to);
            }
            other => panic!("unexpected type for indexed[1]: {:?}", other),
        }

        // Verify data (value)
        assert_eq!(decoded.body.len(), 1);
        match &decoded.body[0] {
            DynSolValue::Uint(v, _) => {
                let expected = U256::from_str(amount).expect("valid u256 amount");
                assert_eq!(*v, expected, "decoded amount should match");
            }
            other => panic!("unexpected type for body[0]: {:?}", other),
        }
    }
}
