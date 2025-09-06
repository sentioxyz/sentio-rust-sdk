use std::collections::HashMap;
use crate::testing::{TestEnvironment, EthTestFacet, MemoryDatabase, TestResult, TestMetadata, CounterResult, GaugeResult, EventResult};
use crate::core::{AttributeValue, PluginManager, RuntimeContext};
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::{ConfigureHandlersResponse, DataBinding};
use crate::entity::store::backend::Backend;
use crate::eth::EthHandlerType;
use crate::timeseries_result::TimeseriesType;

/// Main test processor server that orchestrates processor testing
///
/// This is the primary entry point for testing processors. It manages the processor
/// lifecycle, provides chain-specific testing facets, and coordinates test execution.
/// 
/// # Example
///
/// ```rust
/// use sentio_sdk::testing::TestProcessorServer;
///
/// #[tokio::test]
/// async fn test_my_processor() {
///     let mut server = TestProcessorServer::new();
///     server.start().await.expect("Failed to start server");
///     let eth_facet = server.eth();
///     let result = eth_facet.test_log(mock_log, Some(1)).await;
///     assert_eq!(result.counters.len(), 1);
/// }
/// ```
pub struct TestProcessorServer {
    /// In-memory database for testing (when entity framework is ready)
    pub db: Arc<MemoryDatabase>,
    
    /// Test environment configuration
    pub environment: TestEnvironment,
    
    /// Plugin manager for coordinating processors (public for facet access)
    pub(crate) plugin_manager: Arc<PluginManager>,
    config: Option<ConfigureHandlersResponse>
}

impl TestProcessorServer {
    pub(crate) async fn process_databinding(&self, data_binding: &DataBinding, test_result: &mut TestResult) {
        let (tx, mut rx) = mpsc::channel(1024);
        

        let remote_backend = std::sync::Arc::new(Backend::memory(self.db.clone()));
        let runtime_context = RuntimeContext::new_with_empty_metadata(tx, 1, remote_backend);
        


        match self.plugin_manager.process(&data_binding, runtime_context).await {
            Ok(_process_result) => {
                // Processing succeeded, collect any messages from the channel
                while let Ok(msg) = rx.try_recv() {
                    if let Ok(response) = msg {
                        self.collect_results_from_channel_response(response, test_result);
                    }
                }
                
                // 6. Update test_result with the shared database that contains the processing results
                test_result.db = self.db.clone();
            }
            Err(e) => {
                eprintln!("Error processing log: {}", e);
                // Continue with other logs even if one fails
                // Still update test_result with the shared database to reflect any partial results
                test_result.db = self.db.clone();
            }
        }
    }

    /// Collect results from a single channel response
    fn collect_results_from_channel_response(
        &self,
        response: crate::ProcessStreamResponseV2,
        test_result: &mut TestResult
    ) {
        if let Some(value) = response.value {
            match value {
                crate::processor::process_stream_response_v2::Value::TsRequest(ts_request) => {
                    // Process timeseries data (counters and gauges)
                    for ts_data in ts_request.data {
                        self.process_timeseries_result(ts_data,  test_result);
                    }
                }
                // TODO: Handle event logs and other request types when the correct protobuf types are identified
                _ => {
                    // Handle other request types if needed
                }
            }
        }
    }

    /// Process a single timeseries result (counter or gauge)
    fn process_timeseries_result(
        &self,
        ts_result: crate::TimeseriesResult,
        test_result: &mut TestResult
    ) {
        let metadata = TestMetadata {
            contract_name: ts_result.metadata.as_ref().map(|m| m.contract_name.clone()),
            block_number: ts_result.metadata.as_ref().map(|m| m.block_number as u64),
            handler_type: EthHandlerType::Event,
        };

        let name = ts_result.metadata.as_ref()
            .map(|m| m.name.clone())
            .unwrap_or_default();

        let labels = ts_result.metadata.as_ref()
            .map(|m| m.labels.clone())
            .unwrap_or_default();

        // Get the metric type from the `type` field
        let metric_type = TimeseriesType::from_i32(ts_result.r#type)
            .unwrap_or(TimeseriesType::Counter);

        // Extract value from data field (which is a RichStruct)
        let value = if let Some(ref data) = ts_result.data {
            // Try to extract a numeric value from the RichStruct fields
            data.fields.get("value")
                .and_then(|v| match &v.value {
                    Some(value_type) => match value_type {
                        crate::common::rich_value::Value::FloatValue(f) => Some(*f),
                        crate::common::rich_value::Value::IntValue(i) => Some(*i as f64),
                        _ => None,
                    },
                    None => None,
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };

        match metric_type {
            TimeseriesType::Counter => {
                test_result.counters.push(CounterResult {
                    name,
                    value,
                    labels,
                    metadata,
                });
            }
            TimeseriesType::Gauge => {
                test_result.gauges.push(GaugeResult {
                    name,
                    value,
                    labels,
                    metadata,
                });
            }
            TimeseriesType::Event => {
                // Process event logs
                self.process_event_log(ts_result, test_result);
            }
        }
    }

    /// Process an event log from TimeseriesResult
    fn process_event_log(
        &self,
        ts_result: crate::TimeseriesResult,
        test_result: &mut TestResult
    ) {
        let metadata = TestMetadata {
            contract_name: ts_result.metadata.as_ref().map(|m| m.contract_name.clone()),
            block_number: ts_result.metadata.as_ref().map(|m| m.block_number as u64),
            handler_type: EthHandlerType::Event,
        };

        // Extract event name and attributes from the RichStruct data
        let (event_name, attributes) = if let Some(data) = &ts_result.data {
            let mut event_name = String::new();
            let mut attributes = HashMap::new();

            // Extract event name
            if let Some(name_value) = data.fields.get("event_name") {
                if let Some(value) = &name_value.value {
                    if let crate::common::rich_value::Value::StringValue(name) = value {
                        event_name = name.clone();
                    }
                }
            }

            // Convert all other fields to JSON values for attributes
            for (key, rich_value) in &data.fields {
                if key != "event_name" {  // Skip the event name field
                    if let Ok(attribute_value) = AttributeValue::try_from(rich_value) {
                        attributes.insert(key.clone(), attribute_value);
                    }
                }
            }

            (event_name, attributes)
        } else {
            ("unknown_event".to_string(), HashMap::new())
        };

        test_result.events.push(EventResult {
            name: event_name,
            attributes,
            metadata,
        });
    }

}

impl TestProcessorServer {
}

impl TestProcessorServer {
    /// Create a new test processor server
    ///
    /// # Example
    ///
    /// ```rust
    /// let server = TestProcessorServer::new();
    /// ```
    pub fn new() -> Self {
        let environment = TestEnvironment::default();
        let plugin_manager = Arc::new(PluginManager::default());
        
        Self {
            db: Arc::new(MemoryDatabase::new()),
            environment,
            plugin_manager,
            config: None,
        }
    }
    
    /// Create an Ethereum testing facet by consuming this server
    pub fn eth(self) -> EthTestFacet {
        EthTestFacet::new(self)
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        self.config = Some(self.get_config().await);
        Ok(())
    }

    /// Get processor configuration for debugging
    pub async fn get_config(&self) -> ConfigureHandlersResponse {
        if self.config.is_some() {
            return self.config.as_ref().unwrap().clone();
        }
        // Get the configuration from all registered plugins
        let mut config_response = ConfigureHandlersResponse {
            contract_configs: vec![],
            account_configs: vec![],
        };
        
        let request = crate::processor::ConfigureHandlersRequest {
            chain_id: "1".to_string(), // Default to Ethereum for testing
            template_instances: vec![],
        };
        
        // Get configuration from plugin manager
        self.plugin_manager.configure_all_plugins(request, &mut config_response);
   
       config_response
    }
}

impl crate::BindableServer for TestProcessorServer {
    fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T> + crate::core::plugin::FullPlugin + Default + 'static,
    {
        // For test context, we'll use futures::executor::block_on which can handle nested calls
        let plugin_manager_arc = self.plugin_manager.clone();
        
        futures::executor::block_on(async move {
            plugin_manager_arc.with_plugin_mut::<P, _, _>(|plugin| {
                plugin.register_processor(processor);
            });
        });
    }
}



/// Test-specific errors
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Processor initialization failed: {0}")]
    ProcessorInit(String),
    
    #[error("Test environment setup failed: {0}")]
    EnvironmentSetup(String),
    
    #[error("Test execution failed: {0}")]
    TestExecution(String),
}

/// Clean up test state
///
/// This function resets global state between tests to ensure test isolation.
/// Call this in test setup or teardown as needed.
pub fn clean_test() {
    // TODO: Reset global state, similar to TypeScript cleanTest()
    // This might involve:
    // - Clearing plugin manager state
    // - Resetting metric collectors
    // - Clearing event loggers
}

 
