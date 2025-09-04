pub mod server;
pub mod eth;
pub mod core;
pub mod testing;
pub mod entity;
pub mod codegen;

#[cfg(test)]
mod server_test;

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

// Re-export async_trait macro for convenience
pub use async_trait::async_trait;

// Re-export testing framework components
pub use testing::{TestProcessorServer, TestEnvironment};

// Re-export entity framework components
pub use entity::{Entity, EntityId, EntityStore, Store, Filter, ListOptions, ID, BigDecimal, BigInt, Timestamp, Bytes, Int8, EntityError, EntityResult};

// Re-export codegen components for build scripts
pub use codegen::{codegen, Codegen, CodeGenerator, CodegenResult};

/// Trait that defines the ability to bind processors to a server instance
/// This allows both production Server and TestProcessorServer to work with the same API
pub trait BindableServer {
    /// Register a processor with the appropriate plugin
    fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T> + crate::core::plugin::FullPlugin + Default + 'static;
}

 