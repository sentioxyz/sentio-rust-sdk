//! Entity struct code generator using the rust-codegen crate for robust code generation

use crate::codegen::{CodeGenerator, CodegenResult};
use crate::entity::schema::parser::SchemaParser;
use crate::entity::schema::{EntitySchema, EntityType, FieldDefinition, FieldType};
use anyhow::{Context, Result};
use convert_case::{Case, Casing};
use rust_codegen::{Field, Function, Impl, Scope, Struct, Type};
use std::fs;
use std::path::Path;

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

        Ok(format!("{}{}", header.to_string(), scope.to_string()))
    }

    /// Add necessary imports to the scope
    fn add_imports(&self, scope: &mut Scope, entity: &EntityType, _schema: &EntitySchema) {
        scope.import("sentio_sdk::entity", "*");
        scope.import("derive_builder", "Builder");
        scope.import("serde", "{Serialize, Deserialize}");

        // Add imports for all relation field reference types (both derived and direct)
        for (_, field) in entity.get_derived_fields() {
            if let Some(target_type) = field.base_type().get_object_name() {
                // Import the referenced entity type from the entities module
                scope.import("crate::entities", target_type);
            }
        }

        // Add imports for direct relation fields too
        for (_, field) in entity.get_relation_fields() {
            if !field.has_directive("derivedFrom")
                && let Some(target_type) = field.base_type().get_object_name()
            {
                // Import the referenced entity type from the entities module
                scope.import("crate::entities", target_type);
            }
        }

        scope.raw(""); // Empty line after imports
    }

    /// Generate the main entity struct
    fn generate_entity_struct(
        &self,
        scope: &mut Scope,
        entity: &EntityType,
        schema: &EntitySchema,
    ) -> Result<()> {
        let mut entity_struct = Struct::new(&entity.name);

        // Add documentation
        if let Some(ref description) = entity.description {
            entity_struct.doc(description);
        } else {
            entity_struct.doc(&format!("Entity: {}", entity.name));
        }

        entity_struct.vis("pub");
        // Add derive macros
        entity_struct
            .derive("Debug")
            .derive("Clone")
            .derive("PartialEq")
            .derive("Serialize")
            .derive("Deserialize")
            .derive("Builder");

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
                continue; // Skip derived fields - they'll be methods only
            }

            // Special handling for direct relation fields
            if field.is_relation()
                && let Some(_target_type) = field.base_type().get_object_name()
            {
                let base_name = self.to_snake_case(field_name);
                let is_list = field.field_type.is_list();
                let is_optional = field.field_type.is_optional();

                // Generate field name and type based on relation cardinality
                let (field_name_final, ty_string) = if is_list {
                    (format!("{}_ids", base_name), "Vec<ID>".to_string())
                } else if is_optional {
                    (format!("{}_id", base_name), "Option<ID>".to_string())
                } else {
                    (format!("{}_id", base_name), "ID".to_string())
                };

                let mut annotations = vec![format!("#[serde(rename = \"{}\")]", field_name)];
                if is_optional {
                    annotations.push("#[builder(default)]".to_string());
                }

                let mut f = Field {
                    name: format!("pub {}", field_name_final.clone()),
                    ty: Type::new(&ty_string),
                    documentation: vec![],
                    annotation: annotations,
                };

                // Add field documentation
                if let Some(ref description) = field.description {
                    f.documentation.push(description.to_string());
                }

                if field.is_indexed() {
                    f.documentation.push("Indexed field".to_string());
                }

                f.documentation.push("Relation field".to_string());
                entity_struct.push_field(f);
                continue;
            }

            // Non-relation fields fall back to regular processing
            let field_type =
                self.field_type_to_rust(&field.field_type, schema, field_name == "id")?;

            let rust_field_name = self.to_snake_case(field_name);
            let is_optional = field.field_type.is_optional();

            // Build annotations vector with serde rename (if needed) and builder default for optional fields
            let mut annotations = Vec::new();
            if rust_field_name != *field_name {
                annotations.push(format!("#[serde(rename = \"{}\")]", field_name));
            }
            if is_optional {
                annotations.push("#[builder(default)]".to_string());
            }

            let f = Field {
                name: format!("pub {}", rust_field_name.clone()),
                ty: Type::new(&field_type),
                documentation: vec![],
                annotation: annotations,
            };

            let struct_field = entity_struct.push_field(f);

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

        // Add NAME constant manually since rust-codegen doesn't support associate_const
        // We need to add it inside the impl block as raw content
        let impl_code = format!(
            "impl Entity for {} {{\n    type Id = {};\n    const NAME: &'static str = \"{}\";\n\n    fn id(&self) -> &Self::Id {{\n        &self.id\n    }}\n}}",
            entity.name, id_type, entity.name
        );

        scope.raw(&impl_code);
        scope.raw("");
        Ok(())
    }

    /// Generate helper methods implementation
    fn generate_helper_impl(
        &self,
        scope: &mut Scope,
        entity: &EntityType,
        schema: &EntitySchema,
    ) -> Result<()> {
        let mut helper_impl = Impl::new(&entity.name);

        // Relation getters for derived fields
        for (field_name, field) in entity.get_derived_fields() {
            self.add_derived_field_getter(&mut helper_impl, field_name, field, schema)?;
        }

        // Relation getters for direct relations
        for (field_name, field) in entity.get_relation_fields() {
            if !field.has_directive("derivedFrom") {
                self.add_direct_relation_getter(&mut helper_impl, field_name, field)?;
            }
        }

        scope.push_impl(helper_impl);
        Ok(())
    }

    /// Add derived field getter method
    fn add_derived_field_getter(
        &self,
        impl_block: &mut Impl,
        field_name: &str,
        field: &FieldDefinition,
        _schema: &EntitySchema,
    ) -> Result<()> {
        if let Some(target_type) = field.base_type().get_object_name() {
            // Determine if this is a list or single relation
            let is_list = field.field_type.is_list();
            let return_type = if is_list {
                format!("EntityResult<Vec<{}>>", target_type)
            } else {
                format!("EntityResult<Option<{}>>", target_type)
            };

            let rust_field_name = self.to_snake_case(field_name);
            let mut getter = Function::new(&rust_field_name);
            getter
                .doc(&format!("Get {} (derived relation)", field_name))
                .vis("pub")
                .set_async(true)
                .arg_ref_self()
                .ret(&return_type);

            // Generate query implementation based on @derivedFrom directive
            if let Some(derived_directive) = field.get_directive("derivedFrom") {
                if let Some(derived_field) = derived_directive.get_string_arg("field") {
                    if is_list {
                        // Many relations derived field (case 2) - using Entity Query API
                        getter.line(format!(
                            "Ok({}::find().where_eq(\"{}\", self.id.clone()).list().await?)",
                            target_type, derived_field
                        ));
                    } else {
                        // Single relation derived field (case 4) - using Entity Query API
                        getter.line(format!(
                            "Ok({}::find().where_eq(\"{}\", self.id.clone()).first().await?)",
                            target_type, derived_field
                        ));
                    }
                } else {
                    getter.line("// TODO: derivedFrom field argument missing")
                       .line("Err(EntityError::InvalidQuery(\"derivedFrom field missing\".to_string()))");
                }
            } else {
                getter.line("// TODO: derivedFrom directive missing").line(
                    "Err(EntityError::InvalidQuery(\"derivedFrom directive missing\".to_string()))",
                );
            }

            impl_block.push_fn(getter);
        }
        Ok(())
    }

    /// Add direct relation getter method for non-derived relations
    fn add_direct_relation_getter(
        &self,
        impl_block: &mut Impl,
        field_name: &str,
        field: &FieldDefinition,
    ) -> Result<()> {
        if let Some(target_type) = field.base_type().get_object_name() {
            let is_list = field.field_type.is_list();
            let is_optional = field.field_type.is_optional();
            let rust_field_name = self.to_snake_case(field_name);

            if is_list {
                // Many relations: generate get_many helper
                let mut getter = Function::new(&rust_field_name);
                getter
                    .doc(&format!("Get {} relation", field_name))
                    .vis("pub")
                    .set_async(true)
                    .arg_ref_self()
                    .ret(format!("EntityResult<Vec<{}>>", target_type));

                getter.line(format!(
                    "let ids = self.{0}_ids.iter().map(|id| <{1} as Entity>::Id::from_string(&id.to_string())).collect::<Result<Vec<_>, _>>()?;",
                    rust_field_name, target_type
                ));
                getter.line(format!("Ok({}::get_many(&ids).await?)", target_type));
                impl_block.push_fn(getter);
            } else {
                // Single relation: generate get helper
                let mut getter = Function::new(&rust_field_name);
                getter
                    .doc(&format!("Get {} relation", field_name))
                    .vis("pub")
                    .set_async(true)
                    .arg_ref_self()
                    .ret(format!("EntityResult<Option<{}>>", target_type));

                if is_optional {
                    getter.line(format!(
                        "if let Some(id) = &self.{0}_id {{",
                        rust_field_name
                    ));
                    getter.line(format!(
                        "    let id = <{} as Entity>::Id::from_string(&id.to_string())?;",
                        target_type
                    ));
                    getter.line(format!("    Ok({}::get(&id).await?)", target_type));
                    getter.line("} else {");
                    getter.line("    Ok(None)");
                    getter.line("}");
                } else {
                    getter.line(format!(
                        "let id = <{1} as Entity>::Id::from_string(&self.{0}_id.to_string())?;",
                        rust_field_name, target_type
                    ));
                    getter.line(format!("Ok({}::get(&id).await?)", target_type));
                }

                impl_block.push_fn(getter);
            }
        }
        Ok(())
    }

    /// Convert FieldType to Rust type string
    fn field_type_to_rust(
        &self,
        field_type: &FieldType,
        schema: &EntitySchema,
        _is_id_field: bool,
    ) -> Result<String> {
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

    /// Generate all entities in a single entities.rs file
    fn generate_all_entities(&self, schema: &EntitySchema) -> Result<String> {
        let mut content = String::new();

        // File header (no inner attributes for include! compatibility)
        content.push_str("// Generated entities\n");
        content.push_str("// This file is auto-generated. Do not edit manually.\n\n");

        // Add common imports
        content.push_str("use sentio_sdk::entity::*;\n");
        content.push_str("use derive_builder::Builder;\n");
        content.push_str("use serde::{Serialize, Deserialize};\n\n");

        // Generate each entity
        for (entity_name, entity) in schema.get_entities() {
            let entity_code = self
                .generate_entity_struct_only(entity, schema)
                .with_context(|| format!("Failed to generate entity: {}", entity_name))?;
            content.push_str(&entity_code);
            content.push_str("\n\n");
        }

        Ok(content)
    }

    /// Generate just the entity struct and implementations without imports/header
    fn generate_entity_struct_only(
        &self,
        entity: &EntityType,
        schema: &EntitySchema,
    ) -> Result<String> {
        let mut scope = Scope::new();

        // Generate the main entity struct
        self.generate_entity_struct(&mut scope, entity, schema)?;

        // Generate Entity trait implementation
        self.generate_entity_trait_impl(&mut scope, entity)?;

        // Generate helper methods implementation
        self.generate_helper_impl(&mut scope, entity, schema)?;

        Ok(scope.to_string())
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
        let schema = parser
            .parse_schema(&schema_content)
            .with_context(|| "Failed to parse GraphQL schema")?;

        let mut generated_files = Vec::new();

        let output_dir = dst_dir;

        // Ensure output directory exists
        if !output_dir.exists() {
            fs::create_dir_all(output_dir).with_context(|| {
                format!(
                    "Failed to create output directory: {}",
                    output_dir.display()
                )
            })?;
        }

        // Generate all entities in a single entities.rs file
        let entities_content = self.generate_all_entities(&schema)?;
        let entities_path = output_dir.join("entities.rs");
        fs::write(&entities_path, entities_content).with_context(|| {
            format!("Failed to write entities file: {}", entities_path.display())
        })?;
        generated_files.push(entities_path);

        let entity_count = schema.get_entities().count();
        Ok(CodegenResult {
            generator_name: self.generator_name().to_string(),
            files_generated: generated_files,
            success: true,
            message: format!("Generated {} entities in entities.rs", entity_count),
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
        let id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))),
        );
        entity.add_field("id".to_string(), id_field);

        // Add name field
        let name_field = FieldDefinition::new(
            "name".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::String))),
        );
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
        assert!(code.contains("NAME") && code.contains("User"));
    }

    #[test]
    fn test_generate_timeseries_entity() {
        let mut entity = EntityType::new("Transaction".to_string());

        // Add id field (Int8 for timeseries)
        let id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::Int8))),
        );
        entity.add_field("id".to_string(), id_field);

        // Add timestamp field
        let timestamp_field = FieldDefinition::new(
            "timestamp".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::Timestamp))),
        );
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
        let id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))),
        );
        entity.add_field("id".to_string(), id_field);

        // Add balance field with BigInt type
        let balance_field = FieldDefinition::new(
            "balance".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::BigInt))),
        );
        entity.add_field("balance".to_string(), balance_field);

        // Add address field
        let address_field = FieldDefinition::new(
            "address".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::String))),
        );
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
        assert!(code.contains("NAME"));

        println!("Generated code:\n{}", code);
    }

    #[test]
    fn test_snake_case_conversion() {
        let generator = EntityCodeGenerator::new();

        // Test various camelCase to snake_case conversions
        assert_eq!(
            generator.to_snake_case("transactionHash"),
            "transaction_hash"
        );
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
        let id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))),
        );
        entity.add_field("id".to_string(), id_field);

        // Add camelCase fields
        let tx_hash_field = FieldDefinition::new(
            "transactionHash".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::String))),
        );
        entity.add_field("transactionHash".to_string(), tx_hash_field);

        let block_num_field = FieldDefinition::new(
            "blockNumber".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::BigInt))),
        );
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

    #[test]
    fn test_generate_entity_with_relations() {
        let mut schema = EntitySchema::new();

        // Create User entity
        let mut user_entity = EntityType::new("User".to_string());
        let user_id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))),
        );
        user_entity.add_field("id".to_string(), user_id_field);
        let user_entity_directive = Directive::new("entity".to_string());
        user_entity.add_directive(user_entity_directive);
        schema.add_entity("User".to_string(), user_entity);

        // Create Post entity
        let mut post_entity = EntityType::new("Post".to_string());
        let post_id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))),
        );
        post_entity.add_field("id".to_string(), post_id_field);
        let post_entity_directive = Directive::new("entity".to_string());
        post_entity.add_directive(post_entity_directive);
        schema.add_entity("Post".to_string(), post_entity);

        // Create Account entity with all 4 relation types
        let mut account_entity = EntityType::new("Account".to_string());
        let account_id_field = FieldDefinition::new(
            "id".to_string(),
            FieldType::NonNull(Box::new(FieldType::Scalar(ScalarType::ID))),
        );
        account_entity.add_field("id".to_string(), account_id_field);

        // Case 1: Many relations field - Vec<Post> (direct relation)
        let posts_field = FieldDefinition::new(
            "posts".to_string(),
            FieldType::List(Box::new(FieldType::Object("Post".to_string()))),
        );
        account_entity.add_field("posts".to_string(), posts_field);

        // Case 2: Many relations derived field - [User!]! @derivedFrom
        let mut followers_field = FieldDefinition::new(
            "followers".to_string(),
            FieldType::NonNull(Box::new(FieldType::List(Box::new(FieldType::NonNull(
                Box::new(FieldType::Object("User".to_string())),
            ))))),
        );
        let mut derived_directive = Directive::new("derivedFrom".to_string());
        derived_directive.add_argument(
            "field".to_string(),
            DirectiveArg::String("following".to_string()),
        );
        followers_field.add_directive(derived_directive);
        account_entity.add_field("followers".to_string(), followers_field);

        // Case 3: Single relation - User (direct relation, optional)
        let single_user_field =
            FieldDefinition::new("owner".to_string(), FieldType::Object("User".to_string()));
        account_entity.add_field("owner".to_string(), single_user_field);

        // Case 4: Single relation derived - User @derivedFrom
        let mut derived_user_field =
            FieldDefinition::new("manager".to_string(), FieldType::Object("User".to_string()));
        let mut single_derived_directive = Directive::new("derivedFrom".to_string());
        single_derived_directive.add_argument(
            "field".to_string(),
            DirectiveArg::String("managedAccount".to_string()),
        );
        derived_user_field.add_directive(single_derived_directive);
        account_entity.add_field("manager".to_string(), derived_user_field);

        let account_entity_directive = Directive::new("entity".to_string());
        account_entity.add_directive(account_entity_directive);

        let generator = EntityCodeGenerator::new();
        let code = generator.generate_entity(&account_entity, &schema).unwrap();

        // Test Case 1: Many relations field with IDs
        assert!(code.contains("posts_ids: Vec<ID>"));
        assert!(code.contains("pub async fn posts"));
        assert!(code.contains("EntityResult<Vec<Post>>"));
        assert!(code.contains("get_many(&ids)"));

        // Test Case 2: Many relations derived field
        assert!(code.contains("pub async fn followers"));
        assert!(code.contains("EntityResult<Vec<User>>"));
        assert!(
            code.contains("User::find().where_eq(\"following\", self.id.clone()).list().await")
        );

        // Test Case 3: Single relation (optional) stored as ID
        assert!(code.contains("owner_id: Option<ID>"));
        assert!(code.contains("pub async fn owner"));
        assert!(code.contains("EntityResult<Option<User>>"));
        assert!(code.contains("::get(&id)"));

        // Test Case 4: Single relation derived
        assert!(code.contains("pub async fn manager"));
        assert!(code.contains("EntityResult<Option<User>>"));
        assert!(
            code.contains(
                "User::find().where_eq(\"managedAccount\", self.id.clone()).first().await"
            )
        );

        // Test imports (they might be in different format)
        assert!(code.contains("User") && (code.contains("use") || code.contains("import")));
        assert!(code.contains("Post") && (code.contains("use") || code.contains("import")));

        println!("Generated relations entity code:\n{}", code);
    }
}
