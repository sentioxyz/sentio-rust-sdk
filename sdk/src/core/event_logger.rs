use crate::processor::TimeseriesResult;
use anyhow::Result;
use std::collections::HashMap;
// Re-export moved types for backward compatibility of module path users
pub use super::event_types::{AttributeValue, Event};

// Event implementation moved to event_types.rs

// Converters moved to event_types.rs

// Conversions are implemented in core::event_types

/// Pure event logger struct that works with runtime context
#[derive(Debug, Clone)]
pub struct EventLogger {
    // Pure struct with no state
}

impl EventLogger {
    /// Create a new EventLogger
    pub fn new() -> Self {
        Self {}
    }

    /// Emit an event using the runtime context
    pub async fn emit(&self, event: &Event) -> Result<()> {
        use super::RUNTIME_CONTEXT;
        let timeseries_result = self.event_to_timeseries_result(event)?;

        // Get runtime context with lightweight clone (only Arc pointers are cloned)
        let ctx = RUNTIME_CONTEXT.try_with(|ctx| ctx.clone())
            .map_err(|_| anyhow::anyhow!("Runtime context not available - make sure this is called within a processor handler"))?;

        ctx.send_timeseries_result(event.get_name(), timeseries_result)
            .await
    }

    /// Convert Event to TimeseriesResult using runtime context metadata
    fn event_to_timeseries_result(&self, event: &Event) -> Result<TimeseriesResult> {
        use crate::processor::timeseries_result::TimeseriesType;

        // Convert event to RichStruct
        let rich_struct = self.event_to_rich_struct(event)?;

        let timeseries_result = TimeseriesResult {
            metadata: None,
            r#type: TimeseriesType::Event as i32,
            data: Some(rich_struct),
            runtime_info: None,
        };

        Ok(timeseries_result)
    }

    /// Convert Event to RichStruct following TypeScript normalization patterns
    fn event_to_rich_struct(&self, event: &Event) -> Result<crate::common::RichStruct> {
        use crate::common::{RichStruct, RichValue, rich_value};

        let mut fields = HashMap::new();

        // Add event name
        fields.insert(
            "event_name".to_string(),
            RichValue {
                value: Some(rich_value::Value::StringValue(event.get_name().to_string())),
            },
        );

        // Add severity
        if let Some(severity) = event.get_severity() {
            fields.insert(
                "severity".to_string(),
                RichValue {
                    value: Some(rich_value::Value::StringValue(format!("{:?}", severity))),
                },
            );
        }

        // Add message
        if let Some(message) = event.get_message() {
            fields.insert(
                "message".to_string(),
                RichValue {
                    value: Some(rich_value::Value::StringValue(message.to_string())),
                },
            );
        }

        // Add distinct_entity_id
        if let Some(distinct_id) = event.get_distinct_id() {
            fields.insert(
                "distinctEntityId".to_string(),
                RichValue {
                    value: Some(rich_value::Value::StringValue(distinct_id.to_string())),
                },
            );
        }

        // Add attributes
        for (key, value) in event.get_attributes() {
            let normalized_key = self.normalize_key(key);
            let rich_value: crate::common::RichValue = value.try_into()?;
            fields.insert(normalized_key, rich_value);
        }

        Ok(RichStruct { fields })
    }

    /// Normalize key names following TypeScript patterns
    fn normalize_key(&self, name: &str) -> String {
        if name == "labels" {
            return "labels_".to_string();
        }

        // Replace non-alphanumeric chars with underscore and limit to 128 chars
        let normalized = name
            .chars()
            .take(128)
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        normalized
    }
}

impl Default for EventLogger {
    fn default() -> Self {
        Self::new()
    }
}
