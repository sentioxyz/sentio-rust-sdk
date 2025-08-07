pub mod server;
pub mod eth;
pub mod core;

// Include the generated protobuf code
pub mod common {
    tonic::include_proto!("common");
}

pub mod processor {
    tonic::include_proto!("processor");
}

// Re-export commonly used types for convenience
pub use processor::*;
pub use common::*;
pub use server::{Server, ServerArgs};
pub use core::{BaseProcessor, Plugin};
pub use eth::EthPlugin;
pub use processor::HandlerType;

 