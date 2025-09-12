
pub mod processor;
pub mod handler_register;
pub mod plugin;
pub mod plugin_manager;
pub mod context;
pub mod event_logger;
pub mod metrics;
pub mod event_types;
pub mod conversions;

#[cfg(feature = "benchmark")]
pub mod benchmark;

#[cfg(not(feature = "benchmark"))]
pub mod benchmark {
    pub fn init_if_enabled() {}
    pub fn new_stream_id() -> i32 { 0 }
    pub fn on_stream_open(_stream_id: i32) {}
    pub fn on_stream_close(_stream_id: i32) {}
    pub fn on_binding_spawn(_stream_id: i32) {}
    pub fn on_binding_done(_stream_id: i32) {}
    pub fn record_handler_time(_dur: std::time::Duration) {}
    pub fn record_db_time(_dur: std::time::Duration) {}
    pub fn record_receive_time(_dur: std::time::Duration) {}
}

#[cfg(feature = "profiling")]
pub mod profiling;

pub use processor::{BaseProcessor, TypedProcessor};
pub use handler_register::{HandlerInfo, HandlerRegister};
pub use plugin::{Plugin, PluginRegister, AsyncPluginProcessor};
pub use plugin_manager::PluginManager;
pub use context::{Context, BaseContext, RuntimeContext, RUNTIME_CONTEXT, MetaData, Labels, Meter, Counter, Gauge, MetricOptions, NumberValue, StateCollector, StateUpdateCollector, StateUpdate};
pub use event_types::{Event, AttributeValue};
pub use event_logger::EventLogger;



pub(crate) const USER_PROCESSOR: &str = "user_processor";
