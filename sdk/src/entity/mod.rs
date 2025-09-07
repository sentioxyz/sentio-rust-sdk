//! Entity Framework for Sentio Rust SDK
//!
//! This module provides GraphQL schema-driven entity management for Sentio processors.
//! It enables users to define GraphQL schemas with custom directives and automatically
//! generates type-safe Rust entity structs and store operations.

pub mod codegen;
pub mod schema;
pub mod serialization;
pub mod store;
pub mod traits;
pub mod types;

// Re-export commonly used types and traits
pub use serialization::{FromRichValue, ToRichValue, from_rich_struct, to_rich_struct};
pub use store::{Store, StoreContext};
pub use traits::{Entity, EntityId, EntityStore, Filter, ListOptions, QueryBuilder};
pub use types::{BigDecimal, BigInt, Bytes, EntityError, EntityResult, ID, Int8, Timestamp};

// Re-export schema types
pub use schema::{Directive, EntitySchema, EntityType, FieldType};
