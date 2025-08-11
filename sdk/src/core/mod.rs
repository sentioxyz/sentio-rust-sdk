
pub mod processor;
pub mod handler_register;
pub mod plugin;
pub mod plugin_manager;

pub use processor::{BaseProcessor, TypedProcessor};
pub use handler_register::{HandlerInfo, HandlerRegister};
pub use plugin::{Plugin, PluginRegister, AsyncPluginProcessor};
pub use plugin_manager::PluginManager;


pub(crate) const USER_PROCESSOR: &str = "user_processor";
