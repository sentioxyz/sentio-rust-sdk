use anyhow::Result;
use crate::processor::{MetricValue, metric_value::Value, CounterResult, GaugeResult, RecordMetaData};
use super::{Context, Labels};

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

    /// Add a value to the counter
    pub async fn add<T: Into<NumberValue>>(&self, ctx: &dyn Context, value: T, labels: Option<Labels>) -> Result<()> {
        let labels = labels.unwrap_or_default();
        let metadata = self.get_metadata(ctx, &labels).await;
        let metric_value = value.into().to_metric_value();

        let counter_result = CounterResult {
            metadata: Some(metadata),
            metric_value: Some(metric_value),
            add: true,
            runtime_info: None,
        };

        // TODO: Send through runtime context
        // For now, we'll store in context until we have the runtime integration
        tracing::debug!("Counter {} add: {:?}", self.name, counter_result);
        Ok(())
    }

    /// Subtract a value from the counter
    pub async fn sub<T: Into<NumberValue>>(&self, ctx: &dyn Context, value: T, labels: Option<Labels>) -> Result<()> {
        let labels = labels.unwrap_or_default();
        let metadata = self.get_metadata(ctx, &labels).await;
        let metric_value = value.into().to_metric_value();

        let counter_result = CounterResult {
            metadata: Some(metadata),
            metric_value: Some(metric_value),
            add: false,
            runtime_info: None,
        };

        // TODO: Send through runtime context
        tracing::debug!("Counter {} sub: {:?}", self.name, counter_result);
        Ok(())
    }

    async fn get_metadata(&self, ctx: &dyn Context, labels: &Labels) -> RecordMetaData {
        let metadata_lock = ctx.get_metadata();
        let metadata = metadata_lock.read().await;
        
        // Combine context metadata with labels
        let mut all_labels = metadata.base_labels.clone();
        all_labels.extend(labels.clone());
        
        RecordMetaData {
            address: metadata.address.clone(),
            contract_name: metadata.contract_name.clone(),
            block_number: metadata.block_number,
            log_index: metadata.log_index,
            transaction_index: metadata.transaction_index,
            transaction_hash: metadata.transaction_hash.clone(),
            chain_id: metadata.chain_id.clone(),
            name: self.name.clone(),
            labels: all_labels,
        }
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

    /// Record a value for the gauge
    pub async fn record<T: Into<NumberValue>>(&self, ctx: &dyn Context, value: T, labels: Option<Labels>) -> Result<()> {
        let labels = labels.unwrap_or_default();
        let metadata = self.get_metadata(ctx, &labels).await;
        let metric_value = value.into().to_metric_value();

        let gauge_result = GaugeResult {
            metadata: Some(metadata),
            metric_value: Some(metric_value),
            runtime_info: None,
        };

        // TODO: Send through runtime context
        tracing::debug!("Gauge {} record: {:?}", self.name, gauge_result);
        Ok(())
    }

    async fn get_metadata(&self, ctx: &dyn Context, labels: &Labels) -> RecordMetaData {
        let metadata_lock = ctx.get_metadata();
        let metadata = metadata_lock.read().await;
        
        // Combine context metadata with labels
        let mut all_labels = metadata.base_labels.clone();
        all_labels.extend(labels.clone());
        
        RecordMetaData {
            address: metadata.address.clone(),
            contract_name: metadata.contract_name.clone(),
            block_number: metadata.block_number,
            log_index: metadata.log_index,
            transaction_index: metadata.transaction_index,
            transaction_hash: metadata.transaction_hash.clone(),
            chain_id: metadata.chain_id.clone(),
            name: self.name.clone(),
            labels: all_labels,
        }
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
    use crate::core::{Context, BaseContext, MetaData, Labels, EventLogger};
    use std::collections::HashMap;
    use tokio::sync::RwLock;
    use std::sync::Arc;

    // Create a test context for testing metrics
    struct TestContext {
        base_context: BaseContext,
    }

    impl TestContext {
        fn new() -> Self {
            let base_context = BaseContext::with_context(
                "0x1234".to_string(),
                "TestContract".to_string(),
                "1".to_string(),
                1000,
                "0xabcd".to_string(),
                0,
                0,
            );
            
            Self { base_context }
        }
    }

    #[tonic::async_trait]
    impl Context for TestContext {
        fn event_logger(&self) -> &dyn EventLogger {
            &self.base_context.event_logger
        }

        fn get_metadata(&self) -> &RwLock<MetaData> {
            &self.base_context.metadata
        }

        fn meter(&self) -> &Meter {
            &self.base_context.meter
        }
    }

    #[tokio::test]
    async fn test_counter_creation_and_usage() {
        let ctx = TestContext::new();
        let meter = ctx.meter();
        
        // Create a counter
        let counter = meter.counter("test_counter");
        
        // Test adding values
        let labels = Some(HashMap::from([
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ]));
        
        // These should not panic and should compile
        assert!(counter.add(&ctx, 1, labels.clone()).await.is_ok());
        assert!(counter.add(&ctx, 5.5, labels.clone()).await.is_ok());
        assert!(counter.sub(&ctx, 2, labels.clone()).await.is_ok());
    }

    #[tokio::test]
    async fn test_gauge_creation_and_usage() {
        let ctx = TestContext::new();
        let meter = ctx.meter();
        
        // Create a gauge
        let gauge = meter.gauge("test_gauge");
        
        // Test recording values
        let labels = Some(HashMap::from([
            ("metric_type".to_string(), "test".to_string()),
        ]));
        
        // These should not panic and should compile
        assert!(gauge.record(&ctx, 100, labels.clone()).await.is_ok());
        assert!(gauge.record(&ctx, 50.5, labels.clone()).await.is_ok());
        assert!(gauge.record(&ctx, "123.456", labels.clone()).await.is_ok());
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