//! Core traits for the entity framework

use crate::db_request::DbOperator;
use crate::entity::types::ID;
use crate::entity::*;
use crate::rich_value::Value;
use crate::{RichValue, RichValueList};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Core trait that all entities must implement
pub trait Entity:
    Clone + Debug + Send + Sync + 'static + Sized + serde::Serialize + for<'de> serde::Deserialize<'de>
{
    /// The type used for this entity's primary key
    type Id: EntityId;

    /// The table/collection name for this entity
    const TABLE_NAME: &'static str;

    /// Get the entity's primary key
    fn id(&self) -> &Self::Id;

    /// Get the table name (convenience method)
    fn table_name() -> &'static str {
        Self::TABLE_NAME
    }

    /// Convert entity to JSON for storage
    fn to_json(&self) -> Result<serde_json::Value>
    where
        Self: Serialize,
    {
        Ok(serde_json::to_value(self)?)
    }

    /// Create entity from JSON
    fn from_json(value: serde_json::Value) -> Result<Self>
    where
        Self: for<'de> Deserialize<'de>,
    {
        Ok(serde_json::from_value(value)?)
    }

    /// Create entity from protobuf RichStruct using direct conversion
    fn from_rich_struct(rich_struct: &crate::common::RichStruct) -> Result<Self>
    where
        Self: for<'de> Deserialize<'de>,
    {
        crate::entity::serialization::from_rich_struct(rich_struct)
    }

    /// Convert entity to protobuf RichStruct using direct conversion
    fn to_rich_struct(&self) -> Result<crate::common::RichStruct>
    where
        Self: Serialize,
    {
        crate::entity::serialization::to_rich_struct(self)
    }

    // === Enhanced Entity API Methods ===

    /// Get an entity by ID - convenience method for EntityX::get(id)
    async fn get(id: &Self::Id) -> Result<Option<Self>> {
        use crate::entity::Store;
        let store = Store::from_current_context().await?;
        store.get(id).await
    }

    /// Get multiple entities by their IDs - convenience method for EntityX::get_many(ids)
    async fn get_many(ids: &[Self::Id]) -> Result<Vec<Self>> {
        use crate::entity::Store;
        let store = Store::from_current_context().await?;
        store.get_many(ids).await
    }

    /// Save (upsert) this entity to the store
    async fn save(&self) -> Result<()> {
        use crate::entity::Store;
        let store = Store::from_current_context().await?;
        store.upsert(self).await
    }

    /// Delete this entity from the store
    async fn delete(&self) -> Result<()> {
        use crate::entity::Store;
        let store = Store::from_current_context().await?;
        store.delete::<Self>(self.id()).await
    }

    /// Create a query builder for finding entities of this type
    fn find() -> QueryBuilder<Self> {
        QueryBuilder::new()
    }
}

/// Trait for entity ID types
pub trait EntityId:
    Clone + Debug + Send + Sync + PartialEq + Eq + std::hash::Hash + 'static
{
    /// Convert ID to string representation for storage/query
    fn as_string(&self) -> String;

    /// Create ID from string representation
    fn from_string(s: &str) -> Result<Self>;

    /// Convert to generic ID type
    fn to_generic_id(&self) -> ID;
}

/// Implement EntityId for common types
impl EntityId for String {
    fn as_string(&self) -> String {
        self.clone()
    }

    fn from_string(s: &str) -> Result<Self> {
        Ok(s.to_string())
    }

    fn to_generic_id(&self) -> ID {
        ID::String(self.clone())
    }
}

impl EntityId for i64 {
    fn as_string(&self) -> String {
        self.to_string()
    }

    fn from_string(s: &str) -> Result<Self> {
        Ok(s.parse()?)
    }

    fn to_generic_id(&self) -> ID {
        ID::Int(*self)
    }
}

impl EntityId for uuid::Uuid {
    fn as_string(&self) -> String {
        self.to_string()
    }

    fn from_string(s: &str) -> Result<Self> {
        Ok(uuid::Uuid::parse_str(s)?)
    }

    fn to_generic_id(&self) -> ID {
        ID::Uuid(*self)
    }
}

impl EntityId for ID {
    fn as_string(&self) -> String {
        self.to_string()
    }

    fn from_string(s: &str) -> Result<Self> {
        // Try to parse as different types
        if let Ok(uuid) = uuid::Uuid::parse_str(s) {
            return Ok(ID::Uuid(uuid));
        }
        if let Ok(int) = s.parse::<i64>() {
            return Ok(ID::Int(int));
        }
        Ok(ID::String(s.to_string()))
    }

    fn to_generic_id(&self) -> ID {
        self.clone()
    }
}

/// Core trait for entity store operations
#[async_trait]
pub trait EntityStore: Send + Sync {
    /// Get an entity by ID
    async fn get<T: Entity>(&self, id: &T::Id) -> Result<Option<T>>
    where
        T: for<'de> serde::Deserialize<'de>;

    /// Get multiple entities by their IDs in a single optimized query
    /// Uses list with IN filter when ids.len() > 1, otherwise falls back to get()
    async fn get_many<T: Entity>(&self, ids: &[T::Id]) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize;

    /// Insert or update an entity
    async fn upsert<T: Entity>(&self, entity: &T) -> Result<()>
    where
        T: serde::Serialize;

    /// Insert or update multiple entities
    async fn upsert_many<T: Entity>(&self, entities: &[T]) -> Result<()>
    where
        T: serde::Serialize;

    /// Delete an entity by ID
    async fn delete<T: Entity>(&self, id: &T::Id) -> Result<()>;

    /// Delete multiple entities by ID
    async fn delete_many<T: Entity>(&self, ids: &[T::Id]) -> Result<()>;

    /// List entities with optional filtering
    async fn list<T: Entity>(&self, options: ListOptions<T>) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize;
}

/// Query filter for entity operations
#[derive(Debug, Clone)]
pub struct Filter<T: Entity> {
    pub field: String,
    pub operator: DbOperator,
    pub value: FilterValue,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Entity> Filter<T> {
    pub fn new(field: String, operator: FilterOperator, value: FilterValue) -> Self {
        Self {
            field,
            operator,
            value,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn eq<V>(field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        Self::new(field.to_string(), FilterOperator::Eq, value.into())
    }

    pub fn ne<V>(field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        Self::new(field.to_string(), FilterOperator::Ne, value.into())
    }

    pub fn gt<V>(field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        Self::new(field.to_string(), FilterOperator::Gt, value.into())
    }

    pub fn gte<V>(field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        Self::new(field.to_string(), FilterOperator::Ge, value.into())
    }

    pub fn lt<V>(field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        Self::new(field.to_string(), FilterOperator::Le, value.into())
    }

    pub fn lte<V>(field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        Self::new(field.to_string(), FilterOperator::Le, value.into())
    }

    /// Create an IN filter for matching any value in the provided array
    pub fn in_<V>(field: &str, values: Vec<V>) -> Self
    where
        V: Into<FilterValue>,
    {
        let filter_values: Vec<FilterValue> = values.into_iter().map(|v| v.into()).collect();
        Self::new(
            field.to_string(),
            FilterOperator::In,
            FilterValue::List(filter_values),
        )
    }
}

pub type FilterOperator = DbOperator;

/// Values that can be used in filters
#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Null,
    List(Vec<FilterValue>),
}

impl ToRichValue for FilterValue {
    fn to_rich_value(&self) -> Result<RichValue> {
        match self {
            FilterValue::String(s) => s.to_rich_value(),
            FilterValue::Int(i) => i.to_rich_value(),
            FilterValue::Float(f) => f.to_rich_value(),
            FilterValue::Boolean(b) => b.to_rich_value(),
            FilterValue::Null => Ok(RichValue {
                value: Some(Value::NullValue(0)),
            }),
            FilterValue::List(lst) => {
                let rich_list: Result<Vec<RichValue>> =
                    lst.iter().map(|v| v.to_rich_value()).collect();

                Ok(RichValue {
                    value: Some(Value::ListValue(RichValueList { values: rich_list? })),
                })
            }
        }
    }
}

impl From<String> for FilterValue {
    fn from(s: String) -> Self {
        FilterValue::String(s)
    }
}

impl From<&str> for FilterValue {
    fn from(s: &str) -> Self {
        FilterValue::String(s.to_string())
    }
}

impl From<i64> for FilterValue {
    fn from(i: i64) -> Self {
        FilterValue::Int(i)
    }
}

impl From<f64> for FilterValue {
    fn from(f: f64) -> Self {
        FilterValue::Float(f)
    }
}

impl From<bool> for FilterValue {
    fn from(b: bool) -> Self {
        FilterValue::Boolean(b)
    }
}

impl From<ID> for FilterValue {
    fn from(id: ID) -> Self {
        FilterValue::String(id.to_string())
    }
}

/// Options for listing entities
#[derive(Debug, Clone)]
pub struct ListOptions<T: Entity> {
    pub filters: Vec<Filter<T>>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

impl<T: Entity> Default for ListOptions<T> {
    fn default() -> Self {
        Self {
            filters: vec![],
            limit: None,
            cursor: None,
        }
    }
}

impl<T: Entity> ListOptions<T> {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Query builder for fluent entity querying API
#[derive(Debug, Clone)]
pub struct QueryBuilder<T: Entity> {
    filters: Vec<Filter<T>>,
    limit: Option<u32>,
    cursor: Option<String>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Entity> QueryBuilder<T> {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            limit: None,
            cursor: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add an equality filter
    pub fn where_eq<V>(mut self, field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::eq(field, value));
        self
    }

    /// Add a not-equal filter
    pub fn where_ne<V>(mut self, field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::ne(field, value));
        self
    }

    /// Add a greater-than filter
    pub fn where_gt<V>(mut self, field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::gt(field, value));
        self
    }

    /// Add a greater-than-or-equal filter
    pub fn where_gte<V>(mut self, field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::gte(field, value));
        self
    }

    /// Add a less-than filter
    pub fn where_lt<V>(mut self, field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::lt(field, value));
        self
    }

    /// Add a less-than-or-equal filter
    pub fn where_lte<V>(mut self, field: &str, value: V) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::lte(field, value));
        self
    }

    /// Add an IN filter (field matches any of the provided values)
    pub fn where_in<V>(mut self, field: &str, values: Vec<V>) -> Self
    where
        V: Into<FilterValue>,
    {
        self.filters.push(Filter::in_(field, values));
        self
    }

    /// Add a custom filter
    pub fn where_filter(mut self, filter: Filter<T>) -> Self {
        self.filters.push(filter);
        self
    }

    /// Set the maximum number of results to return
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the cursor for pagination
    pub fn cursor(mut self, cursor: String) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Execute the query and return the results
    pub async fn list(self) -> Result<Vec<T>> {
        use crate::entity::Store;
        let store = Store::from_current_context().await?;

        let options = ListOptions {
            filters: self.filters,
            limit: self.limit,
            cursor: self.cursor,
        };

        store.list(options).await
    }

    /// Execute the query and return the first result (if any)
    pub async fn first(self) -> Result<Option<T>> {
        let mut results = self.limit(1).list().await?;
        Ok(results.pop())
    }
}
