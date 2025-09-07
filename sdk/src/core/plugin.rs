use crate::{BaseProcessor, ConfigureHandlersResponse};
use crate::processor::HandlerType;
use std::any::Any;

/// Plugin trait that defines the available handler types for a processor
pub trait Plugin: Send + Sync + Any {
    /// Returns an array of all supported handler types for this plugin
    fn handler_types(&self) -> &'static [HandlerType];
    
    /// Get the number of registered processors
    fn processor_count(&self) -> usize;
    
    fn chain_ids(&self) -> Vec<String>;

    fn name() -> &'static str where Self: Sized;

    /// Configure handlers for all processors managed by the plugin
    /// This method registers all handlers with the plugin's handler register
    fn configure(&mut self, config: &mut ConfigureHandlersResponse);
    
    /// Process a data binding request for a specific handler type
    /// Returns whether this plugin can handle the given handler type
    fn can_handle_type(&self, handler_type: HandlerType) -> bool;
}

/// Async processing trait for plugins - separate from Plugin for dyn compatibility
#[tonic::async_trait]
pub trait AsyncPluginProcessor: Send + Sync {
    /// Process data binding for handlers managed by this plugin
    async fn process_binding(&self, data: &crate::DataBinding) -> anyhow::Result<crate::ProcessResult>;
}

/// Combined trait for plugins that support both sync and async operations
pub trait FullPlugin: Plugin + AsyncPluginProcessor {}

/// Extension trait for type-safe processor registration
pub trait PluginRegister<T: BaseProcessor + 'static> {
    /// Register a processor with this plugin
    fn register_processor(&mut self, processor: T) -> &mut T;
} 
