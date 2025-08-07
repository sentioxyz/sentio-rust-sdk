use anyhow::Result;
use tonic::Status;
use tracing::{info, debug};

use crate::processor::{
    InitResponse, ConfigureHandlersRequest, ConfigureHandlersResponse,
    ProcessStreamRequest, ProcessStreamResponseV2, ProjectConfig, ExecutionConfig,
};
use crate::server::ProcessorV3Handler;

/// Default implementation of ProcessorV3Handler for basic Ethereum processing
pub struct DefaultProcessorV3Handler {
    project_name: String,
    project_version: String,
    chain_ids: Vec<String>,
}

impl DefaultProcessorV3Handler {
    /// Create a new default handler with standard Ethereum configuration
    pub fn new() -> Self {
        Self {
            project_name: "sentio-processor".to_string(),
            project_version: "0.1.0".to_string(),
            chain_ids: vec!["ethereum".to_string()],
        }
    }

    /// Create a new handler with custom project information
    pub fn with_project_info(name: String, version: String) -> Self {
        Self {
            project_name: name,
            project_version: version,
            chain_ids: vec!["ethereum".to_string()],
        }
    }

    /// Add supported chain IDs
    pub fn with_chains(mut self, chain_ids: Vec<String>) -> Self {
        self.chain_ids = chain_ids;
        self
    }
}

impl Default for DefaultProcessorV3Handler {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl ProcessorV3Handler for DefaultProcessorV3Handler {
    async fn init(&self) -> Result<InitResponse, Status> {
        info!("Initializing Sentio Processor...");
        debug!("Project: {} v{}", self.project_name, self.project_version);
        debug!("Supported chains: {:?}", self.chain_ids);
        
        let response = InitResponse {
            chain_ids: self.chain_ids.clone(),
            db_schema: None,
            config: Some(ProjectConfig {
                name: self.project_name.clone(),
                version: self.project_version.clone(),
            }),
            execution_config: Some(ExecutionConfig {
                sequential: false,
                force_exact_block_time: false,
                process_binding_timeout: 30,
                skip_start_block_validation: false,
                rpc_retry_times: 3,
                eth_abi_decoder_config: None,
            }),
            metric_configs: vec![],
            export_configs: vec![],
            event_log_configs: vec![],
        };

        info!("Initialization complete with {} chains", response.chain_ids.len());
        Ok(response)
    }

    async fn configure_handlers(
        &self,
        request: ConfigureHandlersRequest,
    ) -> Result<ConfigureHandlersResponse, Status> {
        info!(
            "Configuring handlers for chain: {}, templates: {}",
            request.chain_id,
            request.template_instances.len()
        );

        // Log template details in debug mode
        for (i, template) in request.template_instances.iter().enumerate() {
            debug!("Template {}: {:?} (ID: {})", i, template.contract, template.template_id);
        }

        // Default implementation returns empty configs
        // Users can extend this by implementing their own ProcessorV3Handler
        let response = ConfigureHandlersResponse {
            contract_configs: vec![],
            account_configs: vec![],
        };

        debug!("Handler configuration completed with {} contract configs", 
               response.contract_configs.len());
        Ok(response)
    }

    async fn process_bindings_stream(
        &self,
        _request: tonic::Streaming<ProcessStreamRequest>,
    ) -> Result<tonic::codec::Streaming<ProcessStreamResponseV2>, Status> {
        info!("Starting bindings stream processing");
        debug!("Using default implementation - streaming not implemented");
        
        // Default implementation returns unimplemented
        // Users should implement their own ProcessorV3Handler for real processing
        Err(Status::unimplemented("Default implementation - implement ProcessorV3Handler for custom processing"))
    }
}