pub mod eth_processor;
pub mod handler_type;
pub mod eth_plugin;
pub mod context;
mod eth_types;
mod tests;

pub use eth_types::*;

pub use handler_type::EthHandlerType;
pub use eth_plugin::EthPlugin;

// Re-export alloy Log for convenient access via crate::eth::Log
pub use alloy::rpc::types::Log;
