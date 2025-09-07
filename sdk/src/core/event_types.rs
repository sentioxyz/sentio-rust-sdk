use anyhow::Result;
use std::collections::HashMap;

use crate::LogLevel;
use crate::common::{RichStruct, RichValue, RichValueList, TokenAmount, rich_value};
use crate::core::conversions::{
    bigdecimal_to_proto, bigint_to_proto, proto_to_bigdecimal, proto_to_bigint,
};
use crate::entity::types::{BigDecimal, BigInt, Bytes, Timestamp};

/// Attribute value that can be stored in events
#[derive(Debug, Clone)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    LogLevel(i32),
    Timestamp(Timestamp),
    BigInt(BigInt),
    BigDecimal(BigDecimal),
    Bytes(Bytes),
    Token(TokenAmount),
    Array(Vec<AttributeValue>),
    Object(HashMap<String, AttributeValue>),
}

/// Event builder with fluent API for creating events
#[derive(Debug, Clone)]
pub struct Event {
    pub(crate) name: String,
    pub(crate) distinct_id: Option<String>,
    pub(crate) severity: Option<LogLevel>,
    pub(crate) message: Option<String>,
    pub(crate) attributes: HashMap<String, AttributeValue>,
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

    /// Add an attribute to the event
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

// Implement From conversions for common types to AttributeValue
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

impl From<Timestamp> for AttributeValue {
    fn from(value: Timestamp) -> Self {
        AttributeValue::Timestamp(value)
    }
}

impl From<BigInt> for AttributeValue {
    fn from(value: BigInt) -> Self {
        AttributeValue::BigInt(value)
    }
}

impl From<Bytes> for AttributeValue {
    fn from(value: Bytes) -> Self {
        AttributeValue::Bytes(value)
    }
}

impl From<TokenAmount> for AttributeValue {
    fn from(value: TokenAmount) -> Self {
        AttributeValue::Token(value)
    }
}

impl From<BigDecimal> for AttributeValue {
    fn from(value: BigDecimal) -> Self {
        AttributeValue::BigDecimal(value)
    }
}

// BigInt/BigDecimal conversions are provided by core::conversions

// TryFrom conversions between AttributeValue and RichValue, aligned with entity/serialization.rs
impl TryFrom<&AttributeValue> for RichValue {
    type Error = anyhow::Error;
    fn try_from(value: &AttributeValue) -> Result<Self> {
        Ok(match value {
            AttributeValue::String(s) => RichValue {
                value: Some(rich_value::Value::StringValue(s.clone())),
            },
            AttributeValue::Number(n) => RichValue {
                value: Some(rich_value::Value::FloatValue(*n)),
            },
            AttributeValue::Integer(i) => RichValue {
                value: Some(rich_value::Value::Int64Value(*i)),
            },
            AttributeValue::Boolean(b) => RichValue {
                value: Some(rich_value::Value::BoolValue(*b)),
            },
            AttributeValue::LogLevel(level) => RichValue {
                value: Some(rich_value::Value::IntValue(*level)),
            },
            AttributeValue::Timestamp(ts) => {
                let ts = prost_types::Timestamp {
                    seconds: ts.timestamp(),
                    nanos: ts.timestamp_subsec_nanos() as i32,
                };
                RichValue {
                    value: Some(rich_value::Value::TimestampValue(ts)),
                }
            }
            AttributeValue::BigInt(bi) => {
                let proto = bigint_to_proto(bi);
                RichValue {
                    value: Some(rich_value::Value::BigintValue(proto)),
                }
            }
            AttributeValue::Bytes(bytes) => RichValue {
                value: Some(rich_value::Value::BytesValue(bytes.to_vec())),
            },
            AttributeValue::BigDecimal(bd) => {
                let proto = bigdecimal_to_proto(bd);
                RichValue {
                    value: Some(rich_value::Value::BigdecimalValue(proto)),
                }
            }
            AttributeValue::Token(t) => RichValue {
                value: Some(rich_value::Value::TokenValue(t.clone())),
            },
            AttributeValue::Array(list) => {
                let values: Result<Vec<_>> = list.iter().map(|v| RichValue::try_from(v)).collect();
                RichValue {
                    value: Some(rich_value::Value::ListValue(RichValueList {
                        values: values?,
                    })),
                }
            }
            AttributeValue::Object(map) => {
                let mut fields = HashMap::new();
                for (k, v) in map.iter() {
                    fields.insert(k.clone(), RichValue::try_from(v)?);
                }
                RichValue {
                    value: Some(rich_value::Value::StructValue(RichStruct { fields })),
                }
            }
        })
    }
}

impl TryFrom<&RichValue> for AttributeValue {
    type Error = anyhow::Error;
    fn try_from(rich: &RichValue) -> Result<Self> {
        use chrono::{DateTime, Utc};
        match &rich.value {
            Some(rich_value::Value::StringValue(s)) => Ok(AttributeValue::String(s.clone())),
            Some(rich_value::Value::FloatValue(f)) => Ok(AttributeValue::Number(*f)),
            Some(rich_value::Value::IntValue(i)) => Ok(AttributeValue::Integer(*i as i64)),
            Some(rich_value::Value::Int64Value(i)) => Ok(AttributeValue::Integer(*i)),
            Some(rich_value::Value::BoolValue(b)) => Ok(AttributeValue::Boolean(*b)),
            Some(rich_value::Value::ListValue(list)) => {
                let items: Result<Vec<_>> = list
                    .values
                    .iter()
                    .map(|v| AttributeValue::try_from(v))
                    .collect();
                Ok(AttributeValue::Array(items?))
            }
            Some(rich_value::Value::StructValue(sv)) => {
                let mut map = HashMap::new();
                for (k, v) in &sv.fields {
                    map.insert(k.clone(), AttributeValue::try_from(v)?);
                }
                Ok(AttributeValue::Object(map))
            }
            Some(rich_value::Value::TimestampValue(ts)) => {
                let dt: DateTime<Utc> = DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                    .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?
                    .with_timezone(&Utc);
                Ok(AttributeValue::Timestamp(dt))
            }
            Some(rich_value::Value::BigintValue(bi)) => {
                Ok(AttributeValue::BigInt(proto_to_bigint(bi)))
            }
            Some(rich_value::Value::BytesValue(bytes)) => {
                Ok(AttributeValue::Bytes(Bytes::from(bytes.clone())))
            }
            Some(rich_value::Value::TokenValue(tok)) => Ok(AttributeValue::Token(tok.clone())),
            Some(rich_value::Value::BigdecimalValue(proto_bd)) => {
                Ok(AttributeValue::BigDecimal(proto_to_bigdecimal(proto_bd)?))
            }
            Some(rich_value::Value::NullValue(_)) | None => {
                Err(anyhow::anyhow!("Null value unsupported for AttributeValue"))
            }
        }
    }
}
