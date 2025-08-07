use chrono::prelude::*;
use tracing::Event;

pub struct EthBindOptions {
    address: String,
    // Optional, if not set, then use eth mainnet
    network: Option<String>,
    // Optional, override default contract name
    name: Option<String>,
    start: Option<TimeOrBlock>,
    end: Option<TimeOrBlock>,
}

pub enum TimeOrBlock {
    Block(u64),
    Time(DateTime<Utc>)
}

pub struct EthProcessor {
    options: EthBindOptions,
}

impl EthProcessor {
    pub fn bind(args: EthBindOptions) -> Self {
        Self { options: args }
    }
}


pub trait EthOnEvent {
    fn on_event(&self, event: Event) -> Self;
}