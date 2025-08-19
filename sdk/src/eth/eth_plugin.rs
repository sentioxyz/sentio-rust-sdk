use crate::core::{BaseProcessor, HandlerRegister, Plugin, PluginRegister, AsyncPluginProcessor};
use crate::core::plugin::FullPlugin;
use crate::eth::eth_processor::{EthProcessor, TimeOrBlock};
use crate::processor::HandlerType;
use crate::{ConfigureHandlersRequest, ConfigureHandlersResponse, ContractConfig, ContractInfo, LogFilter, LogHandlerConfig, Topic};
use tracing::debug;
use crate::log_filter::AddressOrType;

#[derive(Default)]
pub struct EthPlugin {
    handler_register: HandlerRegister<HandlerType>,
    processors: Vec<Box<EthProcessor>>,
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
        self.processors.iter().map(|p| p.chain_id().to_string()).collect()
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "eth-plugin"
    }

    fn configure(&mut self, request: &ConfigureHandlersRequest, config: &mut ConfigureHandlersResponse) {
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
                    handler_name: handler.name.clone().unwrap_or("".to_string())
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
                            log_filter.address_or_type = Some(AddressOrType::Address(address.clone()));
                        }
                        log_filter.topics.push(Topic { hashes: filter.topics.clone() });
                        log_config.filters.push(log_filter);
                    }
                }
                contract_config.log_configs.push(log_config);
            }

            config.contract_configs.push(contract_config);
        }
    }

    fn can_handle_type(&self, handler_type: HandlerType) -> bool {
        matches!(handler_type, 
            HandlerType::EthLog | 
            HandlerType::EthBlock | 
            HandlerType::EthTrace | 
            HandlerType::EthTransaction
        )
    }
}

impl EthPlugin {
    fn find_handler(&self, chain_id: &str, handler_id: i32) -> anyhow::Result<(&EthProcessor, &crate::eth::eth_processor::EventHandler)> {
        // Look up the handler information
        let handler_info = self.handler_register.get_info(chain_id, handler_id)
            .ok_or_else(|| anyhow::anyhow!("Handler {} not found for chain {}", handler_id, chain_id))?;

        let processor_idx = handler_info.processor_idx;
        let handler_idx = handler_info.handler_idx;

        debug!("Found handler - processor_idx: {}, handler_idx: {}", processor_idx, handler_idx);

        // Get the processor and event handler
        let processor = self.processors.get(processor_idx)
            .ok_or_else(|| anyhow::anyhow!("Processor index {} not found", processor_idx))?;

        let event_handler = processor.event_handlers.get(handler_idx)
            .ok_or_else(|| anyhow::anyhow!("Event handler index {} not found in processor {}", handler_idx, processor_idx))?;

        Ok((processor.as_ref(), event_handler))
    }

    async fn process_eth_log(&self, data: &crate::DataBinding) -> anyhow::Result<crate::ProcessResult> {
        debug!("Processing ETH log for chain_id: {}", data.chain_id);
        
        // Extract ETH log data
        let eth_log_data = match &data.data {
            Some(d) => match &d.value {
                Some(crate::processor::data::Value::EthLog(log_data)) => log_data,
                _ => return Err(anyhow::anyhow!("Expected ETH log data but got different type")),
            },
            None => return Err(anyhow::anyhow!("No data provided in DataBinding")),
        };
        
        // Process each handler_id for ETH log
        for &handler_id in &data.handler_ids {
            debug!("Processing ETH log handler_id: {} for chain: {}", handler_id, data.chain_id);
            
            let (processor, event_handler) = self.find_handler(&data.chain_id, handler_id)?;
            
            debug!("Calling ETH log handler for processor: {}", processor.name());

            // Create RawEvent from eth_log_data
            // For now, use placeholder values until we can properly parse eth_log_data.raw_log
            let raw_event = crate::eth::eth_processor::RawEvent {
                address: "0x".to_string(), // TODO: Extract from raw_log JSON
                data: "0x".to_string(),    // TODO: Extract from raw_log JSON
                topics: vec![],            // TODO: Extract from raw_log JSON
            };
            
            // Create context with event logger
            let context = crate::eth::context::EthContext::new();
            
            // Call the event handler
            (event_handler.handler)(raw_event, context).await;
        }
        
        Ok(crate::ProcessResult::default())
    }
}

#[tonic::async_trait]
impl AsyncPluginProcessor for EthPlugin {
    async fn process_binding(&self, data: &crate::DataBinding) -> anyhow::Result<crate::ProcessResult> {
        debug!("EthPlugin processing binding for chain_id: {}, handler_ids: {:?}", data.chain_id, data.handler_ids);
        
        // Dispatch by handler type
        let handler_type = crate::processor::HandlerType::try_from(data.handler_type)?;
        
        match handler_type {
            HandlerType::EthLog => self.process_eth_log(data).await,
            HandlerType::EthBlock => {
                debug!("ETH block processing not implemented yet");
                Ok(crate::ProcessResult::default())
            },
            HandlerType::EthTrace => {
                debug!("ETH trace processing not implemented yet");
                Ok(crate::ProcessResult::default())
            },
            HandlerType::EthTransaction => {
                debug!("ETH transaction processing not implemented yet");
                Ok(crate::ProcessResult::default())
            },
            _ => Err(anyhow::anyhow!("Unsupported handler type: {:?}", handler_type))
        }
    }

}

// Implement the combined FullPlugin trait
impl FullPlugin for EthPlugin {}

impl PluginRegister<EthProcessor> for EthPlugin {
    fn register_processor(&mut self, processor: EthProcessor) -> &mut EthProcessor {
        debug!(
            "Registering processor: {} (chain_id: {})",
            processor.name(),
            processor.chain_id()
        );

        self.processors.push(Box::new(processor));

        // Get the last element and downcast it back to the concrete type
        // This is safe because we just pushed the processor
        let last_processor = self.processors.last_mut().unwrap();

        // Use Any trait to downcast back to a concrete type
        use std::any::Any;
        let any_ref = last_processor.as_mut() as &mut dyn Any;
        any_ref.downcast_mut::<EthProcessor>().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eth::eth_processor::{EthBindOptions, EthOnEvent};
    use crate::{ConfigureHandlersRequest, ConfigureHandlersResponse};

    #[test]
    fn test_configure_method() {
        let mut plugin = EthPlugin::default();
        let mut config = ConfigureHandlersResponse::default();

        // Create a test processor
        let options = EthBindOptions::new("0x1234567890123456789012345678901234567890")
            .with_network("ethereum")
            .with_name("test-processor");

        let mut processor = EthProcessor::new();
        processor.options = options;
        
        let processor = processor.on_event(|_event, _ctx| async {
            // Test event handler
        }, vec![], None);
        
        // Register the processor
        plugin.register_processor(processor);

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
