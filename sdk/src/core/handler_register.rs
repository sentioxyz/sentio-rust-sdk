use std::collections::HashMap;
use std::hash::Hash;

/// Information about a registered handler
#[derive(Debug, Clone)]
pub struct HandlerInfo<T> {
    pub chain_id: String,
    pub handler_type: T,
    pub processor_idx: usize,
    pub handler_idx: usize,
    pub handle_id: i32,
}

/// Registry for managing handler IDs and dispatching
pub struct HandlerRegister<T> {
    /// Map from chain_id to list of handlers
    handlers: HashMap<String, Vec<HandlerInfo<T>>>,
}

impl<T> HandlerRegister<T> {
    /// Create a new HandlerRegister
    pub fn new() -> Self {
        Self {
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

    /// Register a new handler and return its unique ID (index in the vector)
    pub fn register(&mut self, chain_id: &str, handler_type: T, processor_idx: usize, handler_idx: usize) -> i32 {
        let chain_handlers = self.handlers.entry(chain_id.to_string()).or_insert_with(Vec::new);
        let handle_id = chain_handlers.len() as i32;
        
        let handler_info = HandlerInfo {
            chain_id: chain_id.to_string(),
            handler_type,
            processor_idx,
            handler_idx,
            handle_id,
        };
        
        chain_handlers.push(handler_info);
        
        handle_id
    }

    /// Get handler information by chain_id and handler ID
    pub fn get(&self, chain_id: &str, handler_id: i32) -> Option<(T, usize, usize)> {
        self.handlers
            .get(chain_id)?
            .get(handler_id as usize)
            .map(|info| (info.handler_type.clone(), info.processor_idx, info.handler_idx))
    }

    /// Get full handler information by chain_id and handler ID
    pub fn get_info(&self, chain_id: &str, handler_id: i32) -> Option<&HandlerInfo<T>> {
        self.handlers
            .get(chain_id)?
            .get(handler_id as usize)
    }

    /// Get all handlers for a specific chain ID
    pub fn get_handlers_for_chain(&self, chain_id: &str) -> Vec<(i32, &HandlerInfo<T>)> {
        self.handlers
            .get(chain_id)
            .map(|chain_handlers| {
                chain_handlers
                    .iter()
                    .enumerate()
                    .map(|(idx, info)| (idx as i32, info))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all handlers of a specific type
    pub fn get_handlers_by_type(&self, handler_type: &T) -> Vec<(String, i32, &HandlerInfo<T>)> {
        self.handlers
            .iter()
            .flat_map(|(chain_id, chain_handlers)| {
                chain_handlers
                    .iter()
                    .enumerate()
                    .filter(|(_, info)| &info.handler_type == handler_type)
                    .map(|(idx, info)| (chain_id.clone(), idx as i32, info))
            })
            .collect()
    }

    /// Get total number of registered handlers
    pub fn len(&self) -> usize {
        self.handlers.values().map(|v| v.len()).sum()
    }

    /// Check if the register is empty
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty() || self.handlers.values().all(|v| v.is_empty())
    }

    /// Clear all registered handlers
    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    /// Clear all registered handlers for a specific chain
    pub fn clear_chain(&mut self, chain_id: &str) {
        self.handlers.remove(chain_id);
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
        
        let handler_id = register.register("1", TestHandlerType::Event, 0, 42);
        // Handle ID should be the index in the vector, starting from 0
        assert_eq!(handler_id, 0);
        
        let (handler_type, processor_idx, handler_idx) = register.get("1", handler_id).unwrap();
        assert_eq!(handler_type, TestHandlerType::Event);
        assert_eq!(processor_idx, 0);
        assert_eq!(handler_idx, 42);
    }

    #[test]
    fn test_unique_handler_ids() {
        let mut register = HandlerRegister::default();
        
        let id1 = register.register("1", TestHandlerType::Event, 0, 1);
        let id2 = register.register("1", TestHandlerType::Event, 0, 2);
        let id3 = register.register("2", TestHandlerType::Call, 1, 1);
        
        // Within same chain, IDs should be sequential from 0
        assert_eq!(id1, 0);  // First handler for chain "1"
        assert_eq!(id2, 1);  // Second handler for chain "1"
        
        // Different chain starts from 0 again
        assert_eq!(id3, 0);  // First handler for chain "2"
        
        // Verify we can get them back
        assert!(register.get("1", id1).is_some());
        assert!(register.get("1", id2).is_some());
        assert!(register.get("2", id3).is_some());
        
        // Cross-chain access with non-existent indices should fail
        assert!(register.get("2", id2).is_none());  // Chain "2" only has 1 handler (index 0), so index 1 should fail
        assert!(register.get("1", 5).is_none());    // Chain "1" only has 2 handlers (indices 0,1), so index 5 should fail
    }

    #[test]
    fn test_get_handlers_for_chain() {
        let mut register = HandlerRegister::default();
        
        let _id1 = register.register("1", TestHandlerType::Event, 0, 1);
        let _id2 = register.register("1", TestHandlerType::Call, 1, 2);
        let _id3 = register.register("2", TestHandlerType::Event, 0, 3);
        
        let chain1_handlers = register.get_handlers_for_chain("1");
        assert_eq!(chain1_handlers.len(), 2);
        
        let chain2_handlers = register.get_handlers_for_chain("2");
        assert_eq!(chain2_handlers.len(), 1);
    }

    #[test]
    fn test_get_handlers_by_type() {
        let mut register = HandlerRegister::default();
        
        let _id1 = register.register("1", TestHandlerType::Event, 0, 1);
        let _id2 = register.register("1", TestHandlerType::Event, 1, 2);
        let _id3 = register.register("2", TestHandlerType::Call, 0, 3);
        
        let event_handlers = register.get_handlers_by_type(&TestHandlerType::Event);
        assert_eq!(event_handlers.len(), 2);
        // Check that we get chain_id, handler_id, and handler_info
        assert_eq!(event_handlers[0].0, "1");  // chain_id
        assert_eq!(event_handlers[0].1, 0);    // handler_id (index)
        assert_eq!(event_handlers[1].0, "1");  // chain_id
        assert_eq!(event_handlers[1].1, 1);    // handler_id (index)
        
        let call_handlers = register.get_handlers_by_type(&TestHandlerType::Call);
        assert_eq!(call_handlers.len(), 1);
        assert_eq!(call_handlers[0].0, "2");   // chain_id
        assert_eq!(call_handlers[0].1, 0);     // handler_id (index)
    }

    #[test]
    fn test_chain_id_handling() {
        let mut register = HandlerRegister::default();
        
        // Test different chain IDs - each chain starts from index 0
        let id1 = register.register("1", TestHandlerType::Event, 0, 0);
        let id2 = register.register("42", TestHandlerType::Event, 0, 0);
        let id3 = register.register("ethereum", TestHandlerType::Event, 0, 0);
        let id4 = register.register("polygon", TestHandlerType::Event, 0, 0);
        
        // Each chain starts with handler_id 0
        assert_eq!(id1, 0);
        assert_eq!(id2, 0);
        assert_eq!(id3, 0);
        assert_eq!(id4, 0);
        
        // Same chain should get incremented IDs
        let id5 = register.register("ethereum", TestHandlerType::Event, 0, 1);
        assert_eq!(id5, 1);
        
        // Verify handlers exist at expected chain/id combinations
        assert!(register.get("1", 0).is_some());
        assert!(register.get("42", 0).is_some());
        assert!(register.get("ethereum", 0).is_some());
        assert!(register.get("ethereum", 1).is_some());
        assert!(register.get("polygon", 0).is_some());
    }

    #[test]
    fn test_clear_chain() {
        let mut register = HandlerRegister::default();
        
        let _id1 = register.register("1", TestHandlerType::Event, 0, 1);
        let _id2 = register.register("1", TestHandlerType::Call, 1, 2);
        let _id3 = register.register("2", TestHandlerType::Event, 0, 3);
        
        assert_eq!(register.len(), 3);
        
        // Clear chain "1"
        register.clear_chain("1");
        
        assert_eq!(register.len(), 1);
        let remaining = register.get_handlers_for_chain("2");
        assert_eq!(remaining.len(), 1);
        
        // Verify chain "1" handlers are gone
        let chain1_handlers = register.get_handlers_for_chain("1");
        assert_eq!(chain1_handlers.len(), 0);
    }

    #[test]
    fn test_sequence_generation() {
        let mut register = HandlerRegister::default();
        
        // Register multiple handlers for the same chain
        let id1 = register.register("1", TestHandlerType::Event, 0, 0);
        let id2 = register.register("1", TestHandlerType::Event, 0, 1);
        let id3 = register.register("1", TestHandlerType::Event, 0, 2);
        
        // All should be sequential starting from 0
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[test]
    fn test_large_handler_count() {
        let mut register = HandlerRegister::default();
        
        // Test that we can handle many handlers in a single chain
        let chain_id = "test";
        let mut handler_ids = Vec::new();
        
        for i in 0..1000 {
            let handler_id = register.register(chain_id, TestHandlerType::Event, 0, i);
            handler_ids.push(handler_id);
            assert_eq!(handler_id, i as i32);
        }
        
        // Verify all handlers can be retrieved
        for (i, &handler_id) in handler_ids.iter().enumerate() {
            let result = register.get(chain_id, handler_id);
            assert!(result.is_some());
            let (handler_type, processor_idx, handler_idx) = result.unwrap();
            assert_eq!(handler_type, TestHandlerType::Event);
            assert_eq!(processor_idx, 0);
            assert_eq!(handler_idx, i);
        }
    }
}