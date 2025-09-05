use crate::core::plugin::FullPlugin;
use crate::core::{
    AsyncPluginProcessor, BaseProcessor, HandlerRegister, MetaData, Plugin, PluginRegister,
    StateCollector, StateUpdateCollector, RUNTIME_CONTEXT,
};
use crate::eth::eth_processor::{EthProcessorImpl, EthEvent, TimeOrBlock};
use crate::eth::ParsedEthData;
use crate::log_filter::AddressOrType;
use crate::processor::HandlerType;
use crate::{
    ConfigureHandlersRequest, ConfigureHandlersResponse, ContractConfig, ContractInfo, LogFilter,
    LogHandlerConfig, Topic,
};
use anyhow;
use tracing::debug;

#[derive(Default)]
pub struct EthPlugin {
    handler_register: HandlerRegister<HandlerType>,
    processors: Vec<Box<EthProcessorImpl>>,
}

impl EthPlugin {
    /// Get a reference to the handler register
    pub fn handler_register(&self) -> &HandlerRegister<HandlerType> {
        &self.handler_register
    }

    /// Get a mutable reference to the handler register
    pub fn handler_register_mut(&mut self) -> &mut HandlerRegister<HandlerType> {
        &mut self.handler_register
    }

    /// Get the number of registered processors
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }
}

impl Plugin for EthPlugin {
    fn handler_types(&self) -> &'static [HandlerType] {
        &[
            HandlerType::EthLog,
            HandlerType::EthBlock,
            HandlerType::EthTrace,
            HandlerType::EthTransaction,
        ]
    }

    fn processor_count(&self) -> usize {
        self.processors.len()
    }

    fn chain_ids(&self) -> Vec<String> {
        self.processors
            .iter()
            .map(|p| p.chain_id().to_string())
            .collect()
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "eth-plugin"
    }

    fn configure(
        &mut self,
        request: &ConfigureHandlersRequest,
        config: &mut ConfigureHandlersResponse,
    ) {
        debug!(
            "Configuring EthPlugin handlers for chain_id: {:?}",
            request.chain_id
        );
        let chain_id = if request.chain_id.is_empty() {
            None
        } else {
            Some(request.chain_id.clone())
        };

        for (processor_idx, processor) in self.processors.iter().enumerate() {
            let processor_chain_id = processor.chain_id();

            // Filter by chain_id if provided, otherwise process all
            if let Some(filter_chain_id) = chain_id.as_ref() {
                if processor_chain_id != filter_chain_id {
                    continue;
                }
            }

            let mut contract_config = ContractConfig::default();
            contract_config.processor_type = crate::core::USER_PROCESSOR.to_owned();
            if let Some(TimeOrBlock::Block(block)) = processor.options.start {
                contract_config.start_block = block;
            }
            if let Some(TimeOrBlock::Block(block)) = processor.options.end {
                contract_config.end_block = block;
            }
            contract_config.contract = Some(ContractInfo {
                address: processor.options.address.clone(),
                name: processor.name().to_string(),
                abi: "".to_owned(),
                chain_id: processor_chain_id.to_string(),
            });

            debug!(
                "Registering handlers for processor '{}' (chain_id: {})",
                processor.name(),
                processor_chain_id
            );

            for (handle_idx, handler) in processor.event_handlers.iter().enumerate() {
                let handler_id = self.handler_register.register(
                    processor_chain_id,
                    HandlerType::EthLog,
                    processor_idx,
                    handle_idx,
                );
                let mut log_config = LogHandlerConfig {
                    handler_id,
                    filters: vec![],
                    fetch_config: handler.fetch_config(),
                    handler_name: handler.name.clone().unwrap_or("".to_string()),
                };

                if handler.filters.len() == 0 {
                    // add empty filter for all events
                    log_config.filters.push(LogFilter::default());
                } else {
                    for filter in handler.filters.iter() {
                        let mut log_filter = LogFilter::default();
                        if let Some(contract) = &contract_config.contract {
                            let mut address = &contract.address;
                            if let Some(addr) = &filter.address {
                                address = addr
                            }
                            log_filter.address_or_type =
                                Some(AddressOrType::Address(address.clone()));
                        }
                        log_filter.topics.push(Topic {
                            hashes: filter.topics.clone(),
                        });
                        log_config.filters.push(log_filter);
                    }
                }
                contract_config.log_configs.push(log_config);
            }

            config.contract_configs.push(contract_config);
        }
    }

    fn can_handle_type(&self, handler_type: HandlerType) -> bool {
        matches!(
            handler_type,
            HandlerType::EthLog
                | HandlerType::EthBlock
                | HandlerType::EthTrace
                | HandlerType::EthTransaction
        )
    }
}

impl EthPlugin {
    fn find_handler(
        &self,
        chain_id: &str,
        handler_id: i32,
    ) -> anyhow::Result<(&EthProcessorImpl, &crate::eth::eth_processor::EventHandler)> {
        // Look up the handler information
        let handler_info = self
            .handler_register
            .get_info(chain_id, handler_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Handler {} not found for chain {}", handler_id, chain_id)
            })?;

        let processor_idx = handler_info.processor_idx;
        let handler_idx = handler_info.handler_idx;

        debug!(
            "Found handler - processor_idx: {}, handler_idx: {}",
            processor_idx, handler_idx
        );

        // Get the processor and event handler
        let processor = self
            .processors
            .get(processor_idx)
            .ok_or_else(|| anyhow::anyhow!("Processor index {} not found", processor_idx))?;

        let event_handler = processor.event_handlers.get(handler_idx).ok_or_else(|| {
            anyhow::anyhow!(
                "Event handler index {} not found in processor {}",
                handler_idx,
                processor_idx
            )
        })?;

        Ok((processor.as_ref(), event_handler))
    }

    async fn process_eth_log(
        &self,
        data: &crate::DataBinding,
    ) -> anyhow::Result<crate::ProcessResult> {
        debug!("Processing ETH log for chain_id: {}", data.chain_id);

        // Extract ETH log data
        let eth_log_data = match &data.data {
            Some(d) => match &d.value {
                Some(crate::processor::data::Value::EthLog(log_data)) => log_data,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Expected ETH log data but got different type"
                    ))
                }
            },
            None => return Err(anyhow::anyhow!("No data provided in DataBinding")),
        };

        // Parse all Ethereum data using ethers library
        let parsed_data = ParsedEthData::from(eth_log_data);

        let mut result = crate::ProcessResult::default();
        // Process each handler_id for ETH log
        for &handler_id in &data.handler_ids {
            debug!(
                "Processing ETH log handler_id: {} for chain: {}",
                handler_id, data.chain_id
            );

            let (processor, event_handler) = self.find_handler(&data.chain_id, handler_id)?;

            debug!(
                "Calling ETH log handler for processor: {}",
                processor.name()
            );

            // Check if we have a parsed log to work with
            if let Some(ref log) = parsed_data.log {
                let event = EthEvent {
                    log: log.clone(),
                    decoded_log: None,
                };

                // TODO: Implement log decoding if needed
                if event_handler.need_decode_log() {
                    // todo decode log
                }

                // Create state collector for this handler execution
                let (state_collector, state_receiver) = StateCollector::new();
                let mut update_collector = StateUpdateCollector::new(state_receiver);

                // Create context with state collector
                let context =
                    crate::eth::context::EthContext::with_state_collector(state_collector);
                let metadata = MetaData::default();
                let runtime_ctx = RUNTIME_CONTEXT.get();

                // Execute the user handler with owned context using trait method
                RUNTIME_CONTEXT
                    .scope(
                        runtime_ctx.with_metadata(metadata),
                        event_handler.handler.handle_event(event, context),
                    )
                    .await;

                // Collect state updates that occurred during handler execution
                let state_result = update_collector.collect_updates();
                result = result.merge(state_result)
            } else {
                debug!("No log found for processor: {}", processor.name());
            }
        }

        Ok(result)
    }
}

impl crate::ProcessResult {
    fn merge(mut self, other: crate::ProcessResult) -> Self {
        // Extend vectors with other's values
        self.gauges.extend(other.gauges);
        self.counters.extend(other.counters);
        #[allow(deprecated)]
        self.logs.extend(other.logs);
        self.events.extend(other.events);
        self.exports.extend(other.exports);
        self.timeseries_result.extend(other.timeseries_result);

        // Merge states - combine config_updated flags and errors
        match (self.states.as_mut(), other.states) {
            (Some(self_state), Some(other_state)) => {
                // If either has config_updated = true, result should be true
                self_state.config_updated = self_state.config_updated || other_state.config_updated;

                // Combine errors - if both have errors, concatenate them
                match (&self_state.error, other_state.error) {
                    (Some(self_error), Some(other_error)) => {
                        self_state.error = Some(format!("{}; {}", self_error, other_error));
                    }
                    (None, Some(other_error)) => {
                        self_state.error = Some(other_error);
                    }
                    // If self has error and other doesn't, keep self's error
                    // If neither has error, keep None
                    _ => {}
                }
            }
            (None, Some(other_state)) => {
                // If self has no state but other does, use other's state
                self.states = Some(other_state);
            }
            // If other has no state, keep self's state (or None)
            _ => {}
        }

        self
    }
}

#[tonic::async_trait]
impl AsyncPluginProcessor for EthPlugin {
    async fn process_binding(
        &self,
        data: &crate::DataBinding,
    ) -> anyhow::Result<crate::ProcessResult> {
        debug!(
            "EthPlugin processing binding for chain_id: {}, handler_ids: {:?}",
            data.chain_id, data.handler_ids
        );

        // Dispatch by handler type
        let handler_type = crate::processor::HandlerType::try_from(data.handler_type)?;

        match handler_type {
            HandlerType::EthLog => self.process_eth_log(data).await,
            HandlerType::EthBlock => {
                debug!("ETH block processing not implemented yet");
                Ok(crate::ProcessResult::default())
            }
            HandlerType::EthTrace => {
                debug!("ETH trace processing not implemented yet");
                Ok(crate::ProcessResult::default())
            }
            HandlerType::EthTransaction => {
                debug!("ETH transaction processing not implemented yet");
                Ok(crate::ProcessResult::default())
            }
            _ => Err(anyhow::anyhow!(
                "Unsupported handler type: {:?}",
                handler_type
            )),
        }
    }
}

// Implement the combined FullPlugin trait
impl FullPlugin for EthPlugin {}

impl PluginRegister<EthProcessorImpl> for EthPlugin {
    fn register_processor(&mut self, processor: EthProcessorImpl) -> &mut EthProcessorImpl {
        debug!(
            "Registering processor: {} (chain_id: {})",
            processor.name(),
            processor.chain_id()
        );

        self.processors.push(Box::new(processor));

        // Return a reference to the last processor
        self.processors.last_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eth::eth_processor::EthProcessor;
    use crate::{ConfigureHandlersRequest, ConfigureHandlersResponse};
    use crate::eth::EthEventHandler;

    #[derive(Clone)]
    struct TestProcessor {
        address: String,
        chain_id: String,
        name: String,
    }

    impl TestProcessor {
        fn new() -> Self {
            Self {
                address: "0x1234567890123456789012345678901234567890".to_string(),
                chain_id: "ethereum".to_string(),
                name: "test-processor".to_string(),
            }
        }
    }

    impl EthProcessor for TestProcessor {
        fn address(&self) -> &str {
            &self.address
        }
        
        fn chain_id(&self) -> &str {
            &self.chain_id
        }
        
        fn name(&self) -> &str {
            &self.name
        }
    }

    struct TestEventMarker;
    
    impl crate::eth::EventMarker for TestEventMarker {
        fn filter() -> Vec<crate::eth::eth_processor::EventFilter> {
            vec![]
        }
    }

    #[crate::async_trait]
    impl EthEventHandler<TestEventMarker> for TestProcessor {
        async fn on_event(&self, _event: EthEvent, _ctx: crate::eth::context::EthContext) {
            // Test event handler
        }
    }

    #[test]
    fn test_configure_method() {
        let mut plugin = EthPlugin::default();
        let mut config = ConfigureHandlersResponse::default();

        // Create a test processor using the new trait-based API
        let processor_impl = TestProcessor::new()
            .configure_event::<TestEventMarker>(None);

        // We need to manually create the EthProcessorImpl for the test
        use std::sync::Arc;
        let processor_arc = Arc::new(TestProcessor::new());
        let mut processor_impl = crate::eth::eth_processor::EthProcessorImpl::new(processor_arc.clone());
        
        // Add the event handler manually for the test
        processor_impl.add_event_handler(TestProcessor::new(), None);

        // Register the processor
        plugin.register_processor(processor_impl);

        // Test configure method with no chain filter
        let request = ConfigureHandlersRequest {
            chain_id: "".to_string(),
            template_instances: vec![],
        };
        plugin.configure(&request, &mut config);

        // Should have registered handlers for each handler type
        let registered_count = plugin.handler_register.len();
        assert!(registered_count > 0, "Should have registered some handlers");

        // Clear and test configure method with chain filter
        plugin.handler_register.clear();
        let request_with_chain = ConfigureHandlersRequest {
            chain_id: "ethereum".to_string(),
            template_instances: vec![],
        };
        plugin.configure(&request_with_chain, &mut config);

        // Should have same number of handlers when filtering by correct chain
        assert_eq!(plugin.handler_register.len(), registered_count);

        // Clear and test configure method with non-matching chain filter
        plugin.handler_register.clear();
        let request_non_matching = ConfigureHandlersRequest {
            chain_id: "999".to_string(),
            template_instances: vec![],
        };
        plugin.configure(&request_non_matching, &mut config);

        // Should have no handlers when filtering by non-matching chain
        assert_eq!(plugin.handler_register.len(), 0);
    }
}
