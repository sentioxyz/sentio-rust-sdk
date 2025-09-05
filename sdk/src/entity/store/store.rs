//! Core store implementation for entities

use crate::db_request::DbFilter;
use crate::entity::store::StorageBackend;
use crate::entity::traits::{
    Entity, EntityId, EntityStore, ListOptions,
};
use crate::entity::ToRichValue;
use crate::{db_response, RichValueList};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Store implementation that uses a storage backend
pub struct StoreImpl<B: StorageBackend> {
    /// Storage backend
    backend: Arc<RwLock<B>>,
}

impl<B: StorageBackend> StoreImpl<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(RwLock::new(backend)),
        }
    }

    /// Create a new store instance with a shared backend
    pub fn from_arc(backend: Arc<RwLock<B>>) -> Self {
        Self { backend }
    }

    /// Get table name for an entity type
    fn get_table_name<T: Entity>() -> String {
        T::table_name().to_string()
    }

    /// Convert db_response::Value directly to entity
    fn db_value_to_entities<T: Entity>(db_response_value: db_response::Value) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        use crate::db_response::Value;
        match db_response_value {
            Value::EntityList(entity_list) => {
                entity_list
                    .entities
                    .iter()
                    .map(|entity| {
                        // Convert protobuf Entity directly to T
                        if let Some(ref data) = entity.data {
                            T::from_rich_struct(data)
                                .map_err(|e| anyhow!("Failed to convert entity: {}", e))
                        } else {
                            Err(anyhow!("Entity has no data"))
                        }
                    })
                    .collect::<Result<Vec<T>>>()
            }
            Value::Error(err) => Err(anyhow!("Database error: {}", err)),
            _ => Err(anyhow!("Unsupported db_response::Value variant")),
        }
    }
}

#[async_trait]
impl<B: StorageBackend> EntityStore for StoreImpl<B> {
    async fn get<T: Entity>(&self, id: &T::Id) -> Result<Option<T>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let mut backend = self.backend.write().await;
        let table_name = Self::get_table_name::<T>();
        let id_string = id.as_string();

        if let Some(db_value) = backend.get(&table_name, &id_string).await? {
            let entities = Self::db_value_to_entities::<T>(db_value)?;
            if let Some(entity) = entities.into_iter().next() {
                Ok(Some(entity))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn upsert<T: Entity>(&self, entity: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let backend = self.backend.read().await;
        let table_name = Self::get_table_name::<T>();
        let id_string = entity.id().as_string();
        let data = T::to_rich_struct(entity)?;

        backend
            .upsert(vec![table_name], vec![id_string], vec![data])
            .await
    }

    async fn upsert_many<T: Entity>(&self, entities: &[T]) -> Result<()>
    where
        T: serde::Serialize,
    {
        let backend = self.backend.read().await;
        let table_name = Self::get_table_name::<T>();
        let tables = vec![table_name.clone(); entities.len()];
        let ids = entities
            .iter()
            .map(|entity| entity.id().as_string())
            .collect::<Vec<_>>();
        let data = entities
            .iter()
            .map(|entity|  T::to_rich_struct(entity))
            .collect::<Result<Vec<_>>>()?;

        backend.upsert(tables, ids, data).await
    }

    async fn delete<T: Entity>(&self, id: &T::Id) -> Result<()> {
        let backend = self.backend.read().await;
        let table_name = Self::get_table_name::<T>();
        let id_string = id.as_string();

        backend.delete(vec![table_name], vec![id_string]).await
    }

    async fn delete_many<T: Entity>(&self, ids: &[T::Id]) -> Result<()> {
        let backend = self.backend.read().await;
        let table_name = Self::get_table_name::<T>();
        let tables = vec![table_name.clone(); ids.len()];
        let ids = ids.iter().map(|id| id.as_string()).collect::<Vec<_>>();
        backend.delete(tables, ids).await
    }

    async fn list<T: Entity>(&self, options: ListOptions<T>) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize,
    {
        let mut backend = self.backend.write().await;
        let table_name = Self::get_table_name::<T>();
        let mut filters = vec![];

        for f in options.filters {
            let value = f.value.to_rich_value()?;
            let filter = DbFilter {
                field: f.field,
                op: f.operator as i32,
                value: Some(RichValueList {
                    values: vec![value],
                })
            };
            filters.push(filter);
        }

        let response = backend.list(&table_name, filters, options.cursor.unwrap_or_default(), options.limit ).await?;
        match response {
            Some(db_value) => {
                let entities = Self::db_value_to_entities::<T>(db_value)?;
                Ok(entities)
            }
            _ => {
                Ok(vec![])
            }
        }
    }
}

/// Type alias for the store with RemoteBackend
pub type Store = StoreImpl<crate::entity::store::backend::RemoteBackend>;
impl<B: StorageBackend + Default> Default for StoreImpl<B> {
    fn default() -> Self {
        Self::new(B::default())
    }
}

#[cfg(test)]
mod tests {

}
