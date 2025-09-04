//! Entity Framework for Sentio Rust SDK
//! 
//! This module provides GraphQL schema-driven entity management for Sentio processors.
//! It enables users to define GraphQL schemas with custom directives and automatically
//! generates type-safe Rust entity structs and store operations.

pub mod schema;
pub mod store;
pub mod traits;
pub mod types;
pub mod codegen;
pub mod serialization;

// Re-export commonly used types and traits
pub use traits::{Entity, EntityId, EntityStore, Filter, ListOptions};
pub use store::{Store, StoreContext};
pub use types::{ID, BigDecimal, BigInt, Timestamp, Bytes, Int8, EntityError, EntityResult};
pub use serialization::{ToRichValue, FromRichValue, to_rich_struct, from_rich_struct};

// Re-export schema types
pub use schema::{EntitySchema, EntityType, FieldType, Directive};