// In-memory database implementation for testing
use crate::db_request::DbFilter;
use crate::entity::store::backend::StorageBackend;
use crate::entity::store::store::StoreImpl;
use crate::common::RichStruct;
use crate::{db_response, processor::Entity};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use dashmap::DashMap;

#[derive(Debug, Clone)]
pub struct MemoryDatabase {
    // table_name -> entity_id -> entity
    data: DashMap<String, DashMap<String, Entity>>,
}

impl MemoryDatabase {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }
    
    pub async fn clear(&mut self) {
        self.data.clear();
    }
    
    pub async fn get_table_count(&self, table: &str) -> usize {
        self.data
            .get(table)
            .map(|table_data| table_data.len())
            .unwrap_or(0)
    }
    
    /// Get all entities in a table (for testing purposes)
    pub async fn list_table_entities(&self, table: &str) -> Vec<Entity> {
        self.data
            .get(table)
            .map(|table_data| table_data.iter().map(|entry| entry.value().clone()).collect())
            .unwrap_or_else(Vec::new)
    }
    
    /// Get a specific entity by ID (for testing purposes)  
    pub async fn get_entity(&self, table: &str, id: &str) -> Option<Entity> {
        self.data
            .get(table)
            .and_then(|table_data| table_data.get(id).map(|entry| entry.value().clone()))
    }
    
    /// Check if entity exists (for testing purposes)
    pub async fn entity_exists(&self, table: &str, id: &str) -> bool {
        self.data
            .get(table)
            .map(|table_data| table_data.contains_key(id))
            .unwrap_or(false)
    }
    
    /// Get all table names that contain entities (for testing purposes)
    pub async fn get_table_names(&self) -> Vec<String> {
        self.data
            .iter()
            .filter_map(|entry| {
                let table_name = entry.key().clone();
                let table_data = entry.value();
                if !table_data.is_empty() {
                    Some(table_name)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for MemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryDatabase {
    async fn get(&self, table: &str, id: &str) -> Result<Option<db_response::Value>> {
        if let Some(table_data) = self.data.get(table) {
            if let Some(entity) = table_data.get(id) {
                let entity_list = crate::processor::EntityList {
                    entities: vec![entity.value().clone()],
                };
                return Ok(Some(db_response::Value::EntityList(entity_list)));
            }
        }
        
        // Return empty list instead of None to match expected behavior
        Ok(Some(db_response::Value::EntityList(
            crate::processor::EntityList {
                entities: vec![],
            }
        )))
    }

    async fn delete(&self, tables: Vec<String>, ids: Vec<String>) -> Result<()> {
        for (table, id) in tables.into_iter().zip(ids.into_iter()) {
            if let Some(table_data) = self.data.get(&table) {
                table_data.remove(&id);
            }
        }
        
        Ok(())
    }

    async fn list(
        &self,
        table: &str,
        _filters: Vec<DbFilter>,
        _cursor: String,
        page_size: Option<u32>,
    ) -> Result<Option<db_response::Value>> {
        if let Some(table_data) = self.data.get(table) {
            let mut entities: Vec<Entity> = table_data.iter().map(|entry| entry.value().clone()).collect();
            
            // Sort by entity ID for consistent ordering
            entities.sort_by(|a, b| a.entity.cmp(&b.entity));
            
            // Apply page size limit
            if let Some(limit) = page_size {
                entities.truncate(limit as usize);
            }
            
            let entity_list = crate::processor::EntityList { entities };
            return Ok(Some(db_response::Value::EntityList(entity_list)));
        }
        
        // Return empty list for non-existent tables
        Ok(Some(db_response::Value::EntityList(
            crate::processor::EntityList {
                entities: vec![],
            }
        )))
    }

    async fn upsert(&self, tables: Vec<String>, ids: Vec<String>, entity_data: Vec<RichStruct>) -> Result<()> {
        for ((table, id), rich_struct) in tables.into_iter().zip(ids.into_iter()).zip(entity_data.into_iter()) {
            // Create the entity from the RichStruct
            let entity = Entity {
                entity: id.clone(),
                gen_block_number: 0, // TODO: could be configurable for testing
                gen_block_chain: "test".to_string(),
                gen_block_time: None,
                data: Some(rich_struct),
            };
            
            // Insert into the appropriate table
            let table_map = self.data.entry(table).or_insert_with(DashMap::new);
            table_map.insert(id, entity);
        }
        
        Ok(())
    }
}

/// Test wrapper that makes MemoryDatabase compatible with RemoteBackend usage patterns
/// This allows us to use MemoryDatabase in contexts where RemoteBackend is expected
pub struct TestBackend {
    memory_db: Arc<MemoryDatabase>,
}

impl TestBackend {
    pub fn new(memory_db: MemoryDatabase) -> Self {
        Self {
            memory_db: Arc::new(memory_db),
        }
    }
    
    pub fn from_arc(memory_db: Arc<MemoryDatabase>) -> Self {
        Self {
            memory_db,
        }
    }
}

#[async_trait]
impl StorageBackend for TestBackend {
    async fn get(&self, table: &str, id: &str) -> Result<Option<db_response::Value>> {

        self.memory_db.get(table, id).await
    }

    async fn delete(&self, table: Vec<String>, ids: Vec<String>) -> Result<()> {
        self.memory_db.delete(table, ids).await
    }

    async fn list(
        &self,
        table: &str,
        filters: Vec<DbFilter>,
        cursor: String,
        page_size: Option<u32>,
    ) -> Result<Option<db_response::Value>> {
        self.memory_db.list(table, filters, cursor, page_size).await
    }

    async fn upsert(&self, table: Vec<String>, id: Vec<String>, data: Vec<RichStruct>) -> Result<()> {
        self.memory_db.upsert(table, id, data).await
    }
}

/// Test store type alias for use in testing contexts
pub type TestStore = StoreImpl<MemoryDatabase>;