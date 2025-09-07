pub mod codegen;
pub mod core;
pub mod entity;
pub mod eth;
pub mod server;
pub mod service;
pub mod testing;

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
pub use common::*;
pub use core::{BaseProcessor, Plugin};
pub use eth::EthPlugin;
pub use processor::HandlerType;
pub use processor::*;
pub use server::{Server, ServerArgs};
pub use service::ProcessorService;

// Re-export async_trait macro for convenience
pub use async_trait::async_trait;

// Re-export testing framework components
pub use testing::{TestEnvironment, TestProcessorServer};

// Re-export entity framework components
pub use entity::{
    BigDecimal, BigInt, Bytes, Entity, EntityError, EntityId, EntityResult, EntityStore, Filter,
    ID, Int8, ListOptions, QueryBuilder, Store, Timestamp,
};

// Re-export codegen components for build scripts
pub use codegen::{CodeGenerator, Codegen, CodegenResult, codegen};

/// Trait that defines the ability to bind processors to a server instance
/// This allows both production Server and TestProcessorServer to work with the same API
pub trait BindableServer {
    /// Register a processor with the appropriate plugin
    fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T>
            + crate::core::plugin::FullPlugin
            + Default
            + 'static;
}
