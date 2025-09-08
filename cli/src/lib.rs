//! Sentio CLI Library
//!
//! This library provides programmatic access to Sentio CLI functionality,
//! including code generation, project management, and build operations.
//!
//! This is particularly useful for build.rs scripts that need to generate
//! code during the build process.

pub mod codegen;
pub mod commands;
pub mod utils;

// Re-export commonly used types and functions
pub use codegen::{GeneratorResult, run_generation_sync};

// Re-export command types for advanced usage
pub use commands::{
    Command,
    auth::{AuthAction, AuthCommand},
    build::BuildCommand,
    contract::{ContractAction, ContractCommand},
    generate::GenCommand,
    init::InitCommand,
    test::TestCommand,
    upload::UploadCommand,
};

use std::path::Path;

pub fn generate_code<P: AsRef<Path>>(project_dir: P) {
    match run_generation_sync(project_dir) {
        Ok(results) => {
            if results.is_empty() {
                // No generators found to run, which is fine
            } else {
                let total_files: usize = results.iter().map(|r| r.files_generated.len()).sum();
                if total_files > 0 {
                    println!(
                        "cargo:warning=✅ Code generation completed: {} files generated",
                        total_files
                    );
                }

                // Report any failures
                for result in &results {
                    if !result.success {
                        println!(
                            "cargo:warning=❌ {} generator failed: {}",
                            result.generator_name, result.message
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!("cargo:warning=❌ Code generation failed: {}", e);
        }
    };
}