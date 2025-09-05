//! Schema validation for entity definitions

use super::types::{EntitySchema, EntityType, FieldDefinition, FieldType};
use anyhow::Result;
use std::collections::HashSet;

/// Validator for entity schemas
pub struct SchemaValidator {
    /// Validation errors collected during validation
    errors: Vec<String>,
    /// Validation warnings collected during validation
    warnings: Vec<String>,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Validate a complete entity schema
    pub fn validate(&mut self, schema: &EntitySchema) -> Result<ValidationResult> {
        self.errors.clear();
        self.warnings.clear();

        // Validate each entity
        for (name, entity) in &schema.entities {
            self.validate_entity(name, entity, schema);
        }

        // Validate cross-entity relationships
        self.validate_relationships(schema);

        Ok(ValidationResult {
            errors: self.errors.clone(),
            warnings: self.warnings.clone(),
        })
    }

    /// Validate a single entity
    fn validate_entity(&mut self, name: &str, entity: &EntityType, schema: &EntitySchema) {
        // Validate entity name
        if name.is_empty() {
            self.errors.push("Entity name cannot be empty".to_string());
            return;
        }

        if !name.chars().next().unwrap_or('a').is_ascii_uppercase() {
            self.warnings.push(format!("Entity name '{}' should start with an uppercase letter", name));
        }

        // Validate required fields
        self.validate_required_fields(entity);

        // Validate field definitions
        for (field_name, field) in &entity.fields {
            self.validate_field(entity, field_name, field, schema);
        }

        // Validate directives
        self.validate_entity_directives(entity);

        // Validate timeseries-specific rules
        if entity.is_timeseries() {
            self.validate_timeseries_entity(entity);
        }

        // Validate immutable-specific rules
        if entity.is_immutable() {
            self.validate_immutable_entity(entity);
        }
    }

    /// Validate required fields for an entity
    fn validate_required_fields(&mut self, entity: &EntityType) {
        // Every entity must have an id field
        if !entity.fields.contains_key("id") {
            self.errors.push(format!("Entity '{}' must have an 'id' field", entity.name));
            return;
        }

        let id_field = &entity.fields["id"];
        
        // Validate id field type
        if entity.is_timeseries() {
            if !self.is_int8_non_null(&id_field.field_type) {
                self.errors.push(format!("Timeseries entity '{}' must have 'id: Int8!' field", entity.name));
            }
        } else {
            if !self.is_id_non_null(&id_field.field_type) {
                self.errors.push(format!("Entity '{}' should have 'id: ID!' field", entity.name));
            }
        }

        // Timeseries entities must have a timestamp field
        if entity.is_timeseries() && !entity.fields.contains_key("timestamp") {
            self.errors.push(format!("Timeseries entity '{}' must have a 'timestamp' field", entity.name));
        }

        if entity.is_timeseries() {
            if let Some(timestamp_field) = entity.fields.get("timestamp") {
                if !self.is_timestamp_non_null(&timestamp_field.field_type) {
                    self.errors.push(format!("Timeseries entity '{}' must have 'timestamp: Timestamp!' field", entity.name));
                }
            }
        }
    }

    /// Validate a field definition
    fn validate_field(&mut self, entity: &EntityType, field_name: &str, field: &FieldDefinition, schema: &EntitySchema) {
        // Validate field name
        if field_name.is_empty() {
            self.errors.push(format!("Field name cannot be empty in entity '{}'", entity.name));
            return;
        }

        if field_name.starts_with(char::is_uppercase) {
            self.warnings.push(format!("Field name '{}' in entity '{}' should start with a lowercase letter", field_name, entity.name));
        }

        // Validate field type
        self.validate_field_type(&field.field_type, entity, field_name, schema);

        // Validate field directives
        self.validate_field_directives(entity, field_name, field, schema);
    }

    /// Validate field type
    fn validate_field_type(&mut self, field_type: &FieldType, entity: &EntityType, field_name: &str, schema: &EntitySchema) {
        match field_type {
            FieldType::Object(type_name) => {
                // Check if the referenced type exists
                if !schema.is_entity(type_name) {
                    self.errors.push(format!(
                        "Field '{}' in entity '{}' references unknown type '{}'", 
                        field_name, entity.name, type_name
                    ));
                }
            }
            FieldType::NonNull(inner) => {
                self.validate_field_type(inner, entity, field_name, schema);
            }
            FieldType::List(inner) => {
                self.validate_field_type(inner, entity, field_name, schema);
            }
            FieldType::Scalar(_) => {
                // Scalars are always valid
            }
        }
    }

    /// Validate field directives
    fn validate_field_directives(&mut self, entity: &EntityType, field_name: &str, field: &FieldDefinition, schema: &EntitySchema) {
        for directive in &field.directives {
            match directive.name.as_str() {
                "unique" => {
                    // Unique fields should probably be indexed
                    if !field.has_directive("index") {
                        self.warnings.push(format!(
                            "Unique field '{}' in entity '{}' should consider adding @index directive", 
                            field_name, entity.name
                        ));
                    }
                }
                "derivedFrom" => {
                    self.validate_derived_from_directive(entity, field_name, field, directive, schema);
                }
                "index" => {
                    // Index directives are generally fine
                }
                _ => {
                    self.warnings.push(format!(
                        "Unknown directive '@{}' on field '{}' in entity '{}'",
                        directive.name, field_name, entity.name
                    ));
                }
            }
        }
    }

    /// Validate @derivedFrom directive
    fn validate_derived_from_directive(
        &mut self, 
        entity: &EntityType, 
        field_name: &str, 
        field: &FieldDefinition, 
        directive: &super::types::Directive, 
        schema: &EntitySchema
    ) {
        // Must have a 'field' argument
        let from_field = match directive.get_string_arg("field") {
            Some(field) => field,
            None => {
                self.errors.push(format!(
                    "@derivedFrom directive on field '{}' in entity '{}' must have a 'field' argument",
                    field_name, entity.name
                ));
                return;
            }
        };

        // Field must be a list type
        if !field.is_list() {
            self.errors.push(format!(
                "Field '{}' with @derivedFrom in entity '{}' must be a list type",
                field_name, entity.name
            ));
        }

        // Validate the referenced field exists in the target entity
        if let Some(target_type_name) = field.base_type().get_object_name() {
            if let Some(target_entity) = schema.get_entity(target_type_name) {
                if !target_entity.fields.contains_key(from_field) {
                    self.errors.push(format!(
                        "@derivedFrom references non-existent field '{}' in entity '{}'",
                        from_field, target_type_name
                    ));
                } else {
                    // Check if the referenced field points back to this entity
                    let referenced_field = &target_entity.fields[from_field];
                    if let Some(ref_type_name) = referenced_field.base_type().get_object_name() {
                        if ref_type_name != &entity.name {
                            self.warnings.push(format!(
                                "@derivedFrom field '{}' references field '{}' in '{}' that doesn't point back to '{}'",
                                field_name, from_field, target_type_name, entity.name
                            ));
                        }
                    }
                }
            }
        }
    }

    /// Validate entity directives
    fn validate_entity_directives(&mut self, entity: &EntityType) {
        let mut has_entity_directive = false;

        for directive in &entity.directives {
            match directive.name.as_str() {
                "entity" => {
                    has_entity_directive = true;
                    
                    // Validate entity directive arguments
                    for (arg_name, _arg_value) in &directive.arguments {
                        match arg_name.as_str() {
                            "timeseries" | "immutable" => {
                                // Valid boolean arguments
                            }
                            _ => {
                                self.warnings.push(format!(
                                    "Unknown argument '{}' in @entity directive on '{}'",
                                    arg_name, entity.name
                                ));
                            }
                        }
                    }
                }
                _ => {
                    self.warnings.push(format!(
                        "Unknown directive '@{}' on entity '{}'",
                        directive.name, entity.name
                    ));
                }
            }
        }

        if !has_entity_directive {
            self.errors.push(format!("Type '{}' must have @entity directive to be an entity", entity.name));
        }
    }

    /// Validate timeseries-specific rules
    fn validate_timeseries_entity(&mut self, entity: &EntityType) {
        // Timeseries entities should be immutable by design
        if !entity.is_immutable() {
            self.warnings.push(format!(
                "Timeseries entity '{}' should consider being immutable (@entity(timeseries: true, immutable: true))",
                entity.name
            ));
        }

        // Check for fields that might not make sense in timeseries
        for (field_name, field) in &entity.fields {
            if field.has_directive("unique") && field_name != "id" {
                self.warnings.push(format!(
                    "Unique field '{}' in timeseries entity '{}' may not be necessary",
                    field_name, entity.name
                ));
            }
        }
    }

    /// Validate immutable-specific rules
    fn validate_immutable_entity(&mut self, entity: &EntityType) {
        // Just a placeholder for now - could add specific immutable validation rules
        if entity.fields.len() < 2 {
            self.warnings.push(format!(
                "Immutable entity '{}' has very few fields - consider if this is intentional",
                entity.name
            ));
        }
    }

    /// Validate cross-entity relationships
    fn validate_relationships(&mut self, schema: &EntitySchema) {
        let mut referenced_types = HashSet::new();

        // Collect all referenced types
        for entity in schema.entities.values() {
            for field in entity.fields.values() {
                if let Some(type_name) = field.base_type().get_object_name() {
                    referenced_types.insert(type_name);
                }
            }
        }

        // Check if all referenced types exist
        for type_name in &referenced_types {
            if !schema.is_entity(type_name) {
                self.errors.push(format!("Referenced type '{}' is not defined as an entity", type_name));
            }
        }

        // Check for circular references (could be valid but worth noting)
        self.detect_circular_references(schema);
    }

    /// Detect circular references in entity relationships
    fn detect_circular_references(&mut self, schema: &EntitySchema) {
        for (entity_name, entity) in &schema.entities {
            let mut visited = HashSet::new();
            let mut path = Vec::new();
            
            if self.has_circular_reference(entity_name, schema, &mut visited, &mut path) {
                self.warnings.push(format!(
                    "Circular reference detected involving entity '{}': {}",
                    entity_name,
                    path.join(" -> ")
                ));
            }
        }
    }

    /// Check if an entity has circular references
    fn has_circular_reference(
        &self,
        entity_name: &str,
        schema: &EntitySchema,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>
    ) -> bool {
        if visited.contains(entity_name) {
            return true;
        }

        visited.insert(entity_name.to_string());
        path.push(entity_name.to_string());

        if let Some(entity) = schema.get_entity(entity_name) {
            for field in entity.fields.values() {
                if let Some(referenced_type) = field.base_type().get_object_name() {
                    if schema.is_entity(referenced_type) {
                        if self.has_circular_reference(referenced_type, schema, visited, path) {
                            return true;
                        }
                    }
                }
            }
        }

        visited.remove(entity_name);
        path.pop();
        false
    }

    /// Helper: Check if field type is ID!
    fn is_id_non_null(&self, field_type: &FieldType) -> bool {
        matches!(field_type, FieldType::NonNull(inner) if matches!(**inner, FieldType::Scalar(crate::entity::types::ScalarType::ID)))
    }

    /// Helper: Check if field type is Int8!
    fn is_int8_non_null(&self, field_type: &FieldType) -> bool {
        matches!(field_type, FieldType::NonNull(inner) if matches!(**inner, FieldType::Scalar(crate::entity::types::ScalarType::Int8)))
    }

    /// Helper: Check if field type is Timestamp!
    fn is_timestamp_non_null(&self, field_type: &FieldType) -> bool {
        matches!(field_type, FieldType::NonNull(inner) if matches!(**inner, FieldType::Scalar(crate::entity::types::ScalarType::Timestamp)))
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of schema validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Validation errors that must be fixed
    pub errors: Vec<String>,
    /// Validation warnings (suggestions for improvement)
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Check if validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get total number of issues (errors + warnings)
    pub fn issue_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }

    /// Print all validation results
    pub fn print_results(&self) {
        for error in &self.errors {
            println!("❌ Error: {}", error);
        }
        for warning in &self.warnings {
            println!("⚠️  Warning: {}", warning);
        }

        if self.is_valid() && self.warnings.is_empty() {
            println!("✅ Schema validation passed with no issues");
        } else if self.is_valid() {
            println!("✅ Schema validation passed with {} warning(s)", self.warnings.len());
        } else {
            println!("❌ Schema validation failed with {} error(s) and {} warning(s)", 
                    self.errors.len(), self.warnings.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::schema::parser::SchemaParser;

    #[test]
    fn test_validate_valid_schema() {
        let schema_str = r#"
            type User @entity {
                id: ID!
                name: String!
                email: String! @unique
            }
        "#;

        let parser = SchemaParser::new();
        let schema = parser.parse_schema(schema_str).unwrap();
        
        let mut validator = SchemaValidator::new();
        let result = validator.validate(&schema).unwrap();
        
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_missing_id_field() {
        let schema_str = r#"
            type User @entity {
                name: String!
            }
        "#;

        let parser = SchemaParser::new();
        let result = parser.parse_schema(schema_str);
        
        // The parser should fail during validation because of missing id field
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must have an 'id' field"));
    }

    #[test]
    fn test_validate_circular_reference() {
        let schema_str = r#"
            type User @entity {
                id: ID!
                profile: Profile!
            }

            type Profile @entity {
                id: ID!
                user: User!
            }
        "#;

        let parser = SchemaParser::new();
        let schema = parser.parse_schema(schema_str).unwrap();
        
        let mut validator = SchemaValidator::new();
        let result = validator.validate(&schema).unwrap();
        
        // Should have warnings about circular references
        assert!(!result.warnings.is_empty());
        assert!(result.warnings.iter().any(|w| w.contains("Circular reference")));
    }

    #[test]
    fn test_validate_derived_from() {
        let schema_str = r#"
            type User @entity {
                id: ID!
                transactions: [Transaction!]! @derivedFrom(field: "user")
            }

            type Transaction @entity {
                id: ID!
                user: User!
            }
        "#;

        let parser = SchemaParser::new();
        let schema = parser.parse_schema(schema_str).unwrap();
        
        let mut validator = SchemaValidator::new();
        let result = validator.validate(&schema).unwrap();
        
        assert!(result.is_valid());
    }
}