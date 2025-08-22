use crate::core::{Context, BaseContext, StateCollector};

#[derive(Clone)]
pub struct EthContext {
    base_context: BaseContext,
    state_collector: Option<StateCollector>,
}

impl EthContext {
    /// Create a new EthContext with the default configuration (no state collection)
    pub fn new() -> Self {
        let base_context = BaseContext::new();
        
        Self { 
            base_context,
            state_collector: None,
        }
    }
    
    /// Create a new EthContext with state collection enabled
    pub fn with_state_collector(state_collector: StateCollector) -> Self {
        let base_context = BaseContext::new();
        
        Self {
            base_context,
            state_collector: Some(state_collector),
        }
    }
}

impl Default for EthContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Context for EthContext {
    fn base_context(&mut self) -> &mut BaseContext {
        &mut self.base_context
    }
    
    /// Override to provide state collector access
    fn state_collector(&self) -> Option<&StateCollector> {
        self.state_collector.as_ref()
    }
}
