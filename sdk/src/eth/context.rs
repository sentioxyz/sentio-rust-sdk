use crate::core::{Context, BaseContext};

#[derive(Clone)]
pub struct EthContext {
    base_context: BaseContext,
}

impl EthContext {
    
}

impl EthContext {
    /// Create a new EthContext with the default configuration
    pub fn new() -> Self {
        let base_context = BaseContext::new();
        
        Self { 
            base_context,
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
    
}
