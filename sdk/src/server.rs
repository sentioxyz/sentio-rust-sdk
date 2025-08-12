use std::net::SocketAddr;

use crate::core::plugin_manager::PluginManager;
use anyhow::Result;
use clap::Parser;
use tonic::{transport::Server as TonicServer, Request, Response, Status};
use tracing::{debug, error, info};

use crate::processor::{
    processor_v3_server::{ProcessorV3, ProcessorV3Server as TonicProcessorV3Server},
    ConfigureHandlersRequest, ConfigureHandlersResponse, ExecutionConfig, InitResponse,
    ProcessStreamRequest, ProcessStreamResponseV2,
};

/// Command line arguments for the Sentio server
#[derive(Parser, Debug, Clone)]
#[command(name = "sentio-processor")]
#[command(about = "Sentio Processor gRPC Server")]
pub struct ServerArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "4000")]
    pub port: u16,

    /// Enable debug/verbose logging
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub debug: bool,

    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Additional unrecognized arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

/// Sentio Processor gRPC Server
pub struct Server {
    args: Option<ServerArgs>,
    pub plugin_manager: std::sync::Arc<tokio::sync::RwLock<PluginManager>>,
}

impl Server {
    /// Create a new Server with standard Ethereum configuration
    pub fn new() -> Self {
        Self {
            args: None,
            plugin_manager: std::sync::Arc::new(tokio::sync::RwLock::new(Default::default())),
        }
    }

    /// Register a processor with the appropriate plugin
    /// This method uses tokio runtime to handle async operations synchronously
    pub fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T> + crate::core::plugin::FullPlugin + Default + 'static,
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let plugin_manager_arc = self.plugin_manager.clone();
        
        rt.block_on(async move {
            let mut plugin_manager = plugin_manager_arc.write().await;
            let plugin = plugin_manager.plugin::<P>();
            plugin.register_processor(processor);
        });
    }

    /// Initialize logging based on debug flag
    fn init_logging(debug: bool) {
        let level = if debug { "debug" } else { "info" };

        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| level.into()),
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
        // Note: We can't easily get processor count here without blocking on async lock
        // This will be logged during init() call instead
        info!("Starting server with plugin manager initialized");

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

        info!(
            "Starting Sentio Processor server on {} with shutdown support",
            addr
        );
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

        info!(
            "Starting Sentio Processor server on {} with shutdown support",
            addr
        );
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
    type ProcessBindingsStreamStream = std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<ProcessStreamResponseV2, Status>> + Send>,
    >;

    async fn init(&self, request: Request<()>) -> Result<Response<InitResponse>, Status> {
        debug!(
            "Received init request from client: {:?}",
            request.remote_addr()
        );
        info!("Initializing Sentio Processor...");

        // Collect unique chain IDs from all registered processors
        let plugin_manager = self.plugin_manager.read().await;
        let mut chain_ids = plugin_manager.get_all_chain_ids();
        let processor_count = plugin_manager.total_processor_count();

        // Sort for consistent ordering
        chain_ids.sort();

        debug!(
            "Found {} unique chain IDs from {} processors: {:?}",
            chain_ids.len(),
            processor_count,
            chain_ids
        );

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

        info!(
            "Init completed, returning {} chain IDs",
            response.chain_ids.len()
        );
        Ok(Response::new(response))
    }

    async fn configure_handlers(
        &self,
        request: Request<ConfigureHandlersRequest>,
    ) -> Result<Response<ConfigureHandlersResponse>, Status> {
        let remote_addr = request.remote_addr();
        let req_data = request.into_inner();
        debug!(
            "Received configure_handlers request from {:?} for chain: {}",
            remote_addr, req_data.chain_id
        );
        debug!(
            "Template instances count: {}",
            req_data.template_instances.len()
        );

        info!(
            "Configuring handlers for chain: {}, templates: {}",
            req_data.chain_id,
            req_data.template_instances.len()
        );

        // Log template details in debug mode
        for (i, template) in req_data.template_instances.iter().enumerate() {
            debug!(
                "Template {}: {:?} (ID: {})",
                i, template.contract, template.template_id
            );
        }

        let mut response = ConfigureHandlersResponse {
            contract_configs: vec![],
            account_configs: vec![],
        };

        // Use the plugin_manager's configure_all_plugins method
        self.plugin_manager
            .write()
            .await
            .configure_all_plugins(req_data, &mut response);

        debug!(
            "Handler configuration completed with {} contract configs",
            response.contract_configs.len()
        );
        info!(
            "Configure handlers completed for chain, returning {} contract configs",
            response.contract_configs.len()
        );
        Ok(Response::new(response))
    }

    async fn process_bindings_stream(
        &self,
        request: Request<tonic::Streaming<ProcessStreamRequest>>,
    ) -> Result<Response<Self::ProcessBindingsStreamStream>, Status> {
        use crate::processor::process_stream_request;
        use tokio_stream::{wrappers::ReceiverStream, StreamExt};

        debug!(
            "Starting process_bindings_stream from client: {:?}",
            request.remote_addr()
        );
        info!("Starting bindings stream processing");

        let mut inbound_stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(1000);

        // Clone the plugin manager Arc for sharing between tasks
        let plugin_manager = self.plugin_manager.clone();

        tokio::spawn(async move {
            while let Some(stream_request) = inbound_stream.next().await {
                match stream_request {
                    Ok(req) => {
                        debug!(
                            "Received stream request with process_id: {}",
                            req.process_id
                        );
                        let process_id = req.process_id;
                        let tx_clone = tx.clone();

                        // Process the request and send responses
                        if let Some(value) = req.value {
                            match value {
                                process_stream_request::Value::Binding(binding) => {
                                    debug!("Processing binding for chain_id: {}", binding.chain_id);

                                    // Use PluginManager's process method directly with the binding
                                    // No need to create a new DataBinding since binding is already the right type
                                    let pm = plugin_manager.read().await;
                                    match pm.process(&binding).await {
                                        Ok(result) => {
                                            debug!(
                                                "Successfully processed binding for chain '{}'",
                                                binding.chain_id
                                            );
                                            let response = ProcessStreamResponseV2 {
                                                process_id,
                                                value: Some(crate::processor::process_stream_response_v2::Value::Result(result)),
                                            };
                                            if let Err(e) = tx_clone.send(Ok(response)).await {
                                                error!("Failed to send response: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Failed to process binding for chain '{}': {}",
                                                binding.chain_id, e
                                            );
                                        }
                                    }
                                }
                                process_stream_request::Value::DbResult(_db_result) => {
                                    debug!("Received DB result, ignoring for now");
                                    // TODO: Handle DB results if needed
                                }
                                process_stream_request::Value::Start(start) => {
                                    debug!("Received start signal: {}", start);
                                    // Handle start signal if needed
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving stream request: {}", e);
                        break;
                    }
                }
            }
            debug!("Stream processing task completed");
        });

        let response_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(response_stream)))
    }
}
