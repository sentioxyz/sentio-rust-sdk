use anyhow::Result;
use sentio_sdk::Server;

// No #[tokio::main] needed! The server creates its own runtime
fn main() -> Result<()> {
    println!("Starting Ethereum Basic Processor server...");
    println!("Use --help to see CLI options. Example: --port 8080 --debug");

    // The SDK provides a default ProcessorV3Handler implementation
    let server = Server::new();

    // This blocks until the server stops - no async main needed!
    server.start()?;

    Ok(())
}