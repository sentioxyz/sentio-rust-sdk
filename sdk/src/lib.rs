pub mod server;
pub mod eth;
pub mod default_handler;

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
pub use server::{ProcessorV3Handler, Server, ServerArgs};
pub use default_handler::DefaultProcessorV3Handler;

// Re-export tonic for users who need to implement servers
pub use tonic;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
