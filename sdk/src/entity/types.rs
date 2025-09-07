//! Core types for the entity framework

use chrono::{DateTime, Utc};
use bigdecimal::BigDecimal as BigDecimalImpl;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Entity ID type - can be String, i64, or UUID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ID {
    String(String),
    Int(i64),
    Uuid(uuid::Uuid),
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ID::String(s) => write!(f, "{}", s),
            ID::Int(i) => write!(f, "{}", i),
            ID::Uuid(u) => write!(f, "{}", u),
        }
    }
}

impl From<String> for ID {
    fn from(s: String) -> Self {
        ID::String(s)
    }
}

impl From<&str> for ID {
    fn from(s: &str) -> Self {
        ID::String(s.to_string())
    }
}

impl From<i64> for ID {
    fn from(i: i64) -> Self {
        ID::Int(i)
    }
}

impl From<uuid::Uuid> for ID {
    fn from(u: uuid::Uuid) -> Self {
        ID::Uuid(u)
    }
}

/// BigDecimal type for high-precision decimal numbers
pub type BigDecimal = BigDecimalImpl;

/// Timestamp type for date/time values
pub type Timestamp = DateTime<Utc>;

/// Bytes type for binary data
pub type Bytes = bytes::Bytes;

/// Int8 type for 64-bit signed integers (used for timeseries entity IDs)
pub type Int8 = i64;

/// BigInt type for arbitrary precision integers  
pub type BigInt = num_bigint::BigInt;

/// Supported GraphQL scalar types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScalarType {
    ID,
    String,
    Int,
    Float,
    Boolean,
    BigInt,
    BigDecimal,
    Timestamp,
    Bytes,
    Int8,
}

impl ScalarType {
    /// Get the Rust type name for code generation
    pub fn rust_type(&self) -> &'static str {
        match self {
            ScalarType::ID => "ID",
            ScalarType::String => "String",
            ScalarType::Int => "i32",
            ScalarType::Float => "f64", 
            ScalarType::Boolean => "bool",
            ScalarType::BigInt => "BigInt",
            ScalarType::BigDecimal => "BigDecimal",
            ScalarType::Timestamp => "Timestamp",
            ScalarType::Bytes => "Bytes",
            ScalarType::Int8 => "Int8",
        }
    }

    /// Check if this is a custom scalar type (not standard GraphQL)
    pub fn is_custom(&self) -> bool {
        matches!(self, 
            ScalarType::BigInt |
            ScalarType::BigDecimal | 
            ScalarType::Timestamp | 
            ScalarType::Bytes | 
            ScalarType::Int8
        )
    }
}

impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ScalarType::ID => "ID",
            ScalarType::String => "String",
            ScalarType::Int => "Int",
            ScalarType::Float => "Float",
            ScalarType::Boolean => "Boolean",
            ScalarType::BigInt => "BigInt",
            ScalarType::BigDecimal => "BigDecimal",
            ScalarType::Timestamp => "Timestamp",
            ScalarType::Bytes => "Bytes",
            ScalarType::Int8 => "Int8",
        };
        write!(f, "{}", name)
    }
}

/// Comprehensive error type for entity operations
#[derive(Error, Debug)]
pub enum EntityError {
    /// Entity not found by the given ID
    #[error("Entity '{entity_type}' with id '{id}' not found")]
    NotFound {
        entity_type: String,
        id: String,
    },

    /// Invalid entity ID format
    #[error("Invalid ID format for entity '{entity_type}': {reason}")]
    InvalidId {
        entity_type: String,
        reason: String,
    },

    /// Validation error for entity fields
    #[error("Validation error for entity '{entity_type}': {field} - {reason}")]
    Validation {
        entity_type: String,
        field: String,
        reason: String,
    },

    /// Constraint violation (unique, foreign key, etc.)
    #[error("Constraint violation for entity '{entity_type}': {constraint}")]
    ConstraintViolation {
        entity_type: String,
        constraint: String,
    },

    /// Serialization/deserialization error
    #[error("Serialization error for entity '{entity_type}': {message}")]
    Serialization {
        entity_type: String,
        message: String,
    },

    /// Database/store operation error
    #[error("Store operation failed for entity '{entity_type}': {operation} - {reason}")]
    Store {
        entity_type: String,
        operation: String,
        reason: String,
    },

    /// Schema-related error
    #[error("Schema error for entity '{entity_type}': {reason}")]
    Schema {
        entity_type: String,
        reason: String,
    },

    /// Type conversion error
    #[error("Type conversion error for entity '{entity_type}' field '{field}': expected {expected}, got {actual}")]
    TypeConversion {
        entity_type: String,
        field: String,
        expected: String,
        actual: String,
    },

    /// Permission/authorization error
    #[error("Permission denied for operation '{operation}' on entity '{entity_type}'")]
    Permission {
        entity_type: String,
        operation: String,
    },

    /// Configuration error
    #[error("Configuration error: {reason}")]
    Configuration {
        reason: String,
    },

    /// Generic error with context
    #[error("Error in entity '{entity_type}': {message}")]
    Generic {
        entity_type: String,
        message: String,
    },

    /// Internal system error
    #[error("Internal error: {message}")]
    Internal {
        message: String,
    },

    /// Builder pattern error
    #[error("Builder error for entity '{entity_type}': {message}")]
    Builder {
        entity_type: String,
        message: String,
    },
}

impl EntityError {
    /// Create a NotFound error
    pub fn not_found<E: AsRef<str>, I: fmt::Display>(entity_type: E, id: I) -> Self {
        Self::NotFound {
            entity_type: entity_type.as_ref().to_string(),
            id: id.to_string(),
        }
    }

    /// Create an InvalidId error
    pub fn invalid_id<E: AsRef<str>, R: AsRef<str>>(entity_type: E, reason: R) -> Self {
        Self::InvalidId {
            entity_type: entity_type.as_ref().to_string(),
            reason: reason.as_ref().to_string(),
        }
    }

    /// Create a Validation error
    pub fn validation<E: AsRef<str>, F: AsRef<str>, R: AsRef<str>>(
        entity_type: E,
        field: F,
        reason: R,
    ) -> Self {
        Self::Validation {
            entity_type: entity_type.as_ref().to_string(),
            field: field.as_ref().to_string(),
            reason: reason.as_ref().to_string(),
        }
    }

    /// Create a ConstraintViolation error
    pub fn constraint_violation<E: AsRef<str>, C: AsRef<str>>(entity_type: E, constraint: C) -> Self {
        Self::ConstraintViolation {
            entity_type: entity_type.as_ref().to_string(),
            constraint: constraint.as_ref().to_string(),
        }
    }

    /// Create a Serialization error
    pub fn serialization<E: AsRef<str>>(entity_type: E, source: serde_json::Error) -> Self {
        Self::Serialization {
            entity_type: entity_type.as_ref().to_string(),
            message: source.to_string(),
        }
    }

    /// Create a Store error
    pub fn store<E: AsRef<str>, O: AsRef<str>, R: AsRef<str>>(
        entity_type: E,
        operation: O,
        reason: R,
    ) -> Self {
        Self::Store {
            entity_type: entity_type.as_ref().to_string(),
            operation: operation.as_ref().to_string(),
            reason: reason.as_ref().to_string(),
        }
    }

    /// Create a Schema error
    pub fn schema<E: AsRef<str>, R: AsRef<str>>(entity_type: E, reason: R) -> Self {
        Self::Schema {
            entity_type: entity_type.as_ref().to_string(),
            reason: reason.as_ref().to_string(),
        }
    }

    /// Create a TypeConversion error
    pub fn type_conversion<E: AsRef<str>, F: AsRef<str>, Ex: AsRef<str>, A: AsRef<str>>(
        entity_type: E,
        field: F,
        expected: Ex,
        actual: A,
    ) -> Self {
        Self::TypeConversion {
            entity_type: entity_type.as_ref().to_string(),
            field: field.as_ref().to_string(),
            expected: expected.as_ref().to_string(),
            actual: actual.as_ref().to_string(),
        }
    }

    /// Create a Permission error
    pub fn permission<E: AsRef<str>, O: AsRef<str>>(entity_type: E, operation: O) -> Self {
        Self::Permission {
            entity_type: entity_type.as_ref().to_string(),
            operation: operation.as_ref().to_string(),
        }
    }

    /// Create a Configuration error
    pub fn configuration<R: AsRef<str>>(reason: R) -> Self {
        Self::Configuration {
            reason: reason.as_ref().to_string(),
        }
    }

    /// Create a Generic error
    pub fn generic<E: AsRef<str>, M: AsRef<str>>(entity_type: E, message: M) -> Self {
        Self::Generic {
            entity_type: entity_type.as_ref().to_string(),
            message: message.as_ref().to_string(),
        }
    }

    /// Create an Internal error
    pub fn internal<M: AsRef<str>>(message: M) -> Self {
        Self::Internal {
            message: message.as_ref().to_string(),
        }
    }

    /// Create a Builder error
    pub fn builder<E: AsRef<str>, M: AsRef<str>>(entity_type: E, message: M) -> Self {
        Self::Builder {
            entity_type: entity_type.as_ref().to_string(),
            message: message.as_ref().to_string(),
        }
    }

    /// Get the entity type associated with this error (if any)
    pub fn entity_type(&self) -> Option<&str> {
        match self {
            Self::NotFound { entity_type, .. }
            | Self::InvalidId { entity_type, .. }
            | Self::Validation { entity_type, .. }
            | Self::ConstraintViolation { entity_type, .. }
            | Self::Serialization { entity_type, .. }
            | Self::Store { entity_type, .. }
            | Self::Schema { entity_type, .. }
            | Self::TypeConversion { entity_type, .. }
            | Self::Permission { entity_type, .. }
            | Self::Generic { entity_type, .. }
            | Self::Builder { entity_type, .. } => Some(entity_type),
            Self::Configuration { .. } | Self::Internal { .. } => None,
        }
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::NotFound { .. } 
            | Self::InvalidId { .. } 
            | Self::Validation { .. } 
            | Self::Permission { .. }
            | Self::Builder { .. } => true,
            
            Self::ConstraintViolation { .. } 
            | Self::TypeConversion { .. } => false,
            
            Self::Serialization { .. } 
            | Self::Store { .. } 
            | Self::Schema { .. } 
            | Self::Configuration { .. } 
            | Self::Generic { .. } 
            | Self::Internal { .. } => false,
        }
    }
}

/// Result type alias for entity operations
pub type EntityResult<T> = Result<T, EntityError>;

// Implement From trait for derive_builder compatibility
impl From<derive_builder::UninitializedFieldError> for EntityError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        EntityError::Builder {
            entity_type: "Unknown".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<anyhow::Error> for EntityError {
    fn from(error: anyhow::Error) -> Self {
        EntityError::Internal {
            message: error.to_string(),
        }
    }
}