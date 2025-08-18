use tokio::sync::RwLock;
use crate::core::{Context, BaseContext, EventLogger, MetaData, Meter};

#[derive(Clone)]
pub struct EthContext {
    base_context: BaseContext,
}

impl EthContext {
    /// Create a new EthContext with the default configuration
    pub fn new() -> Self {
        let base_context = BaseContext::new();
        
        Self { 
            base_context,
        }
    }
    
    /// Create a new EthContext with context information
    pub fn with_context(
        address: String,
        contract_name: String,
        chain_id: String,
        block_number: u64,
        transaction_hash: String,
        transaction_index: i32,
        log_index: i32,
    ) -> Self {
        let base_context = BaseContext::with_context(
            address,
            contract_name,
            chain_id,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        );
        
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

#[tonic::async_trait]
impl Context for EthContext {
    fn event_logger(&self) -> &dyn EventLogger {
        &self.base_context.event_logger
    }
    
    fn get_metadata(&self) -> &RwLock<MetaData> {
        self.base_context.metadata.as_ref()
    }

    fn meter(&self) -> &Meter {
        &self.base_context.meter
    }
}
