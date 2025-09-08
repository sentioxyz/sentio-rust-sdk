use std::env;
use sentio_cli::generate_code;

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
    generate_code(&project_dir); 
}