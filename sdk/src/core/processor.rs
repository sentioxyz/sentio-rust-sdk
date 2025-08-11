use super::HandlerRegister;
use std::hash::Hash;
use std::any::Any;

/// Base trait that all processors must implement
pub trait BaseProcessor: Send + Sync + Any {
    /// Returns the chain ID this processor works on
    fn chain_id(&self) -> &str;
    
    /// Returns a human-readable name for this processor
    fn name(&self) -> &str {
        "unnamed"
    }

    /// Get the total number of handlers registered
    fn handler_count(&self) -> usize {
        0
    }
}

/// Extended trait for processors with typed handler registers
pub trait TypedProcessor<T>: BaseProcessor
where
    T: Clone + PartialEq + Hash,
{
    /// Returns a reference to the handler register for this processor
    fn handler_register(&self) -> &HandlerRegister<T>;

    /// Returns a mutable reference to the handler register for this processor
    fn handler_register_mut(&mut self) -> &mut HandlerRegister<T>;
}