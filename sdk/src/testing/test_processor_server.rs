use crate::testing::{TestEnvironment, EthTestFacet, MemoryDatabase};
use crate::core::PluginManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ConfigureHandlersResponse;

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
    pub db: MemoryDatabase,
    
    /// Test environment configuration
    pub environment: TestEnvironment,
    
    /// Plugin manager for coordinating processors (public for facet access)
    pub(crate) plugin_manager: Arc<RwLock<PluginManager>>,
    config: Option<ConfigureHandlersResponse>
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
        let plugin_manager = Arc::new(RwLock::new(PluginManager::default()));
        
        Self {
            db: MemoryDatabase::new(),
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
        {
            let mut plugin_manager = self.plugin_manager.write().await;
            plugin_manager.configure_all_plugins(request, &mut config_response);
        }
   
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
            let mut plugin_manager = plugin_manager_arc.write().await;
            let plugin = plugin_manager.plugin::<P>();
            plugin.register_processor(processor);
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

 