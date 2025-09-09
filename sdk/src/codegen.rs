//! Unified code generation interface for the Sentio SDK
//! 
//! This module provides a simple, synchronous API for all code generators.
//! It handles source discovery, validation, and generation coordination.

use std::io::Write;
use anyhow::Result;
use derive_builder::Builder;
use std::path::{Path, PathBuf};

/// Result of a code generation operation
#[derive(Debug, Clone, Builder)]
pub struct CodegenResult {
    pub generator_name: String,
    pub files_generated: Vec<PathBuf>,
    pub success: bool,
    pub message: String,
}

/// Trait for all code generators
pub trait CodeGenerator {
    /// Name of this generator (e.g. "entity", "abi")
    fn generator_name(&self) -> &str;
    
    /// Check if this generator should run for the given source directory
    fn should_generate(&self, src_dir: &Path) -> bool;
    
    /// Generate code from source to destination directory
    fn generate(&self, src_dir: &Path, dst_dir: &Path) -> Result<CodegenResult>;
}

/// Unified code generation runner
pub struct Codegen {
    generators: Vec<Box<dyn CodeGenerator>>,
}

impl Codegen {
    /// Create a new codegen runner with default generators
    pub fn new() -> Self {
        let mut codegen = Self {
            generators: Vec::new(),
        };
        
        // Register built-in generators
        codegen.register_generator(Box::new(crate::entity::codegen::EntityCodeGenerator::new()));
        
        codegen
    }
    
    /// Register a custom generator
    pub fn register_generator(&mut self, generator: Box<dyn CodeGenerator>) {
        self.generators.push(generator);
    }
    
    /// Run all applicable generators from src to dst directory
    pub fn generate_all(&self, src_dir: &Path, dst_dir: &Path) -> Result<Vec<CodegenResult>> {
        let mut results = Vec::new();
        
        for generator in &self.generators {
            if generator.should_generate(src_dir) {
                match generator.generate(src_dir, dst_dir) {
                    Ok(result) => results.push(result),
                    Err(e) => results.push(CodegenResult {
                        generator_name: generator.generator_name().to_string(),
                        files_generated: vec![],
                        success: false,
                        message: format!("Generation failed: {}", e),
                    }),
                }
            }
        }

        if !results.is_empty() {
            // generate a mod.rs
            let mut mod_rs_path = PathBuf::from(dst_dir);
            mod_rs_path.push("mod.rs");
            let mut mod_rs = std::fs::File::create(mod_rs_path)?;
            for result in &results {
                for file in &result.files_generated {
                    let file_name = file.file_name().unwrap().to_str().unwrap();
                    let mod_name = file_name.split('.').next().unwrap();
                    mod_rs.write_all(format!("pub mod {};\n", mod_name).as_bytes())?;
                }
            }
            // If a schema.graphql exists under src, expose it as a static const for the app
            let schema_path = src_dir.join("schema.graphql");
            if schema_path.exists() {
                mod_rs.write_all(b"\n")?;
                mod_rs.write_all(b"pub const GQL_SCHEMA: &str = include_str!(\"../../schema.graphql\");\n")?;
            }
            mod_rs.write_all(b"\n")?;
        }
        
        Ok(results)
    }
}

impl Default for Codegen {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple synchronous codegen function for build scripts
pub fn codegen<P: AsRef<Path>>(src_dir: P, dst_dir: P) -> Result<Vec<CodegenResult>> {
    let codegen = Codegen::new();
    codegen.generate_all(src_dir.as_ref(), dst_dir.as_ref())
}
