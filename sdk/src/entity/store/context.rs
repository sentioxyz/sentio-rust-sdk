//! Store context integration with processor context

use super::store::Store;
use std::sync::Arc;

/// Store context that provides access to the entity store
#[derive(Clone)]
pub struct StoreContext {
    /// The entity store instance
    store: Arc<Store>,
}

impl StoreContext {
    /// Create a new store context
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Get access to the entity store
    pub fn store(&self) -> &Store {
        &self.store
    }
}

impl Default for StoreContext {
    fn default() -> Self {
        Self::new(Store::default())
    }
}