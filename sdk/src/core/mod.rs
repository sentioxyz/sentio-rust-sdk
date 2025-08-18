
pub mod processor;
pub mod handler_register;
pub mod plugin;
pub mod plugin_manager;
pub mod context;
pub mod event_logger;

pub use processor::{BaseProcessor, TypedProcessor};
pub use handler_register::{HandlerInfo, HandlerRegister};
pub use plugin::{Plugin, PluginRegister, AsyncPluginProcessor};
pub use plugin_manager::PluginManager;
pub use context::{Context, BaseContext, RuntimeContext, RUNTIME_CONTEXT, MetaData, Labels};
pub use event_logger::{Event, AttributeValue, EventLogger, DefaultEventLogger};


pub(crate) const USER_PROCESSOR: &str = "user_processor";
