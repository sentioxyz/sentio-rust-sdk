use sentio_sdk::Server;
use {{PROJECT_NAME_SNAKE}}::*;
use sentio_sdk::eth::eth_processor::EthProcessor;

fn main() {
    let server = Server::new();

    let processor = {{PROJECT_CLASS_NAME}}::new();
    
    // TODO: Configure your event handlers here
    // Example:
    // processor
    //     .configure_event::<YourEvent>(None)
    //     .bind(&server);
    
    server.start();
}