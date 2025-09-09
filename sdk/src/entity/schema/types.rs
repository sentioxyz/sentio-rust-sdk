//! GraphQL schema type definitions

use crate::entity::types::ScalarType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete parsed GraphQL schema for entities
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySchema {
    /// All entity types defined in the schema
    pub entities: HashMap<String, EntityType>,
    /// Custom scalar types
    pub scalars: HashMap<String, ScalarType>,
}

impl Default for EntitySchema {
    fn default() -> Self {
        Self::new()
    }
}

impl EntitySchema {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            scalars: HashMap::new(),
        }
    }

    /// Add an entity type to the schema
    pub fn add_entity(&mut self, name: String, entity: EntityType) {
        self.entities.insert(name, entity);
    }

    /// Add a custom scalar type
    pub fn add_scalar(&mut self, name: String, scalar: ScalarType) {
        self.scalars.insert(name, scalar);
    }

    /// Get all entity names
    pub fn entity_names(&self) -> Vec<&String> {
        self.entities.keys().collect()
    }

    /// Check if a type name is an entity
    pub fn is_entity(&self, type_name: &str) -> bool {
        self.entities.contains_key(type_name)
    }

    /// Get entity by name
    pub fn get_entity(&self, name: &str) -> Option<&EntityType> {
        self.entities.get(name)
    }

    /// Get all entities as an iterator
    pub fn get_entities(&self) -> impl Iterator<Item = (&String, &EntityType)> {
        self.entities.iter()
    }

    /// Get count of entities
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
}

/// Represents an entity type (GraphQL type with @entity directive)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityType {
    /// Entity name
    pub name: String,
    /// Fields defined on the entity
    pub fields: HashMap<String, FieldDefinition>,
    /// Directives applied to the entity
    pub directives: Vec<Directive>,
    /// Documentation/description
    pub description: Option<String>,
}

impl EntityType {
    pub fn new(name: String) -> Self {
        Self {
            name,
            fields: HashMap::new(),
            directives: Vec::new(),
            description: None,
        }
    }

    /// Add a field to the entity
    pub fn add_field(&mut self, name: String, field: FieldDefinition) {
        self.fields.insert(name, field);
    }

    /// Add a directive to the entity
    pub fn add_directive(&mut self, directive: Directive) {
        self.directives.push(directive);
    }

    /// Check if entity has a specific directive
    pub fn has_directive(&self, name: &str) -> bool {
        self.directives.iter().any(|d| d.name == name)
    }

    /// Get a specific directive
    pub fn get_directive(&self, name: &str) -> Option<&Directive> {
        self.directives.iter().find(|d| d.name == name)
    }

    /// Check if this is a timeseries entity
    pub fn is_timeseries(&self) -> bool {
        if let Some(entity_directive) = self.get_directive("entity") {
            entity_directive.get_bool_arg("timeseries").unwrap_or(false)
        } else {
            false
        }
    }

    /// Check if this is an immutable entity
    pub fn is_immutable(&self) -> bool {
        if let Some(entity_directive) = self.get_directive("entity") {
            entity_directive.get_bool_arg("immutable").unwrap_or(false)
        } else {
            false
        }
    }

    /// Get the primary key field (usually 'id')
    pub fn get_primary_key_field(&self) -> Option<&FieldDefinition> {
        // For timeseries entities, look for Int8! id field
        if self.is_timeseries() {
            self.fields.get("id").filter(|f| {
                matches!(f.field_type, FieldType::NonNull(ref inner) if matches!(**inner, FieldType::Scalar(ScalarType::Int8)))
            })
        } else {
            // For regular entities, look for ID! field
            self.fields.get("id").filter(|f| {
                matches!(f.field_type, FieldType::NonNull(ref inner) if matches!(**inner, FieldType::Scalar(ScalarType::ID)))
            })
        }
    }

    /// Get all relation fields
    pub fn get_relation_fields(&self) -> Vec<(&String, &FieldDefinition)> {
        self.fields.iter()
            .filter(|(_, field)| field.is_relation())
            .collect()
    }

    /// Get all derived fields (fields with @derivedFrom)
    pub fn get_derived_fields(&self) -> Vec<(&String, &FieldDefinition)> {
        self.fields.iter()
            .filter(|(_, field)| field.has_directive("derivedFrom"))
            .collect()
    }
}

/// Field definition within an entity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: FieldType,
    /// Directives applied to the field
    pub directives: Vec<Directive>,
    /// Field description
    pub description: Option<String>,
}

impl FieldDefinition {
    pub fn new(name: String, field_type: FieldType) -> Self {
        Self {
            name,
            field_type,
            directives: Vec::new(),
            description: None,
        }
    }

    /// Add a directive to the field
    pub fn add_directive(&mut self, directive: Directive) {
        self.directives.push(directive);
    }

    /// Check if field has a specific directive
    pub fn has_directive(&self, name: &str) -> bool {
        self.directives.iter().any(|d| d.name == name)
    }

    /// Get a specific directive
    pub fn get_directive(&self, name: &str) -> Option<&Directive> {
        self.directives.iter().find(|d| d.name == name)
    }

    /// Check if this field is required (non-null)
    pub fn is_required(&self) -> bool {
        matches!(self.field_type, FieldType::NonNull(_))
    }

    /// Check if this field is a list
    pub fn is_list(&self) -> bool {
        self.field_type.is_list()
    }

    /// Check if this field is a relation to another entity
    pub fn is_relation(&self) -> bool {
        self.field_type.is_object_type()
    }

    /// Check if this field is unique
    pub fn is_unique(&self) -> bool {
        self.has_directive("unique")
    }

    /// Check if this field is indexed
    pub fn is_indexed(&self) -> bool {
        self.has_directive("index")
    }

    /// Get the underlying type (without NonNull/List wrappers)
    pub fn base_type(&self) -> &FieldType {
        self.field_type.base_type()
    }
}

/// GraphQL field types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    /// Scalar type (String, Int, Boolean, etc.)
    Scalar(ScalarType),
    /// Object type (reference to another entity)
    Object(String),
    /// Non-null wrapper
    NonNull(Box<FieldType>),
    /// List wrapper
    List(Box<FieldType>),
}

impl FieldType {
    /// Get the base type without wrappers
    pub fn base_type(&self) -> &FieldType {
        match self {
            FieldType::NonNull(inner) => inner.base_type(),
            FieldType::List(inner) => inner.base_type(),
            _ => self,
        }
    }

    /// Check if this is a list type
    pub fn is_list(&self) -> bool {
        match self {
            FieldType::List(_) => true,
            FieldType::NonNull(inner) => inner.is_list(),
            _ => false,
        }
    }

    /// Check if this is non-null
    pub fn is_non_null(&self) -> bool {
        matches!(self, FieldType::NonNull(_))
    }

    /// Check if this is an object type
    pub fn is_object_type(&self) -> bool {
        matches!(self.base_type(), FieldType::Object(_))
    }

    /// Get the object type name if this is an object type
    pub fn get_object_name(&self) -> Option<&String> {
        match self.base_type() {
            FieldType::Object(name) => Some(name),
            _ => None,
        }
    }

    /// Get the Rust type representation for code generation
    pub fn rust_type(&self, schema: &EntitySchema) -> String {
        match self {
            FieldType::Scalar(scalar) => scalar.rust_type().to_string(),
            FieldType::Object(name) => {
                if schema.is_entity(name) {
                    name.clone()
                } else {
                    format!("Unknown<{}>", name)
                }
            }
            FieldType::NonNull(inner) => inner.rust_type(schema),
            FieldType::List(inner) => format!("Vec<{}>", inner.rust_type(schema)),
        }
    }

    /// Check if this type is optional in Rust (not wrapped in NonNull)
    pub fn is_optional(&self) -> bool {
        !matches!(self, FieldType::NonNull(_))
    }
}

/// GraphQL directive (e.g., @entity, @unique, @index)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Directive {
    /// Directive name
    pub name: String,
    /// Directive arguments
    pub arguments: HashMap<String, DirectiveArg>,
}

impl Directive {
    pub fn new(name: String) -> Self {
        Self {
            name,
            arguments: HashMap::new(),
        }
    }

    /// Add an argument to the directive
    pub fn add_argument(&mut self, name: String, value: DirectiveArg) {
        self.arguments.insert(name, value);
    }

    /// Get a string argument value
    pub fn get_string_arg(&self, name: &str) -> Option<&String> {
        match self.arguments.get(name) {
            Some(DirectiveArg::String(s)) => Some(s),
            _ => None,
        }
    }

    /// Get a boolean argument value
    pub fn get_bool_arg(&self, name: &str) -> Option<bool> {
        match self.arguments.get(name) {
            Some(DirectiveArg::Boolean(b)) => Some(*b),
            _ => None,
        }
    }

    /// Get an integer argument value
    pub fn get_int_arg(&self, name: &str) -> Option<i64> {
        match self.arguments.get(name) {
            Some(DirectiveArg::Int(i)) => Some(*i),
            _ => None,
        }
    }
}

/// Directive argument values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DirectiveArg {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

impl From<String> for DirectiveArg {
    fn from(s: String) -> Self {
        DirectiveArg::String(s)
    }
}

impl From<&str> for DirectiveArg {
    fn from(s: &str) -> Self {
        DirectiveArg::String(s.to_string())
    }
}

impl From<i64> for DirectiveArg {
    fn from(i: i64) -> Self {
        DirectiveArg::Int(i)
    }
}

impl From<f64> for DirectiveArg {
    fn from(f: f64) -> Self {
        DirectiveArg::Float(f)
    }
}

impl From<bool> for DirectiveArg {
    fn from(b: bool) -> Self {
        DirectiveArg::Boolean(b)
    }
}