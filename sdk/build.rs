fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &["processor.proto", "service/common/protos/common.proto"], 
            &["."]
        )?;
    
    println!("cargo:rerun-if-changed=processor.proto");
    println!("cargo:rerun-if-changed=service/common/protos/common.proto");
    println!("cargo:rerun-if-changed=build.rs");
    
    Ok(())
}