use crate::BaseProcessor;
use crate::processor::HandlerType;
use std::any::Any;

/// Plugin trait that defines the available handler types for a processor
pub trait Plugin: Send + Sync + Any {
    /// Returns an array of all supported handler types for this plugin
    fn handler_types(&self) -> &'static [HandlerType];

    /// Returns the plugin name/identifier
    fn plugin_name(&self) -> &str {
        "unnamed-plugin"
    }
    
    /// Get the number of registered processors
    fn processor_count(&self) -> usize;
    
    /// Iterate over all registered processors
    fn iter_processors(&self) -> Box<dyn Iterator<Item = &Box<dyn BaseProcessor>> + '_>;
}

/// Extension trait for type-safe processor registration
pub trait PluginRegister<T: BaseProcessor + 'static> {
    /// Register a processor with this plugin
    fn register_processor(&mut self, processor: T) -> &mut T;
} 