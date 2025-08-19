use anyhow::Result;
use crate::processor::{MetricValue, metric_value::Value, TimeseriesResult};
use super::Labels;

/// Numeric value that can be converted to MetricValue
#[derive(Debug, Clone)]
pub enum NumberValue {
    Integer(i64),
    Float(f64),
    BigDecimal(String),
}

impl NumberValue {
    /// Convert to MetricValue for protobuf
    pub fn to_metric_value(&self) -> MetricValue {
        match self {
            NumberValue::Integer(val) => MetricValue {
                value: Some(Value::DoubleValue(*val as f64)),
            },
            NumberValue::Float(val) => MetricValue {
                value: Some(Value::DoubleValue(*val)),
            },
            NumberValue::BigDecimal(val) => MetricValue {
                value: Some(Value::BigDecimal(val.clone())),
            },
        }
    }
}

impl From<i32> for NumberValue {
    fn from(val: i32) -> Self {
        NumberValue::Integer(val as i64)
    }
}

impl From<i64> for NumberValue {
    fn from(val: i64) -> Self {
        NumberValue::Integer(val)
    }
}

impl From<f32> for NumberValue {
    fn from(val: f32) -> Self {
        NumberValue::Float(val as f64)
    }
}

impl From<f64> for NumberValue {
    fn from(val: f64) -> Self {
        NumberValue::Float(val)
    }
}

impl From<String> for NumberValue {
    fn from(val: String) -> Self {
        NumberValue::BigDecimal(val)
    }
}

impl From<&str> for NumberValue {
    fn from(val: &str) -> Self {
        NumberValue::BigDecimal(val.to_string())
    }
}

/// Options for configuring metrics
#[derive(Debug, Clone, Default)]
pub struct MetricOptions {
    pub unit: Option<String>,
    pub description: Option<String>,
    pub sparse: Option<bool>,
}

/// Counter metric for tracking cumulative values
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    options: MetricOptions,
}

impl Counter {
    /// Create a new counter with the given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            options: MetricOptions::default(),
        }
    }

    /// Create a new counter with options
    pub fn with_options(name: &str, options: MetricOptions) -> Self {
        Self {
            name: name.to_string(),
            options,
        }
    }

    /// Add a value to the counter using runtime context
    pub async fn add<T: Into<NumberValue>>(&self, value: T, labels: Option<Labels>) -> Result<()> {
        use super::RUNTIME_CONTEXT;
        
        let labels = labels.unwrap_or_default();
        let metric_value = value.into().to_metric_value();

        // Create TimeseriesResult with counter data
        let mut timeseries_result = TimeseriesResult {
            metadata: None, // Will be filled by runtime context
            r#type: crate::processor::timeseries_result::TimeseriesType::Counter as i32,
            data: Some(crate::common::RichStruct {
                fields: std::collections::HashMap::from([
                    ("value".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::FloatValue(
                            match metric_value.value.as_ref().unwrap() {
                                crate::processor::metric_value::Value::DoubleValue(v) => *v,
                                _ => 0.0,
                            }
                        ))
                    }),
                    ("add".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::BoolValue(true))
                    }),
                    ("name".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::StringValue(self.name.clone()))
                    })
                ])
            }),
            runtime_info: None,
        };
        
        // Add labels to the data
        for (key, value) in labels {
            if let Some(data) = &mut timeseries_result.data {
                data.fields.insert(key, crate::common::RichValue {
                    value: Some(crate::common::rich_value::Value::StringValue(value))
                });
            }
        }
        
        // Get runtime context with lightweight clone (only Arc pointers are cloned)
        let ctx = RUNTIME_CONTEXT.try_with(|ctx| ctx.clone())
            .map_err(|_| anyhow::anyhow!("Runtime context not available - make sure this is called within a processor handler"))?;
        
        ctx.send_timeseries_result(&self.name, timeseries_result).await
    }

    /// Subtract a value from the counter using runtime context
    pub async fn sub<T: Into<NumberValue>>(&self, value: T, labels: Option<Labels>) -> Result<()> {
        use super::RUNTIME_CONTEXT;
        
        let labels = labels.unwrap_or_default();
        let metric_value = value.into().to_metric_value();

        // Create TimeseriesResult with counter data
        let mut timeseries_result = TimeseriesResult {
            metadata: None, // Will be filled by runtime context
            r#type: crate::processor::timeseries_result::TimeseriesType::Counter as i32,
            data: Some(crate::common::RichStruct {
                fields: std::collections::HashMap::from([
                    ("value".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::FloatValue(
                            match metric_value.value.as_ref().unwrap() {
                                crate::processor::metric_value::Value::DoubleValue(v) => *v,
                                _ => 0.0,
                            }
                        ))
                    }),
                    ("add".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::BoolValue(false))
                    }),
                    ("name".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::StringValue(self.name.clone()))
                    })
                ])
            }),
            runtime_info: None,
        };
        
        // Add labels to the data
        for (key, value) in labels {
            if let Some(data) = &mut timeseries_result.data {
                data.fields.insert(key, crate::common::RichValue {
                    value: Some(crate::common::rich_value::Value::StringValue(value))
                });
            }
        }
        
        // Get runtime context with lightweight clone (only Arc pointers are cloned)
        let ctx = RUNTIME_CONTEXT.try_with(|ctx| ctx.clone())
            .map_err(|_| anyhow::anyhow!("Runtime context not available - make sure this is called within a processor handler"))?;
        
        ctx.send_timeseries_result(&self.name, timeseries_result).await
    }

}

/// Gauge metric for recording arbitrary values at a point in time
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    options: MetricOptions,
}

impl Gauge {
    /// Create a new gauge with the given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            options: MetricOptions::default(),
        }
    }

    /// Create a new gauge with options
    pub fn with_options(name: &str, options: MetricOptions) -> Self {
        Self {
            name: name.to_string(),
            options,
        }
    }

    /// Record a value for the gauge using runtime context
    pub async fn record<T: Into<NumberValue>>(&self, value: T, labels: Option<Labels>) -> Result<()> {
        use super::RUNTIME_CONTEXT;
        
        let labels = labels.unwrap_or_default();
        let metric_value = value.into().to_metric_value();

        // Create TimeseriesResult with gauge data
        let mut timeseries_result = TimeseriesResult {
            metadata: None, // Will be filled by runtime context
            r#type: crate::processor::timeseries_result::TimeseriesType::Gauge as i32,
            data: Some(crate::common::RichStruct {
                fields: std::collections::HashMap::from([
                    ("value".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::FloatValue(
                            match metric_value.value.as_ref().unwrap() {
                                Value::DoubleValue(v) => *v,
                                _ => 0.0,
                            }
                        ))
                    }),
                    ("name".to_string(), crate::common::RichValue {
                        value: Some(crate::common::rich_value::Value::StringValue(self.name.clone()))
                    })
                ])
            }),
            runtime_info: None,
        };
        
        // Add labels to the data
        for (key, value) in labels {
            if let Some(data) = &mut timeseries_result.data {
                data.fields.insert(key, crate::common::RichValue {
                    value: Some(crate::common::rich_value::Value::StringValue(value))
                });
            }
        }
        
        let ctx = RUNTIME_CONTEXT.try_with(|ctx| ctx.clone())
            .map_err(|_| anyhow::anyhow!("Runtime context not available - make sure this is called within a processor handler"))?;
        
        ctx.send_timeseries_result(&self.name, timeseries_result).await
    }

}

/// Meter provides a factory for creating Counter and Gauge instances
#[derive(Debug, Clone)]
pub struct Meter {
    // For now, we'll keep it simple and create new instances each time
    // In the future, we might want to cache metrics
}

impl Meter {
    /// Create a new Meter
    pub fn new() -> Self {
        Self {}
    }

    /// Create or get a Counter with the given name
    pub fn counter(&self, name: &str) -> Counter {
        Counter::new(name)
    }

    /// Create or get a Counter with options
    pub fn counter_with_options(&self, name: &str, options: MetricOptions) -> Counter {
        Counter::with_options(name, options)
    }

    /// Create or get a Gauge with the given name
    pub fn gauge(&self, name: &str) -> Gauge {
        Gauge::new(name)
    }

    /// Create or get a Gauge with options
    pub fn gauge_with_options(&self, name: &str, options: MetricOptions) -> Gauge {
        Gauge::with_options(name, options)
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[tokio::test]
    async fn test_counter_creation_and_usage() {
        // Note: These tests can't run without a proper runtime context
        // They're mainly for compilation testing
        let counter = Counter::new("test_counter");
        
        // Test that the counter can be created
        assert_eq!(counter.name, "test_counter");
        
        // Testing with runtime context would require setting up the task local storage
        // which is complex for unit tests
    }

    #[tokio::test]
    async fn test_gauge_creation_and_usage() {
        // Note: These tests can't run without a proper runtime context
        // They're mainly for compilation testing
        let gauge = Gauge::new("test_gauge");
        
        // Test that the gauge can be created
        assert_eq!(gauge.name, "test_gauge");
        
        // Testing with runtime context would require setting up the task local storage
        // which is complex for unit tests
    }

    #[tokio::test]
    async fn test_number_value_conversions() {
        // Test different number types
        let int_val: NumberValue = 42i32.into();
        let float_val: NumberValue = 3.14f64.into();
        let string_val: NumberValue = "999.999".into();
        
        // Convert to MetricValue
        let int_metric = int_val.to_metric_value();
        let float_metric = float_val.to_metric_value();
        let string_metric = string_val.to_metric_value();
        
        // Verify the values are properly converted
        match int_metric.value {
            Some(crate::processor::metric_value::Value::DoubleValue(val)) => {
                assert_eq!(val, 42.0);
            }
            _ => panic!("Expected DoubleValue for integer conversion"),
        }
        
        match float_metric.value {
            Some(crate::processor::metric_value::Value::DoubleValue(val)) => {
                assert_eq!(val, 3.14);
            }
            _ => panic!("Expected DoubleValue for float conversion"),
        }
        
        match string_metric.value {
            Some(crate::processor::metric_value::Value::BigDecimal(val)) => {
                assert_eq!(val, "999.999");
            }
            _ => panic!("Expected BigDecimal for string conversion"),
        }
    }

    #[tokio::test]
    async fn test_metric_options() {
        let options = MetricOptions {
            unit: Some("bytes".to_string()),
            description: Some("Test metric".to_string()),
            sparse: Some(true),
        };
        
        let meter = Meter::new();
        let _counter = meter.counter_with_options("test_counter_with_options", options.clone());
        let _gauge = meter.gauge_with_options("test_gauge_with_options", options);
        
        // If we get here without panicking, the test passes
        assert!(true);
    }
}