use std::env;
use sentio_cli::generate_code_sync;

fn main() {
    let project_dir = env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR should be set by cargo");
    
    // Set up rerun triggers for files that should trigger code generation
    println!("cargo:rerun-if-changed=schema.graphql");
    println!("cargo:rerun-if-changed=abis/");
    println!("cargo:rerun-if-changed=contracts/");
    println!("cargo:rerun-if-changed=Move.toml");
    println!("cargo:rerun-if-changed=sui.yaml");
    
    // Run code generation using the sentio CLI library
    match generate_code_sync(&project_dir) {
        Ok(results) => {
            if results.is_empty() {
                // No generators found to run, which is fine
            } else {
                let total_files: usize = results.iter().map(|r| r.files_generated.len()).sum();
                if total_files > 0 {
                    println!("cargo:warning=✅ Code generation completed: {} files generated", total_files);
                }
                
                // Report any failures
                for result in &results {
                    if !result.success {
                        println!("cargo:warning=❌ {} generator failed: {}", result.generator_name, result.message);
                    }
                }
            }
        }
        Err(e) => {
            println!("cargo:warning=❌ Code generation failed: {}", e);
        }
    }
}