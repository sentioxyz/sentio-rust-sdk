use anyhow::{Result, Context};
use async_trait::async_trait;
use super::Command;
use crate::codegen::run_generation_sync;
use std::env;

pub struct GenCommand {
    pub generate_handlers: bool,
    pub generate_contracts: bool,
    pub target_contract: Option<String>,
}

#[async_trait]
impl Command for GenCommand {
    async fn execute(&self) -> Result<()> {
        let project_dir = env::current_dir()
            .context("Failed to get current directory")?;

        println!("🔧 Running code generation for project: {}", project_dir.display());

        let results = run_generation_sync(&project_dir)
            .context("Failed to run code generators")?;

        let mut total_generated = 0;
        let mut generators_run = 0;

        for result in results {
            generators_run += 1;
            
            if result.success {
                println!("✅ {} generator: {}", result.generator_name, result.message);
                if !result.files_generated.is_empty() {
                    println!("   Generated files:");
                    for file in &result.files_generated {
                        // Show relative path from project root
                        let relative_path = file.strip_prefix(&project_dir)
                            .unwrap_or(file);
                        println!("     📄 {}", relative_path.display());
                        total_generated += 1;
                    }
                }
            } else {
                println!("⚠️  {} generator: {}", result.generator_name, result.message);
            }
        }

        if generators_run == 0 {
            println!("⚠️  No generators found to run.");
            println!("   Make sure you have the required files in your project:");
            println!("   - schema.graphql (for entity generation)");
            println!("   - contracts/ or abis/ (for contract generation - coming soon)");
        } else if total_generated > 0 {
            println!("\n🎉 Code generation completed successfully!");
            println!("   Generated {} files using {} generators", total_generated, generators_run);
        } else {
            println!("\n✅ Code generation completed (no files needed to be generated)");
        }

        // Check for specific generation options
        if !self.generate_handlers {
            println!("   (Skipped handler generation as requested)");
        }
        if !self.generate_contracts {
            println!("   (Skipped contract generation as requested)");
        }
        if let Some(ref contract) = self.target_contract {
            println!("   (Targeted contract: {})", contract);
        }

        Ok(())
    }
}