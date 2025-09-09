//! Core store implementation for entities

use crate::db_request::DbFilter;
use crate::entity::ToRichValue;
use crate::entity::store::StorageBackend;
use crate::entity::traits::{Entity, EntityId, EntityStore, Filter, ListOptions};
use crate::{RichValueList, db_response};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::sync::Arc;

/// Store implementation that uses a storage backend
pub struct StoreImpl<B: StorageBackend> {
    /// Storage backend
    backend: Arc<B>,
}

impl<B: StorageBackend> StoreImpl<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Create a new store instance with a shared backend
    pub fn from_arc(backend: Arc<B>) -> Self {
        Self { backend }
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
        let id_string = id.as_string();

        if let Some(db_value) = self.backend.get(T::NAME, &id_string).await? {
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

    async fn get_many<T: Entity>(&self, ids: &[T::Id]) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize,
    {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        if ids.len() == 1 {
            // For single ID, use the regular get method
            match self.get(&ids[0]).await? {
                Some(entity) => Ok(vec![entity]),
                None => Ok(Vec::new()),
            }
        } else {
            // For multiple IDs, use list with IN filter for optimization
            let id_strings: Vec<String> = ids.iter().map(|id| id.as_string()).collect();

            let filter = Filter::<T>::in_("id", id_strings);
            let mut options = ListOptions::new();
            options.filters.push(filter);

            self.list(options).await
        }
    }

    async fn upsert<T: Entity>(&self, entity: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let id_string = entity.id().as_string();
        let data = T::to_rich_struct(entity)?;

        self.backend
            .upsert(vec![T::NAME.to_string()], vec![id_string], vec![data])
            .await
    }

    async fn upsert_many<T: Entity>(&self, entities: &[T]) -> Result<()>
    where
        T: serde::Serialize,
    {
        if entities.is_empty() {
            return Ok(());
        }
        
        // Pre-allocate all vectors with exact capacity for better performance
        let len = entities.len();
        let tables = vec![T::NAME.to_string(); len];
        
        let mut ids = Vec::with_capacity(len);
        let mut data = Vec::with_capacity(len);
        
        // Process entities and collect results
        for entity in entities {
            ids.push(entity.id().as_string());
            data.push(T::to_rich_struct(entity)?);
        }

        self.backend.upsert(tables, ids, data).await
    }

    async fn delete<T: Entity>(&self, id: &T::Id) -> Result<()> {
         let id_string = id.as_string();

        self.backend.delete(vec![T::NAME.to_string()], vec![id_string]).await
    }

    async fn delete_many<T: Entity>(&self, ids: &[T::Id]) -> Result<()> {
         let tables = vec![T::NAME.to_string(); ids.len()];
        let ids = ids.iter().map(|id| id.as_string()).collect::<Vec<_>>();
        self.backend.delete(tables, ids).await
    }

    async fn list<T: Entity>(&self, options: ListOptions<T>) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize,
    {
         let mut filters = vec![];

        for f in options.filters {
            let value = f.value.to_rich_value()?;
            let filter = DbFilter {
                field: f.field,
                op: f.operator as i32,
                value: Some(RichValueList {
                    values: vec![value],
                }),
            };
            filters.push(filter);
        }

        let response = self
            .backend
            .list(
                T::NAME,
                filters,
                options.cursor.unwrap_or_default(),
                options.limit,
            )
            .await?;
        match response {
            Some(db_value) => {
                let entities = Self::db_value_to_entities::<T>(db_value)?;
                Ok(entities)
            }
            _ => Ok(vec![]),
        }
    }
}

/// Type alias for the store with a pluggable backend (remote in prod, memory in tests)
pub type Store = StoreImpl<crate::entity::store::backend::Backend>;

impl Store {
    /// Create a Store from the current runtime context
    pub async fn from_current_context() -> Result<Self> {
        use crate::core::context::RUNTIME_CONTEXT;

        RUNTIME_CONTEXT
            .try_with(|ctx| {
                let backend = ctx.remote_backend.clone();
                Ok(Self::from_arc(backend))
            })
            .map_err(|_| anyhow!("No runtime context available"))?
    }
}

impl<B: StorageBackend + Default> Default for StoreImpl<B> {
    fn default() -> Self {
        Self::new(B::default())
    }
}

#[cfg(test)]
mod tests {}
