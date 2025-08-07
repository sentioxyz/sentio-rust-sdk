use tracing::debug;
use crate::processor::HandlerType;
use crate::core::{Plugin, PluginRegister, HandlerRegister, BaseProcessor};
use crate::eth::processor::EthProcessor;

#[derive(Default)]
pub struct EthPlugin {
    handler_register: HandlerRegister<HandlerType>,
    processors: Vec<Box<dyn BaseProcessor>>,
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
    
    /// Iterate over all registered processors
    pub fn iter_processors(&self) -> impl Iterator<Item = &Box<dyn BaseProcessor>> {
        self.processors.iter()
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
    
    fn iter_processors(&self) -> Box<dyn Iterator<Item = &Box<dyn BaseProcessor>> + '_> {
        Box::new(self.processors.iter())
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "eth-plugin"
    }
}

impl PluginRegister<EthProcessor> for EthPlugin {
    fn register_processor(&mut self, processor: EthProcessor) -> &mut EthProcessor {
        debug!("Registering processor: {} (chain_id: {})", processor.name(), processor.chain_id());

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

