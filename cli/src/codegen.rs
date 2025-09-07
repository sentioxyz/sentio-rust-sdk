//! Main code generation module for Sentio CLI
//! 
//! This module provides a centralized code generation system using the Sentio SDK.

use anyhow::Result;
use std::path::Path;

// Re-export types from SDK for backwards compatibility
pub use sentio_sdk::CodegenResult as GeneratorResult;


/// Synchronous wrapper for build.rs usage - now just delegates to SDK
pub fn run_generation_sync<P: AsRef<Path>>(project_dir: P) -> Result<Vec<GeneratorResult>> {
    let src_dir = project_dir.as_ref();
    let dst_dir = src_dir.join("src");
    
    sentio_sdk::codegen(src_dir, &dst_dir)
}