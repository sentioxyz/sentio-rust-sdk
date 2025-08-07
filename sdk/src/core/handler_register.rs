use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::hash::Hash;

/// Information about a registered handler
#[derive(Debug, Clone)]
pub struct HandlerInfo<T> {
    pub chain_id: u64,
    pub handler_type: T,
    pub handler_idx: u64,
}

/// Registry for managing handler IDs and dispatching
pub struct HandlerRegister<T> {
    /// Counter for generating unique handler IDs
    next_handler_id: AtomicU64,
    /// Map from handler ID to handler information
    handlers: HashMap<u64, HandlerInfo<T>>,
}

impl<T> HandlerRegister<T> {
    /// Create a new HandlerRegister with handler IDs starting from 1
    pub fn new() -> Self {
        Self {
            next_handler_id: AtomicU64::new(1),
            handlers: HashMap::new(),
        }
    }
}

impl<T> Default for HandlerRegister<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HandlerRegister<T>
where
    T: Clone + PartialEq + Hash,
{

    /// Register a new handler and return its unique ID
    pub fn register(&mut self, chain_id: u64, handler_type: T, handler_idx: u64) -> u64 {
        let handler_id = self.next_handler_id.fetch_add(1, Ordering::SeqCst);
        
        let handler_info = HandlerInfo {
            chain_id,
            handler_type,
            handler_idx,
        };
        
        self.handlers.insert(handler_id, handler_info);
        
        handler_id
    }

    /// Get handler information by handler ID
    pub fn get(&self, handler_id: u64) -> Option<(T, u64)> {
        self.handlers.get(&handler_id).map(|info| (info.handler_type.clone(), info.handler_idx))
    }

    /// Get full handler information by handler ID
    pub fn get_info(&self, handler_id: u64) -> Option<&HandlerInfo<T>> {
        self.handlers.get(&handler_id)
    }

    /// Get all handlers for a specific chain ID
    pub fn get_handlers_for_chain(&self, chain_id: u64) -> Vec<(u64, &HandlerInfo<T>)> {
        self.handlers
            .iter()
            .filter(|(_, info)| info.chain_id == chain_id)
            .map(|(id, info)| (*id, info))
            .collect()
    }

    /// Get all handlers of a specific type
    pub fn get_handlers_by_type(&self, handler_type: &T) -> Vec<(u64, &HandlerInfo<T>)> {
        self.handlers
            .iter()
            .filter(|(_, info)| &info.handler_type == handler_type)
            .map(|(id, info)| (*id, info))
            .collect()
    }

    /// Get total number of registered handlers
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if the register is empty
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// Clear all registered handlers
    pub fn clear(&mut self) {
        self.handlers.clear();
        self.next_handler_id.store(1, Ordering::SeqCst);
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    enum TestHandlerType {
        #[default]
        Event,
        Call,
        Block,
        Transaction,
    }

    #[test]
    fn test_register_and_get() {
        let mut register = HandlerRegister::default();
        
        let handler_id = register.register(1, TestHandlerType::Event, 42);
        assert_eq!(handler_id, 1);
        
        let (handler_type, handler_idx) = register.get(handler_id).unwrap();
        assert_eq!(handler_type, TestHandlerType::Event);
        assert_eq!(handler_idx, 42);
    }

    #[test]
    fn test_unique_handler_ids() {
        let mut register = HandlerRegister::default();
        
        let id1 = register.register(1, TestHandlerType::Event, 1);
        let id2 = register.register(1, TestHandlerType::Event, 2);
        let id3 = register.register(2, TestHandlerType::Call, 1);
        
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_get_handlers_for_chain() {
        let mut register = HandlerRegister::default();
        
        let _id1 = register.register(1, TestHandlerType::Event, 1);
        let _id2 = register.register(1, TestHandlerType::Call, 2);
        let _id3 = register.register(2, TestHandlerType::Event, 3);
        
        let chain1_handlers = register.get_handlers_for_chain(1);
        assert_eq!(chain1_handlers.len(), 2);
        
        let chain2_handlers = register.get_handlers_for_chain(2);
        assert_eq!(chain2_handlers.len(), 1);
    }

    #[test]
    fn test_get_handlers_by_type() {
        let mut register = HandlerRegister::default();
        
        let _id1 = register.register(1, TestHandlerType::Event, 1);
        let _id2 = register.register(1, TestHandlerType::Event, 2);
        let _id3 = register.register(2, TestHandlerType::Call, 3);
        
        let event_handlers = register.get_handlers_by_type(&TestHandlerType::Event);
        assert_eq!(event_handlers.len(), 2);
        
        let call_handlers = register.get_handlers_by_type(&TestHandlerType::Call);
        assert_eq!(call_handlers.len(), 1);
    }
}