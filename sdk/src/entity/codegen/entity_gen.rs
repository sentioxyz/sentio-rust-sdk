//! Entity struct code generator using the rust-codegen crate for robust code generation

use crate::entity::schema::{EntitySchema, EntityType, FieldDefinition, FieldType};
use crate::entity::schema::parser::SchemaParser;
use crate::codegen::{CodeGenerator, CodegenResult};
use anyhow::{Result, Context};
use rust_codegen::{Scope, Function, Impl, Struct, Field, Type};
use std::path::Path;
use std::fs;
use convert_case::{Case, Casing};

/// Generator for entity struct code using the rust-codegen crate
pub struct EntityCodeGenerator;

impl EntityCodeGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Convert camelCase or PascalCase field name to snake_case for Rust conventions
    fn to_snake_case(&self, field_name: &str) -> String {
        field_name.to_case(Case::Snake)
    }

    /// Generate Rust code for an entity using the rust-codegen crate
    pub fn generate_entity(&self, entity: &EntityType, schema: &EntitySchema) -> Result<String> {
        let mut header = Scope::new();
        header.raw("#![allow(non_snake_case)]");
        // Add file header comment
        header.raw(&format!("//! Generated entity: {}", entity.name));
        header.raw("// This file is auto-generated. Do not edit manually.");
        header.raw("");

        let mut scope = Scope::new();
        // Add imports
        self.add_imports(&mut scope, entity, schema);

        // Generate the main entity struct
        self.generate_entity_struct(&mut scope, entity, schema)?;

        // Generate Entity trait implementation
        self.generate_entity_trait_impl(&mut scope, entity)?;

        // Generate helper methods implementation
        self.generate_helper_impl(&mut scope, entity, schema)?;

        Ok(format!("{}{}",header.to_string(), scope.to_string()))
    }

    /// Add necessary imports to the scope
    fn add_imports(&self, scope: &mut Scope, entity: &EntityType, schema: &EntitySchema) {
        scope.import("sentio_sdk::entity", "*");
        scope.import("derive_builder", "Builder");
        scope.import("serde", "{Serialize, Deserialize}");
        
        // Add imports for derived field reference types
        for (_, field) in entity.get_derived_fields() {
            if let Some(target_type) = field.base_type().get_object_name() {
                // Import the referenced entity type from the entities module
                scope.import("crate::entities", target_type);
            }
        }
        
        scope.raw(""); // Empty line after imports
    }

    /// Generate the main entity struct
    fn generate_entity_struct(&self, scope: &mut Scope, entity: &EntityType, schema: &EntitySchema) -> Result<()> {
        let mut entity_struct = Struct::new(&entity.name);
        
        // Add documentation
        if let Some(ref description) = entity.description {
            entity_struct.doc(description);
        } else {
            entity_struct.doc(&format!("Entity: {}", entity.name));
        }

        // Add derive macros
        entity_struct.derive("Debug").derive("Clone").derive("PartialEq").derive("Serialize").derive("Deserialize").derive("Builder");
        
        // Configure builder to use EntityError
        // entity_struct.attr("builder(build_fn(error = \"EntityError\"))");
        
        // Add additional documentation for special entity types
        if entity.is_timeseries() {
            entity_struct.doc("Timeseries entity - optimized for time-ordered data");
        }
        if entity.is_immutable() {
            entity_struct.doc("Immutable entity - data cannot be updated after creation");
        }

        // Add fields (skip derived fields)
        for (field_name, field) in &entity.fields {
            if field.has_directive("derivedFrom") {
                continue; // Skip derived fields - they'll be methods
            }

            let field_type = self.field_type_to_rust(&field.field_type, schema, field_name == "id")?;
            let rust_field_name = self.to_snake_case(field_name);
            
            // Only add serde rename annotation if the field name actually changed
            let annotations = if rust_field_name != *field_name {
                vec![format!("#[serde(rename = \"{}\")]", field_name)]
            } else {
                vec![]
            };
            
            let f = Field {
                name: rust_field_name.clone(),
                ty: Type::new(&field_type),
                documentation: vec![],
                annotation: annotations,
            };

            let struct_field = entity_struct.push_field(f);
            struct_field.vis("pub");

            // Add field documentation
            if let Some(ref description) = field.description {
                struct_field.doc(description);
            }

            if field.is_indexed() {
                struct_field.doc("Indexed field");
            }
        }

        scope.push_struct(entity_struct);
        scope.raw(""); // Empty line after struct
        Ok(())
    }

    /// Generate Entity trait implementation
    fn generate_entity_trait_impl(&self, scope: &mut Scope, entity: &EntityType) -> Result<()> {
        // Determine ID type
        let id_type = if entity.is_timeseries() { "i64" } else { "ID" };

        // Add TABLE_NAME constant manually since rust-codegen doesn't support associate_const
        // We need to add it inside the impl block as raw content
        let impl_code = format!(
            "impl Entity for {} {{\n    type Id = {};\n    const TABLE_NAME: &'static str = \"{}\";\n\n    fn id(&self) -> &Self::Id {{\n        &self.id\n    }}\n}}", 
            entity.name, 
            id_type,
            entity.name.to_lowercase()
        );
        
        scope.raw(&impl_code);
        scope.raw("");
        Ok(())
    }

    /// Generate helper methods implementation
    fn generate_helper_impl(&self, scope: &mut Scope, entity: &EntityType, schema: &EntitySchema) -> Result<()> {
        let mut helper_impl = Impl::new(&entity.name);


        // Relation getters for derived fields
        for (field_name, field) in entity.get_derived_fields() {
            self.add_derived_field_getter(&mut helper_impl, field_name, field, schema)?;
        }

        // Relation setters for object fields
        for (field_name, field) in entity.get_relation_fields() {
            if !field.has_directive("derivedFrom") {
                self.add_relation_setter(&mut helper_impl, field_name, field)?;
            }
        }

        // Store operation convenience methods
        self.add_store_operations(&mut helper_impl, entity)?;

        scope.push_impl(helper_impl);
        Ok(())
    }

    /// Add derived field getter method
    fn add_derived_field_getter(
        &self,
        impl_block: &mut Impl,
        field_name: &str,
        field: &FieldDefinition,
        _schema: &EntitySchema
    ) -> Result<()> {
        if let Some(target_type) = field.base_type().get_object_name() {
            let mut getter = Function::new(field_name);
            getter.doc(&format!("Get {} (derived relation)", field_name))
                  .vis("pub")
                  .set_async(true)
                  .arg_ref_self()
                  .arg("store", "Store")
                  .ret(&format!("EntityResult<Vec<{}>>", target_type))
                  .line("// TODO: Implement derived field query")
                  .line("todo!(\"Derived field queries not yet implemented\")");

            impl_block.push_fn(getter);
        }
        Ok(())
    }

    /// Add relation setter method
    fn add_relation_setter(
        &self,
        impl_block: &mut Impl,
        field_name: &str,
        field: &FieldDefinition,
    ) -> Result<()> {
        if let Some(_target_type) = field.base_type().get_object_name() {
            let field_type = self.field_type_to_rust(&field.field_type, &EntitySchema::new(), false)?;
            let rust_field_name = self.to_snake_case(field_name);
            
            let mut setter = Function::new(&format!("set_{}", rust_field_name));
            setter.doc(&format!("Set {} relation", field_name))
                  .vis("pub")
                  .arg_mut_self()
                  .arg(&rust_field_name, &field_type)
                  .line(&format!("self.{} = {};", rust_field_name, rust_field_name));

            impl_block.push_fn(setter);
        }
        Ok(())
    }

    /// Add store operation convenience methods
    fn add_store_operations(&self, _impl_block: &mut Impl, _entity: &EntityType) -> Result<()> {
        // // save method
        // let mut save_fn = Function::new("save");
        // save_fn.doc("Save this entity to the store")
        //        .vis("pub")
        //        .set_async(true)
        //        .arg_ref_self()
        //        .arg("store", "&dyn EntityStore")
        //        .ret("EntityResult<()>")
        //        .line("store.upsert(self).await");
        // impl_block.push_fn(save_fn);
        //
        // // load method (static)
        // let mut load_fn = Function::new("load");
        // load_fn.doc("Load an entity from the store by ID")
        //        .vis("pub")
        //        .set_async(true)
        //        .arg("id", &format!("&<{} as Entity>::Id", entity.name))
        //        .arg("store", "&dyn EntityStore")
        //        .ret(&format!("EntityResult<Option<{}>>", entity.name))
        //        .line("store.get(id).await");
        // impl_block.push_fn(load_fn);
        //
        // // delete method
        // let mut delete_fn = Function::new("delete");
        // delete_fn.doc("Delete this entity from the store")
        //          .vis("pub")
        //          .set_async(true)
        //          .arg_ref_self()
        //          .arg("store", "&dyn EntityStore")
        //          .ret("EntityResult<()>")
        //          .line("store.delete(&self.id).await");
        // impl_block.push_fn(delete_fn);

        Ok(())
    }

    /// Convert FieldType to Rust type string
    fn field_type_to_rust(&self, field_type: &FieldType, schema: &EntitySchema, _is_id_field: bool) -> Result<String> {
        match field_type {
            FieldType::Scalar(scalar) => Ok(scalar.rust_type().to_string()),
            FieldType::Object(type_name) => {
                if schema.is_entity(type_name) {
                    Ok(type_name.clone())
                } else {
                    Ok(format!("UnknownType<{}>", type_name))
                }
            }
            FieldType::NonNull(inner) => self.field_type_to_rust(inner, schema, false),
            FieldType::List(inner) => {
                let inner_type = self.field_type_to_rust(inner, schema, false)?;
                Ok(format!("Vec<{}>", inner_type))
            }
        }
    }
    
    /// Generate mod.rs for entities module
    fn generate_entities_mod(&self, schema: &EntitySchema) -> Result<String> {
        let mut content = String::new();
        content.push_str("//! Generated entities module\n");
        content.push_str("//!\n");
        content.push_str("// This file is auto-generated. Do not edit manually.\n\n");
        
        // Add module declarations
        let entities: Vec<_> = schema.get_entities().map(|(name, _)| name).collect();
        for entity_name in &entities {
            content.push_str(&format!("pub mod {};\n", entity_name.to_lowercase()));
        }
        content.push('\n');
        
        // Add re-exports
        for entity_name in entities {
            content.push_str(&format!("pub use {}::{};\n", entity_name.to_lowercase(), entity_name));
        }
        
        Ok(content)
    }
}

impl Default for EntityCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGenerator for EntityCodeGenerator {
    fn generator_name(&self) -> &str {
        "entity"
    }
    
    fn should_generate(&self, src_dir: &Path) -> bool {
        src_dir.join("schema.graphql").exists()
    }
    
    fn generate(&self, src_dir: &Path, dst_dir: &Path) -> Result<CodegenResult> {
        let schema_path = src_dir.join("schema.graphql");
        
        // Check if schema file exists
        if !schema_path.exists() {
            return Ok(CodegenResult {
                generator_name: self.generator_name().to_string(),
                files_generated: vec![],
                success: false,
                message: format!("Schema file not found: {}", schema_path.display()),
            });
        }
        
        // Read and parse schema
        let schema_content = fs::read_to_string(&schema_path)
            .with_context(|| format!("Failed to read schema file: {}", schema_path.display()))?;
            
        let parser = SchemaParser::new();
        let schema = parser.parse_schema(&schema_content)
            .with_context(|| "Failed to parse GraphQL schema")?;
        
        let mut generated_files = Vec::new();
        
        // Create entities output directory
        let entities_dir = dst_dir.join("entities");
        if !entities_dir.exists() {
            fs::create_dir_all(&entities_dir)
                .with_context(|| format!("Failed to create entities directory: {}", entities_dir.display()))?;
        }
        
        // Generate code for each entity
        for (entity_name, entity) in schema.get_entities() {
            let code = self.generate_entity(entity, &schema)
                .with_context(|| format!("Failed to generate code for entity: {}", entity_name))?;
                
            let file_name = format!("{}.rs", entity_name.to_lowercase());
            let output_path = entities_dir.join(&file_name);
            
            fs::write(&output_path, code)
                .with_context(|| format!("Failed to write entity file: {}", output_path.display()))?;
                
            generated_files.push(output_path);
        }
        
        // Generate mod.rs file for entities module
        let mod_content = self.generate_entities_mod(&schema)?;
        let mod_path = entities_dir.join("mod.rs");
        fs::write(&mod_path, mod_content)
            .with_context(|| format!("Failed to write entities mod file: {}", mod_path.display()))?;
        generated_files.push(mod_path);
        
        let entity_count = schema.get_entities().count();
        Ok(CodegenResult {
            generator_name: self.generator_name().to_string(),
            files_generated: generated_files,
            success: true,
            message: format!("Generated {} entities", entity_count),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::schema::types::*;
    use crate::entity::types::ScalarType;

    #[test]
    fn test_generate_simple_entity() {
        let mut entity = EntityType::new("User".to_string());
        entity.description = Some("A user entity".to_string());
        
        // Add id field
        let id_field = FieldDefinition::new("id".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))));
        entity.add_field("id".to_string(), id_field);

        // Add name field
        let name_field = FieldDefinition::new("name".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::String))));
        entity.add_field("name".to_string(), name_field);

        // Add entity directive
        let entity_directive = Directive::new("entity".to_string());
        entity.add_directive(entity_directive);

        let generator = EntityCodeGenerator::new();
        let schema = EntitySchema::new();
        let code = generator.generate_entity(&entity, &schema).unwrap();

        // Test that the code contains the expected elements (format-agnostic)
        assert!(code.contains("struct User"));
        assert!(code.contains("id: ID") || code.contains("id :ID"));
        assert!(code.contains("name: String") || code.contains("name :String"));
        assert!(code.contains("impl Entity for User"));
        assert!(code.contains("TABLE_NAME") && (code.contains("users") || code.contains("user")));
    }

    #[test]
    fn test_generate_timeseries_entity() {
        let mut entity = EntityType::new("Transaction".to_string());
        
        // Add id field (Int8 for timeseries)
        let id_field = FieldDefinition::new("id".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::Int8))));
        entity.add_field("id".to_string(), id_field);

        // Add timestamp field
        let timestamp_field = FieldDefinition::new("timestamp".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::Timestamp))));
        entity.add_field("timestamp".to_string(), timestamp_field);

        // Add entity directive with timeseries=true
        let mut entity_directive = Directive::new("entity".to_string());
        entity_directive.add_argument("timeseries".to_string(), DirectiveArg::Boolean(true));
        entity.add_directive(entity_directive);

        let generator = EntityCodeGenerator::new();
        let schema = EntitySchema::new();
        let code = generator.generate_entity(&entity, &schema).unwrap();

        // Test that the code contains the expected elements (format-agnostic)
        assert!(code.contains("Timeseries entity"));
        assert!(code.contains("struct Transaction"));
        assert!(code.contains("id: Int8") || code.contains("id :Int8"));
        assert!(code.contains("timestamp: Timestamp") || code.contains("timestamp :Timestamp"));
        assert!(code.contains("type Id = i64") || code.contains("Id = i64"));
    }

    #[test]
    fn test_generate_entity_with_bigint() {
        let mut entity = EntityType::new("TokenBalance".to_string());
        entity.description = Some("Token balance entity with big integer amounts".to_string());
        
        // Add id field
        let id_field = FieldDefinition::new("id".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))));
        entity.add_field("id".to_string(), id_field);

        // Add balance field with BigInt type
        let balance_field = FieldDefinition::new("balance".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::BigInt))));
        entity.add_field("balance".to_string(), balance_field);

        // Add address field
        let address_field = FieldDefinition::new("address".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::String))));
        entity.add_field("address".to_string(), address_field);

        // Add entity directive
        let entity_directive = Directive::new("entity".to_string());
        entity.add_directive(entity_directive);

        let generator = EntityCodeGenerator::new();
        let schema = EntitySchema::new();
        let code = generator.generate_entity(&entity, &schema).unwrap();

        // Test that the code contains the expected elements (format-agnostic)
        assert!(code.contains("struct TokenBalance"));
        assert!(code.contains("id: ID") || code.contains("id :ID"));
        assert!(code.contains("balance: BigInt") || code.contains("balance :BigInt"));
        assert!(code.contains("address: String") || code.contains("address :String"));
        assert!(code.contains("impl Entity for TokenBalance"));
        assert!(code.contains("TABLE_NAME"));

        println!("Generated code:\n{}", code);
    }

    #[test]
    fn test_snake_case_conversion() {
        let generator = EntityCodeGenerator::new();
        
        // Test various camelCase to snake_case conversions
        assert_eq!(generator.to_snake_case("transactionHash"), "transaction_hash");
        assert_eq!(generator.to_snake_case("blockNumber"), "block_number");
        assert_eq!(generator.to_snake_case("firstSeen"), "first_seen");
        assert_eq!(generator.to_snake_case("lastActive"), "last_active");
        assert_eq!(generator.to_snake_case("totalSupply"), "total_supply");
        assert_eq!(generator.to_snake_case("transferCount"), "transfer_count");
        assert_eq!(generator.to_snake_case("id"), "id"); // Already snake_case
        assert_eq!(generator.to_snake_case("timestamp"), "timestamp"); // Already snake_case
        assert_eq!(generator.to_snake_case("APIKey"), "api_key"); // Multiple capitals
    }

    #[test]
    fn test_generate_entity_with_camel_case_fields() {
        let mut entity = EntityType::new("Transfer".to_string());
        entity.description = Some("Transfer entity with camelCase field names".to_string());
        
        // Add id field (already snake_case)
        let id_field = FieldDefinition::new("id".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))));
        entity.add_field("id".to_string(), id_field);

        // Add camelCase fields
        let tx_hash_field = FieldDefinition::new("transactionHash".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::String))));
        entity.add_field("transactionHash".to_string(), tx_hash_field);

        let block_num_field = FieldDefinition::new("blockNumber".to_string(), 
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::BigInt))));
        entity.add_field("blockNumber".to_string(), block_num_field);

        // Add entity directive
        let entity_directive = Directive::new("entity".to_string());
        entity.add_directive(entity_directive);

        let generator = EntityCodeGenerator::new();
        let schema = EntitySchema::new();
        let code = generator.generate_entity(&entity, &schema).unwrap();

        // Test that the code contains snake_case field names
        assert!(code.contains("transaction_hash: String"));
        assert!(code.contains("block_number: BigInt"));
        
        // Test that serde rename attributes are present
        assert!(code.contains("serde(rename = \"transactionHash\")"));
        assert!(code.contains("serde(rename = \"blockNumber\")"));
        
        // Test that id field doesn't have serde rename (since it's already snake_case)
        assert!(!code.contains("serde(rename = \"id\")"));

        println!("Generated camelCase entity code:\n{}", code);
    }
}