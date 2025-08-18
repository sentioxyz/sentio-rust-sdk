use anyhow::Result;
use tracing::debug;
use crate::{LogLevel, processor::{TimeseriesResult, RecordMetaData}};
use std::collections::HashMap;

/// Attribute value that can be stored in events
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    LogLevel(i32), // Store LogLevel as i32 for serialization
    Array(Vec<AttributeValue>),
    Object(HashMap<String, AttributeValue>),
}

/// Event builder with fluent API for creating events
#[derive(Debug, Clone)]
pub struct Event {
    name: String,
    distinct_id: Option<String>,
    severity: Option<LogLevel>,
    message: Option<String>,
    attributes: HashMap<String, AttributeValue>,
}

impl Event {
    /// Create a new event with the given name
    pub fn name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            distinct_id: None,
            severity: None,
            message: None,
            attributes: HashMap::new(),
        }
    }
    
    /// Set the distinct ID for the event
    pub fn distinct_id(mut self, distinct_id: &str) -> Self {
        self.distinct_id = Some(distinct_id.to_string());
        self
    }
    
    /// Set the log level for the event
    pub fn level(mut self, level: LogLevel) -> Self {
        self.severity = Some(level);
        self
    }
    
    /// Set the message for the event
    pub fn message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }
    
    /// Add a string attribute to the event
    pub fn attr(mut self, key: &str, value: impl Into<AttributeValue>) -> Self {
        self.attributes.insert(key.to_string(), value.into());
        self
    }
    
    /// Get the event name
    pub fn get_name(&self) -> &str {
        &self.name
    }
    
    /// Get the distinct ID
    pub fn get_distinct_id(&self) -> Option<&str> {
        self.distinct_id.as_deref()
    }
    
    /// Get the severity level
    pub fn get_severity(&self) -> Option<LogLevel> {
        self.severity
    }
    
    /// Get the message
    pub fn get_message(&self) -> Option<&str> {
        self.message.as_deref()
    }
    
    /// Get the attributes
    pub fn get_attributes(&self) -> &HashMap<String, AttributeValue> {
        &self.attributes
    }
}

/// Implement From conversions for common types to AttributeValue
impl From<String> for AttributeValue {
    fn from(value: String) -> Self {
        AttributeValue::String(value)
    }
}

impl From<&str> for AttributeValue {
    fn from(value: &str) -> Self {
        AttributeValue::String(value.to_string())
    }
}

impl From<f64> for AttributeValue {
    fn from(value: f64) -> Self {
        AttributeValue::Number(value)
    }
}

impl From<i64> for AttributeValue {
    fn from(value: i64) -> Self {
        AttributeValue::Integer(value)
    }
}

impl From<i32> for AttributeValue {
    fn from(value: i32) -> Self {
        AttributeValue::Integer(value as i64)
    }
}

impl From<bool> for AttributeValue {
    fn from(value: bool) -> Self {
        AttributeValue::Boolean(value)
    }
}

impl From<LogLevel> for AttributeValue {
    fn from(value: LogLevel) -> Self {
        AttributeValue::LogLevel(value as i32)
    }
}

impl From<HashMap<String, AttributeValue>> for AttributeValue {
    fn from(value: HashMap<String, AttributeValue>) -> Self {
        AttributeValue::Object(value)
    }
}

impl From<Vec<AttributeValue>> for AttributeValue {
    fn from(value: Vec<AttributeValue>) -> Self {
        AttributeValue::Array(value)
    }
}

/// Event logger trait for emitting events through the runtime context
#[tonic::async_trait]
pub trait EventLogger: Send + Sync {
    /// Emit an event
    async fn emit(&self, event: &Event) -> Result<()>;
}

/// Default event logger implementation that uses the runtime context
#[derive(Clone)]
pub struct DefaultEventLogger {
    /// Reference to metadata lock for accessing context information
    metadata_lock: Option<std::sync::Arc<tokio::sync::RwLock<super::MetaData>>>,
}

impl DefaultEventLogger {
    pub fn new() -> Self {
        Self { 
            metadata_lock: None,
        }
    }
    
    /// Create a new DefaultEventLogger with metadata lock
    pub fn with_metadata_lock(metadata_lock: std::sync::Arc<tokio::sync::RwLock<super::MetaData>>) -> Self {
        Self {
            metadata_lock: Some(metadata_lock),
        }
    }
    
    /// Convert Event to TimeseriesResult following TypeScript normalization patterns
    async fn event_to_timeseries_result(&self, event: &Event) -> Result<TimeseriesResult> {
        use crate::processor::timeseries_result::TimeseriesType;
        
        // Convert event to RichStruct
        let rich_struct = self.event_to_rich_struct(event)?;
        
        // Get metadata from context if available
        let metadata = if let Some(metadata_lock) = &self.metadata_lock {
            let ctx_metadata = metadata_lock.read().await;
            let record_metadata = RecordMetaData {
                address: ctx_metadata.address.clone(),
                contract_name: ctx_metadata.contract_name.clone(),
                block_number: ctx_metadata.block_number,
                transaction_hash: ctx_metadata.transaction_hash.clone(),
                chain_id: ctx_metadata.chain_id.clone(),
                transaction_index: ctx_metadata.transaction_index,
                log_index: ctx_metadata.log_index,
                name: event.get_name().to_string(),
                labels: ctx_metadata.base_labels.clone(),
            };
            
            Some(record_metadata)
        } else {
            None
        };
        
        let timeseries_result = TimeseriesResult {
            metadata,
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
        fields.insert("event_name".to_string(), RichValue {
            value: Some(rich_value::Value::StringValue(event.get_name().to_string()))
        });
        
        // Add severity
        if let Some(severity) = event.get_severity() {
            fields.insert("severity".to_string(), RichValue {
                value: Some(rich_value::Value::StringValue(format!("{:?}", severity)))
            });
        }
        
        // Add message
        if let Some(message) = event.get_message() {
            fields.insert("message".to_string(), RichValue {
                value: Some(rich_value::Value::StringValue(message.to_string()))
            });
        }
        
        // Add distinct_entity_id
        if let Some(distinct_id) = event.get_distinct_id() {
            fields.insert("distinctEntityId".to_string(), RichValue {
                value: Some(rich_value::Value::StringValue(distinct_id.to_string()))
            });
        }
        
        // Add attributes
        for (key, value) in event.get_attributes() {
            let normalized_key = self.normalize_key(key);
            let rich_value = self.attribute_to_rich_value(value)?;
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
        let normalized = name.chars()
            .take(128)
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect();
        
        normalized
    }
    
    /// Convert AttributeValue to RichValue following TypeScript normalization patterns
    fn attribute_to_rich_value(&self, value: &AttributeValue) -> Result<crate::common::RichValue> {
        use crate::common::{RichValue, rich_value};
        
        let rich_value = match value {
            AttributeValue::String(s) => {
                // Truncate strings to max 1000 chars for attribute values
                let truncated = if s.len() > 1000 {
                    debug!("String attribute truncated to 1000 characters: {}", &s[..50]);
                    s.chars().take(1000).collect()
                } else {
                    s.clone()
                };
                
                RichValue {
                    value: Some(rich_value::Value::StringValue(truncated))
                }
            },
            AttributeValue::Number(n) => {
                if n.is_nan() || n.is_infinite() {
                    return Err(anyhow::anyhow!("Cannot submit NaN or Infinity value"));
                }
                
                RichValue {
                    value: Some(rich_value::Value::FloatValue(*n))
                }
            },
            AttributeValue::Integer(i) => {
                RichValue {
                    value: Some(rich_value::Value::Int64Value(*i))
                }
            },
            AttributeValue::Boolean(b) => {
                RichValue {
                    value: Some(rich_value::Value::BoolValue(*b))
                }
            },
            AttributeValue::LogLevel(level) => {
                RichValue {
                    value: Some(rich_value::Value::IntValue(*level))
                }
            },
            AttributeValue::Array(arr) => {
                // Convert array to RichValueList following TypeScript patterns
                let mut rich_values = Vec::new();
                
                for item in arr {
                    let rich_value = self.attribute_to_rich_value(item)?;
                    rich_values.push(rich_value);
                }
                
                RichValue {
                    value: Some(rich_value::Value::ListValue(crate::common::RichValueList {
                        values: rich_values
                    }))
                }
            },
            AttributeValue::Object(obj) => {
                // Convert nested object to RichStruct
                let mut nested_fields = HashMap::new();
                
                for (key, nested_value) in obj {
                    let normalized_key = self.normalize_key(key);
                    let rich_value = self.attribute_to_rich_value(nested_value)?;
                    nested_fields.insert(normalized_key, rich_value);
                }
                
                RichValue {
                    value: Some(rich_value::Value::StructValue(crate::common::RichStruct {
                        fields: nested_fields
                    }))
                }
            }
        };
        
        Ok(rich_value)
    }
}

#[tonic::async_trait]
impl EventLogger for DefaultEventLogger {
    async fn emit(&self, event: &Event) -> Result<()> {
        use crate::core::RUNTIME_CONTEXT;
        
        // Try to get the runtime context from task local storage
        let runtime_context = RUNTIME_CONTEXT.try_with(|runtime_context| runtime_context.clone())
            .map_err(|_| anyhow::anyhow!("No runtime context available for event logging"))?;
        
        // Convert event to TimeseriesResult
        let timeseries_result = self.event_to_timeseries_result(event).await?;
        
        // Send to runtime context
        runtime_context.send_timeseries_result(timeseries_result).await?;
        
        debug!("Emitted event: {}", event.get_name());
        Ok(())
    }
}