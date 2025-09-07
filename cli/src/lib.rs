//! Sentio CLI Library
//! 
//! This library provides programmatic access to Sentio CLI functionality,
//! including code generation, project management, and build operations.
//! 
//! This is particularly useful for build.rs scripts that need to generate
//! code during the build process.

pub mod commands;
pub mod utils;
pub mod codegen;

// Re-export commonly used types and functions
pub use codegen::{GeneratorResult, run_generation_sync};

// Re-export command types for advanced usage
pub use commands::{
    Command,
    generate::GenCommand,
    build::BuildCommand,
    init::InitCommand,
    upload::UploadCommand,
    auth::{AuthCommand, AuthAction},
    contract::{ContractCommand, ContractAction},
    test::TestCommand,
};

use anyhow::Result;
use std::path::Path;

/// Simple synchronous code generation function for build scripts
/// 
/// This is the recommended function for use in build.rs scripts.
/// It will automatically discover and run appropriate code generators
/// based on the project structure.
pub fn generate_code_sync<P: AsRef<Path>>(project_dir: P) -> Result<Vec<GeneratorResult>> {
    run_generation_sync(project_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_generate_code_sync() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a simple schema
        let schema_content = r#"
            scalar BigInt
            scalar BigDecimal
            scalar Bytes
            scalar Timestamp

            type User @entity {
                id: ID!
                name: String!
            }
        "#;

        let schema_path = temp_dir.path().join("schema.graphql");
        fs::write(&schema_path, schema_content).unwrap();

        // Create src directory
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        let results = generate_code_sync(temp_dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }
}