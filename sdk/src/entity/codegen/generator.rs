//! Main entity code generator

use super::{EntityCodeGenerator};
use crate::entity::schema::{EntitySchema, SchemaParser, SchemaValidator, ValidationResult};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::fs;

/// Options for code generation
#[derive(Debug, Clone)]
pub struct GenerationOptions {
    /// Output directory for generated code
    pub output_dir: PathBuf,
    /// Module name for generated entities
    pub module_name: String,
    /// Whether to generate store implementations
    pub generate_store: bool,
    /// Whether to overwrite existing files
    pub overwrite: bool,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("src/generated"),
            module_name: "entities".to_string(),
            generate_store: true,
            overwrite: true,
        }
    }
}

/// Main entity code generator
pub struct EntityGenerator {
    /// Schema parser
    parser: SchemaParser,
    /// Schema validator
    validator: SchemaValidator,
    /// Entity code generator
    entity_generator: EntityCodeGenerator,

}

impl EntityGenerator {
    /// Create a new entity generator
    pub fn new() -> Self {
        Self {
            parser: SchemaParser::new(),
            validator: SchemaValidator::new(),
            entity_generator: EntityCodeGenerator::new(),
         }
    }

    /// Generate code from a schema file
    pub async fn process_schema_file<P: AsRef<Path>>(
        &mut self,
        schema_path: P,
        options: GenerationOptions,
    ) -> Result<GenerationResult> {
        // Parse the schema
        let schema = self.parser.parse_file(&schema_path)
            .with_context(|| format!("Failed to parse schema: {}", schema_path.as_ref().display()))?;

        // Validate the schema
        let validation = self.validator.validate(&schema)?;
        if !validation.is_valid() {
            return Err(anyhow::anyhow!(
                "Schema validation failed with {} errors", 
                validation.errors.len()
            ));
        }

        self.generate_code(schema, options).await
    }

    /// Generate code from a schema string
    pub async fn process_schema_string(
        &mut self,
        schema_content: &str,
        options: GenerationOptions,
    ) -> Result<GenerationResult> {
        // Parse the schema
        let schema = self.parser.parse_schema(schema_content)?;

        // Validate the schema
        let validation = self.validator.validate(&schema)?;
        if !validation.is_valid() {
            return Err(anyhow::anyhow!(
                "Schema validation failed with {} errors", 
                validation.errors.len()
            ));
        }

        self.generate_code(schema, options).await
    }

    /// Generate Rust code from a validated schema
    async fn generate_code(
        &mut self,
        schema: EntitySchema,
        options: GenerationOptions,
    ) -> Result<GenerationResult> {
        let mut result = GenerationResult::new();

        // Create output directory if it doesn't exist
        if !options.output_dir.exists() {
            fs::create_dir_all(&options.output_dir)
                .with_context(|| format!("Failed to create output directory: {}", options.output_dir.display()))?;
        }

        // Generate entity files
        for (entity_name, entity_type) in &schema.entities {
            let entity_code = self.entity_generator.generate_entity(entity_type, &schema)?;
            let entity_file_path = options.output_dir.join(format!("{}.rs", entity_name.to_lowercase()));
            
            if options.overwrite || !entity_file_path.exists() {
                fs::write(&entity_file_path, entity_code)
                    .with_context(|| format!("Failed to write entity file: {}", entity_file_path.display()))?;
                result.generated_files.push(entity_file_path);
            }
        }

        // Generate module file
        let module_code = self.generate_module_file(&schema, &options)?;
        let module_path = options.output_dir.join("mod.rs");
        
        if options.overwrite || !module_path.exists() {
            fs::write(&module_path, module_code)
                .with_context(|| format!("Failed to write module file: {}", module_path.display()))?;
            result.generated_files.push(module_path);
        }

        // Optionally generate a store.rs that re-exports SDK store types
        if options.generate_store {
            let store_path = options.output_dir.join("store.rs");
            if options.overwrite || !store_path.exists() {
                let store_code = self.generate_store_file()?;
                fs::write(&store_path, store_code)
                    .with_context(|| format!("Failed to write store file: {}", store_path.display()))?;
                result.generated_files.push(store_path);
            }
        }


        result.entity_count = schema.entities.len();
        Ok(result)
    }

    /// Generate the module file that re-exports all entities
    fn generate_module_file(&self, schema: &EntitySchema, options: &GenerationOptions) -> Result<String> {
        let mut code = String::new();
        
        code.push_str("//! Generated entities module\n\n");
        code.push_str("// This file is auto-generated. Do not edit manually.\n\n");
        
        // Import dependencies
        code.push_str("use sentio_sdk::entity::{Entity, EntityId, ID, BigDecimal, Timestamp, Bytes};\n");
        code.push_str("use serde::{Serialize, Deserialize};\n\n");

        // Re-export all entities
        for entity_name in schema.entity_names() {
            code.push_str(&format!("pub mod {};\n", entity_name.to_lowercase()));
            code.push_str(&format!("pub use {}::{};\n", entity_name.to_lowercase(), entity_name));
        }

        if options.generate_store {
            code.push_str("\npub mod store;\n");
            code.push_str("pub use store::*;\n");
        }

        Ok(code)
    }

    /// Generate the store helpers module that re-exports SDK store types
    fn generate_store_file(&self) -> Result<String> {
        let mut code = String::new();
        code.push_str("//! Generated store helpers\n\n");
        code.push_str("// This file is auto-generated. Do not edit manually.\n\n");
        code.push_str("pub use sentio_sdk::entity::store::{Store, StoreContext};\n");
        Ok(code)
    }
}

impl Default for EntityGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of code generation
#[derive(Debug)]
pub struct GenerationResult {
    /// List of generated files
    pub generated_files: Vec<PathBuf>,
    /// Number of entities processed
    pub entity_count: usize,
    /// Validation result
    pub validation: Option<ValidationResult>,
}

impl GenerationResult {
    fn new() -> Self {
        Self {
            generated_files: Vec::new(),
            entity_count: 0,
            validation: None,
        }
    }

    /// Print a summary of the generation results
    pub fn print_summary(&self) {
        println!("✅ Code generation completed successfully");
        println!("   Generated {} entities", self.entity_count);
        println!("   Created {} files", self.generated_files.len());
        
        if let Some(ref validation) = self.validation {
            if !validation.warnings.is_empty() {
                println!("   Schema validation warnings: {}", validation.warnings.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_simple_entity() {
        let schema = r#"
            type User @entity {
                id: ID!
                name: String!
                email: String! @unique
            }
        "#;

        let temp_dir = TempDir::new().unwrap();
        let options = GenerationOptions {
            output_dir: temp_dir.path().to_path_buf(),
            module_name: "test_entities".to_string(),
            generate_store: false,
            overwrite: true,
        };

        let mut generator = EntityGenerator::new();
        let result = generator.process_schema_string(schema, options).await.unwrap();

        assert_eq!(result.entity_count, 1);
        assert!(result.generated_files.len() >= 2); // entity file + mod.rs

        // Check that files were actually created
        for file_path in &result.generated_files {
            assert!(file_path.exists(), "Generated file should exist: {:?}", file_path);
        }
    }

    #[tokio::test]
    async fn test_generate_multiple_entities() {
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

        let temp_dir = TempDir::new().unwrap();
        let options = GenerationOptions {
            output_dir: temp_dir.path().to_path_buf(),
            module_name: "test_entities".to_string(),
            generate_store: true,
            overwrite: true,
        };

        let mut generator = EntityGenerator::new();
        let result = generator.process_schema_string(schema, options).await.unwrap();

        assert_eq!(result.entity_count, 2);
        assert!(result.generated_files.len() >= 4); // 2 entity files + mod.rs + store.rs
    }
}
