//! Unified code generation interface for the Sentio SDK
//! 
//! This module provides a simple, synchronous API for all code generators.
//! It handles source discovery, validation, and generation coordination.

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