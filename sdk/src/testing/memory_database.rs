// In-memory database implementation for testing
use crate::db_request::DbFilter;
use crate::entity::store::backend::StorageBackend;
use crate::entity::store::store::StoreImpl;
use crate::common::RichStruct;
use crate::{db_response, processor::Entity};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct MemoryDatabase {
    // table_name -> entity_id -> entity
    data: Arc<RwLock<HashMap<String, HashMap<String, Entity>>>>,
}

impl MemoryDatabase {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn clear(&mut self) {
        self.data.write().await.clear();
    }
    
    pub async fn get_table_count(&self, table: &str) -> usize {
        self.data
            .read()
            .await
            .get(table)
            .map(|t| t.len())
            .unwrap_or(0)
    }
}

impl Default for MemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryDatabase {
    async fn get(&mut self, table: &str, id: &str) -> Result<Option<db_response::Value>> {
        let data = self.data.read().await;
        
        if let Some(table_data) = data.get(table) {
            if let Some(entity) = table_data.get(id) {
                let entity_list = crate::processor::EntityList {
                    entities: vec![entity.clone()],
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
        let mut data = self.data.write().await;
        
        for (table, id) in tables.into_iter().zip(ids.into_iter()) {
            if let Some(table_data) = data.get_mut(&table) {
                table_data.remove(&id);
            }
        }
        
        Ok(())
    }

    async fn list(
        &mut self,
        table: &str,
        _filters: Vec<DbFilter>,
        _cursor: String,
        page_size: Option<u32>,
    ) -> Result<Option<db_response::Value>> {
        let data = self.data.read().await;
        
        if let Some(table_data) = data.get(table) {
            let mut entities: Vec<Entity> = table_data.values().cloned().collect();
            
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
        let mut data = self.data.write().await;
        
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
            data.entry(table)
                .or_insert_with(HashMap::new)
                .insert(id, entity);
        }
        
        Ok(())
    }
}

/// Test wrapper that makes MemoryDatabase compatible with RemoteBackend usage patterns
/// This allows us to use MemoryDatabase in contexts where RemoteBackend is expected
pub struct TestBackend {
    memory_db: Arc<RwLock<MemoryDatabase>>,
}

impl TestBackend {
    pub fn new(memory_db: MemoryDatabase) -> Self {
        Self {
            memory_db: Arc::new(RwLock::new(memory_db)),
        }
    }
    
    pub fn from_arc(memory_db: Arc<RwLock<MemoryDatabase>>) -> Self {
        Self {
            memory_db,
        }
    }
}

#[async_trait]
impl StorageBackend for TestBackend {
    async fn get(&mut self, table: &str, id: &str) -> Result<Option<db_response::Value>> {
        let mut db = self.memory_db.write().await;
        db.get(table, id).await
    }

    async fn delete(&self, table: Vec<String>, ids: Vec<String>) -> Result<()> {
        let db = self.memory_db.read().await;
        db.delete(table, ids).await
    }

    async fn list(
        &mut self,
        table: &str,
        filters: Vec<DbFilter>,
        cursor: String,
        page_size: Option<u32>,
    ) -> Result<Option<db_response::Value>> {
        let mut db = self.memory_db.write().await;
        db.list(table, filters, cursor, page_size).await
    }

    async fn upsert(&self, table: Vec<String>, id: Vec<String>, data: Vec<RichStruct>) -> Result<()> {
        let db = self.memory_db.read().await;
        db.upsert(table, id, data).await
    }
}

/// Test store type alias for use in testing contexts
pub type TestStore = StoreImpl<MemoryDatabase>;