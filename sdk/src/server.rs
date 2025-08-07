use std::net::SocketAddr;

use anyhow::Result;
use clap::Parser;
use tonic::{transport::Server as TonicServer, Request, Response, Status};
use tracing::{info, debug, error};
use crate::core::plugin_manager::PluginManager;

use crate::processor::{
    processor_v3_server::{ProcessorV3, ProcessorV3Server as TonicProcessorV3Server},
    InitResponse, ConfigureHandlersRequest, ConfigureHandlersResponse,
    ProcessStreamRequest, ProcessStreamResponseV2, ExecutionConfig
};

/// Command line arguments for the Sentio server
#[derive(Parser, Debug, Clone)]
#[command(name = "sentio-server")]
#[command(about = "Sentio Processor gRPC Server")]
pub struct ServerArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "50051")]
    pub port: u16,

    /// Enable debug/verbose logging
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub debug: bool,

    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}



/// Sentio Processor gRPC Server
pub struct Server {
    args: Option<ServerArgs>,
    pub plugin_manager: PluginManager,
}

impl Server {
    
    /// Create a new Server with standard Ethereum configuration
    pub fn new() -> Self {
        Self {
            args: None,
            plugin_manager: Default::default(),
        }
    }


    /// Initialize logging based on debug flag
    fn init_logging(debug: bool) {
        let level = if debug { "debug" } else { "info" };
        
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| format!("{}={}", env!("CARGO_PKG_NAME"), level).into())
            )
            .with_target(false)
            .with_thread_ids(debug)
            .with_line_number(debug)
            .with_file(debug)
            .init();
    }

    /// Start the gRPC server and listen for incoming calls (blocking)
    /// This method creates its own Tokio runtime and blocks until the server stops
    /// Parses command line arguments for port and debug settings
    /// Logs any errors and exits the process if startup fails
    pub fn start(self) {
        if let Err(e) = self.try_start() {
            error!("Failed to start server: {}", e);
            std::process::exit(1);
        }
    }

    /// Internal method that returns Result for error handling
    fn try_start(self) -> Result<()> {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        info!("Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);
        info!("Registered {} processor(s):", self.plugin_manager.total_processor_count());
        for (i, processor) in self.plugin_manager.iter_processors().enumerate() {
            info!("  {}. {} (chain_id: {})", i + 1, processor.name(), processor.chain_id());
        }

        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            TonicServer::builder()
                .add_service(TonicProcessorV3Server::new(self))
                .serve(addr)
                .await
        })?;

        Ok(())
    }

    /// Start the gRPC server with graceful shutdown support (blocking)  
    /// This method creates its own Tokio runtime and blocks until the server stops
    /// Parses command line arguments for port and debug settings
    /// Logs any errors and exits the process if startup fails
    pub fn start_with_shutdown<F>(self, shutdown_signal: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Err(e) = self.try_start_with_shutdown(shutdown_signal) {
            error!("Failed to start server with shutdown: {}", e);
            std::process::exit(1);
        }
    }

    /// Internal method for shutdown support that returns Result for error handling
    fn try_start_with_shutdown<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        info!("Starting Sentio Processor server on {} with shutdown support", addr);
        debug!("Server configuration: {:?}", args);

        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            TonicServer::builder()
                .add_service(TonicProcessorV3Server::new(self))
                .serve_with_shutdown(addr, shutdown_signal)
                .await
        })?;

        Ok(())
    }

    /// Start the gRPC server asynchronously (for use within existing async contexts)
    /// This is the async version for when you already have a Tokio runtime
    /// Returns Result for manual error handling (unlike the blocking start() method)
    pub async fn start_async(self) -> Result<()> {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        info!("Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);

        TonicServer::builder()
            .add_service(TonicProcessorV3Server::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    /// Start the gRPC server asynchronously with shutdown support
    /// This is the async version for when you already have a Tokio runtime  
    /// Returns Result for manual error handling (unlike the blocking start methods)
    pub async fn start_async_with_shutdown<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()>,
    {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        info!("Starting Sentio Processor server on {} with shutdown support", addr);
        debug!("Server configuration: {:?}", args);

        TonicServer::builder()
            .add_service(TonicProcessorV3Server::new(self))
            .serve_with_shutdown(addr, shutdown_signal)
            .await?;

        Ok(())
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl ProcessorV3 for Server {
    type ProcessBindingsStreamStream = tonic::codec::Streaming<ProcessStreamResponseV2>;

    async fn init(&self, request: Request<()>) -> Result<Response<InitResponse>, Status> {
        debug!("Received init request from client: {:?}", request.remote_addr());
        info!("Initializing Sentio Processor...");

        // Collect unique chain IDs from all registered processors
        let mut chain_ids = self.plugin_manager.get_all_chain_ids();
        
        // Sort for consistent ordering
        chain_ids.sort();

        debug!("Found {} unique chain IDs from {} processors: {:?}", 
               chain_ids.len(), self.plugin_manager.total_processor_count(), chain_ids);

        let response = InitResponse {
            chain_ids,
            db_schema: None,
            config: None,
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

        info!("Init completed, returning {} chain IDs", response.chain_ids.len());
        Ok(Response::new(response))
    }

    async fn configure_handlers(
        &self,
        request: Request<ConfigureHandlersRequest>,
    ) -> Result<Response<ConfigureHandlersResponse>, Status> {
        let remote_addr = request.remote_addr();
        let req_data = request.into_inner();
        debug!("Received configure_handlers request from {:?} for chain: {}",
               remote_addr, req_data.chain_id);
        debug!("Template instances count: {}", req_data.template_instances.len());

        info!(
            "Configuring handlers for chain: {}, templates: {}",
            req_data.chain_id,
            req_data.template_instances.len()
        );

        // Log template details in debug mode
        for (i, template) in req_data.template_instances.iter().enumerate() {
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
        info!("Configure handlers completed for chain, returning {} contract configs",
              response.contract_configs.len());
        Ok(Response::new(response))
    }

    async fn process_bindings_stream(
        &self,
        request: Request<tonic::Streaming<ProcessStreamRequest>>,
    ) -> Result<Response<Self::ProcessBindingsStreamStream>, Status> {
        debug!("Starting process_bindings_stream from client: {:?}", request.remote_addr());
        info!("Starting bindings stream processing");
        debug!("Using default implementation - streaming not implemented");
        
        // Default implementation returns unimplemented
        // Users should implement their own ProcessorV3Handler for custom processing
        Err(Status::unimplemented("Default implementation - implement ProcessorV3Handler for custom processing"))
    }
}

