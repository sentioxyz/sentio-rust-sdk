//! GraphQL schema parser for entity definitions

use super::types::{EntitySchema, EntityType, FieldDefinition, FieldType, Directive, DirectiveArg};
use crate::entity::types::ScalarType;
use anyhow::{anyhow, Context, Result};
use graphql_parser::schema::{
    Document, Definition, TypeDefinition, ObjectType, Field, Type, Value
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parser for GraphQL schemas with entity directives
pub struct SchemaParser {
    /// Custom scalar types to register
    custom_scalars: HashMap<String, ScalarType>,
}

impl SchemaParser {
    /// Create a new schema parser
    pub fn new() -> Self {
        let mut custom_scalars = HashMap::new();
        
        // Register built-in custom scalars
        custom_scalars.insert("BigInt".to_string(), ScalarType::BigInt);
        custom_scalars.insert("BigDecimal".to_string(), ScalarType::BigDecimal);
        custom_scalars.insert("Timestamp".to_string(), ScalarType::Timestamp);
        custom_scalars.insert("Bytes".to_string(), ScalarType::Bytes);
        custom_scalars.insert("Int8".to_string(), ScalarType::Int8);

        Self { custom_scalars }
    }

    /// Parse a GraphQL schema file
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<EntitySchema> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read schema file: {}", path.as_ref().display()))?;
        
        self.parse_schema(&content)
            .with_context(|| format!("Failed to parse schema file: {}", path.as_ref().display()))
    }

    /// Parse GraphQL schema from string
    pub fn parse_schema(&self, content: &str) -> Result<EntitySchema> {
        let document = graphql_parser::parse_schema::<String>(content)
            .map_err(|e| anyhow!("GraphQL parse error: {}", e))?;

        self.process_document(&document)
    }

    /// Process the parsed GraphQL document
    fn process_document(&self, document: &Document<'_, String>) -> Result<EntitySchema> {
        let mut schema = EntitySchema::new();

        // Add custom scalars
        for (name, scalar) in &self.custom_scalars {
            schema.add_scalar(name.clone(), scalar.clone());
        }

        // Process all definitions
        for definition in &document.definitions {
            match definition {
                Definition::TypeDefinition(type_def) => {
                    self.process_type_definition(type_def, &mut schema)?;
                }
                Definition::DirectiveDefinition(_) => {
                    // We don't need to process directive definitions for now
                    // They're just for validation
                }
                _ => {
                    // Ignore other definition types (interfaces, unions, etc.)
                }
            }
        }

        Ok(schema)
    }

    /// Process a type definition
    fn process_type_definition(&self, type_def: &TypeDefinition<'_, String>, schema: &mut EntitySchema) -> Result<()> {
        match type_def {
            TypeDefinition::Object(object_type) => {
                self.process_object_type(object_type, schema)?;
            }
            TypeDefinition::Scalar(scalar_type) => {
                // Register custom scalar if not already known
                let name = scalar_type.name.clone();
                if !self.custom_scalars.contains_key(&name) && !self.is_builtin_scalar(&name) {
                    // Default to string representation for unknown scalars
                    schema.add_scalar(name, ScalarType::String);
                }
            }
            _ => {
                // Ignore other type definitions for now
            }
        }
        Ok(())
    }

    /// Process an object type (potential entity)
    fn process_object_type(&self, object_type: &ObjectType<'_, String>, schema: &mut EntitySchema) -> Result<()> {
        // Check if this type has an @entity directive
        let has_entity_directive = object_type.directives.iter()
            .any(|dir| dir.name == "entity");

        if !has_entity_directive {
            // Not an entity, skip
            return Ok(());
        }

        let mut entity = EntityType::new(object_type.name.clone());
        entity.description = object_type.description.clone();

        // Process directives
        for directive in &object_type.directives {
            let parsed_directive = self.parse_directive(directive)?;
            entity.add_directive(parsed_directive);
        }

        // Process fields
        for field in &object_type.fields {
            let field_def = self.parse_field(field, schema)?;
            entity.add_field(field.name.clone(), field_def);
        }

        // Validate entity structure
        self.validate_entity(&entity)?;

        schema.add_entity(entity.name.clone(), entity);
        Ok(())
    }

    /// Parse a GraphQL field into a field definition
    fn parse_field(&self, field: &Field<'_, String>, schema: &EntitySchema) -> Result<FieldDefinition> {
        let mut field_def = FieldDefinition::new(field.name.clone(), self.parse_type(&field.field_type, schema)?);
        field_def.description = field.description.clone();

        // Process field directives
        for directive in &field.directives {
            let parsed_directive = self.parse_directive(directive)?;
            field_def.add_directive(parsed_directive);
        }

        Ok(field_def)
    }

    /// Parse a GraphQL type into a FieldType
    fn parse_type(&self, gql_type: &Type<'_, String>, schema: &EntitySchema) -> Result<FieldType> {
        match gql_type {
            Type::NamedType(name) => {
                // Check if it's a custom scalar
                if let Some(scalar) = self.custom_scalars.get(name) {
                    return Ok(FieldType::Scalar(scalar.clone()));
                }

                // Check built-in scalars
                if let Some(scalar) = self.parse_builtin_scalar(name) {
                    return Ok(FieldType::Scalar(scalar));
                }

                // Must be an object type (entity reference)
                Ok(FieldType::Object(name.clone()))
            }
            Type::NonNullType(inner) => {
                let inner_type = self.parse_type(inner, schema)?;
                Ok(FieldType::NonNull(Box::new(inner_type)))
            }
            Type::ListType(inner) => {
                let inner_type = self.parse_type(inner, schema)?;
                Ok(FieldType::List(Box::new(inner_type)))
            }
        }
    }

    /// Parse a GraphQL directive
    fn parse_directive(&self, directive: &graphql_parser::schema::Directive<'_, String>) -> Result<Directive> {
        let mut parsed_directive = Directive::new(directive.name.clone());

        for (arg_name, arg_value) in &directive.arguments {
            let parsed_value = self.parse_directive_value(arg_value)?;
            parsed_directive.add_argument(arg_name.clone(), parsed_value);
        }

        Ok(parsed_directive)
    }

    /// Parse a directive argument value
    fn parse_directive_value(&self, value: &Value<'_, String>) -> Result<DirectiveArg> {
        match value {
            Value::String(s) => Ok(DirectiveArg::String(s.clone())),
            Value::Int(i) => {
                let int_val = i.as_i64().ok_or_else(|| anyhow!("Integer value out of range"))?;
                Ok(DirectiveArg::Int(int_val))
            }
            Value::Float(f) => Ok(DirectiveArg::Float(*f)),
            Value::Boolean(b) => Ok(DirectiveArg::Boolean(*b)),
            Value::Null => Ok(DirectiveArg::Null),
            _ => Err(anyhow!("Unsupported directive argument type: {:?}", value)),
        }
    }

    /// Parse built-in GraphQL scalar types
    fn parse_builtin_scalar(&self, name: &str) -> Option<ScalarType> {
        match name {
            "ID" => Some(ScalarType::ID),
            "String" => Some(ScalarType::String),
            "Int" => Some(ScalarType::Int),
            "Float" => Some(ScalarType::Float),
            "Boolean" => Some(ScalarType::Boolean),
            _ => None,
        }
    }

    /// Check if a scalar type is built-in
    fn is_builtin_scalar(&self, name: &str) -> bool {
        matches!(name, "ID" | "String" | "Int" | "Float" | "Boolean")
    }

    /// Validate entity structure
    fn validate_entity(&self, entity: &EntityType) -> Result<()> {
        // Every entity must have an id field
        let id_field = entity.fields.get("id")
            .ok_or_else(|| anyhow!("Entity '{}' must have an 'id' field", entity.name))?;

        // Check id field type based on entity type
        if entity.is_timeseries() {
            // Timeseries entities must have Int8! id
            if !matches!(id_field.field_type, FieldType::NonNull(ref inner) if matches!(**inner, FieldType::Scalar(ScalarType::Int8))) {
                return Err(anyhow!("Timeseries entity '{}' must have 'id: Int8!' field", entity.name));
            }

            // Timeseries entities must have a timestamp field
            if !entity.fields.contains_key("timestamp") {
                return Err(anyhow!("Timeseries entity '{}' must have a 'timestamp' field", entity.name));
            }

            let timestamp_field = &entity.fields["timestamp"];
            if !matches!(timestamp_field.field_type, FieldType::NonNull(ref inner) if matches!(**inner, FieldType::Scalar(ScalarType::Timestamp))) {
                return Err(anyhow!("Timeseries entity '{}' must have 'timestamp: Timestamp!' field", entity.name));
            }
        } else {
            // Regular entities should have ID! id
            if !matches!(id_field.field_type, FieldType::NonNull(ref inner) if matches!(**inner, FieldType::Scalar(ScalarType::ID))) {
                return Err(anyhow!("Entity '{}' should have 'id: ID!' field", entity.name));
            }
        }

        // Validate derived fields
        for (field_name, field) in &entity.fields {
            if let Some(derived_from) = field.get_directive("derivedFrom") {
                if let Some(_from_field) = derived_from.get_string_arg("field") {
                    // The field must be a list type for derived relations
                    if !field.is_list() {
                        return Err(anyhow!("Derived field '{}' in entity '{}' must be a list type", field_name, entity.name));
                    }
                } else {
                    return Err(anyhow!("@derivedFrom directive on field '{}' in entity '{}' must have a 'field' argument", field_name, entity.name));
                }
            }
        }

        Ok(())
    }
}

impl Default for SchemaParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_entity() {
        let schema = r#"
            type User @entity {
                id: ID!
                name: String!
                email: String!
            }
        "#;

        let parser = SchemaParser::new();
        let result = parser.parse_schema(schema).unwrap();

        assert_eq!(result.entities.len(), 1);
        assert!(result.entities.contains_key("User"));

        let user = &result.entities["User"];
        assert_eq!(user.fields.len(), 3);
        assert!(user.fields.contains_key("id"));
        assert!(user.fields.contains_key("name"));
        assert!(user.fields.contains_key("email"));
    }

    #[test]
    fn test_parse_timeseries_entity() {
        let schema = r#"
            type Transaction @entity(timeseries: true) {
                id: Int8!
                timestamp: Timestamp!
                amount: BigDecimal!
            }
        "#;

        let parser = SchemaParser::new();
        let result = parser.parse_schema(schema).unwrap();

        assert_eq!(result.entities.len(), 1);
        let transaction = &result.entities["Transaction"];
        assert!(transaction.is_timeseries());
    }

    #[test]
    fn test_parse_entity_with_relations() {
        let schema = r#"
            type User @entity {
                id: ID!
                name: String!
                transactions: [Transaction!]! @derivedFrom(field: "user")
            }

            type Transaction @entity {
                id: ID!
                user: User!
                amount: BigDecimal!
            }
        "#;

        let parser = SchemaParser::new();
        let result = parser.parse_schema(schema).unwrap();

        assert_eq!(result.entities.len(), 2);
        
        let user = &result.entities["User"];
        let transactions_field = &user.fields["transactions"];
        assert!(transactions_field.has_directive("derivedFrom"));
        assert!(transactions_field.is_list());

        let transaction = &result.entities["Transaction"];
        let user_field = &transaction.fields["user"];
        assert!(user_field.is_relation());
    }

    #[test]
    fn test_validate_entity_missing_id() {
        let schema = r#"
            type User @entity {
                name: String!
            }
        "#;

        let parser = SchemaParser::new();
        let result = parser.parse_schema(schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must have an 'id' field"));
    }

    #[test]
    fn test_validate_timeseries_entity_wrong_id_type() {
        let schema = r#"
            type Transaction @entity(timeseries: true) {
                id: ID!
                timestamp: Timestamp!
            }
        "#;

        let parser = SchemaParser::new();
        let result = parser.parse_schema(schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must have 'id: Int8!' field"));
    }
}