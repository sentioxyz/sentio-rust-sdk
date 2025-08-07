use std::net::SocketAddr;

use anyhow::Result;
use clap::Parser;
use tonic::{transport::Server as TonicServer, Request, Response, Status};
use tracing::{info, debug};

use crate::processor::{
    processor_v3_server::{ProcessorV3, ProcessorV3Server as TonicProcessorV3Server},
    InitResponse, ConfigureHandlersRequest, ConfigureHandlersResponse,
    ProcessStreamRequest, ProcessStreamResponseV2,
};
use crate::default_handler::DefaultProcessorV3Handler;

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

/// A trait that users implement to handle ProcessorV3 gRPC calls
#[tonic::async_trait]
pub trait ProcessorV3Handler: Send + Sync + 'static {
    /// Initialize the processor and return available chain IDs and configuration
    async fn init(&self) -> Result<InitResponse, Status>;

    /// Configure handlers for a specific chain
    async fn configure_handlers(
        &self,
        request: ConfigureHandlersRequest,
    ) -> Result<ConfigureHandlersResponse, Status>;

    /// Process bindings stream - implement your data processing logic here
    async fn process_bindings_stream(
        &self,
        request: tonic::Streaming<ProcessStreamRequest>,
    ) -> Result<tonic::codec::Streaming<ProcessStreamResponseV2>, Status>;
}

/// Internal ProcessorV3 implementation that wraps the user's handler
struct ProcessorV3Impl<T> {
    handler: T,
}

#[tonic::async_trait]
impl<T: ProcessorV3Handler> ProcessorV3 for ProcessorV3Impl<T> {
    type ProcessBindingsStreamStream = tonic::codec::Streaming<ProcessStreamResponseV2>;

    async fn init(&self, request: Request<()>) -> Result<Response<InitResponse>, Status> {
        debug!("Received init request from client: {:?}", request.remote_addr());
        let response = self.handler.init().await?;
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
        
        let response = self.handler.configure_handlers(req_data).await?;
        info!("Configure handlers completed for chain, returning {} contract configs", 
              response.contract_configs.len());
        Ok(Response::new(response))
    }

    async fn process_bindings_stream(
        &self,
        request: Request<tonic::Streaming<ProcessStreamRequest>>,
    ) -> Result<Response<Self::ProcessBindingsStreamStream>, Status> {
        debug!("Starting process_bindings_stream from client: {:?}", request.remote_addr());
        let stream = self.handler.process_bindings_stream(request.into_inner()).await?;
        debug!("Process bindings stream established");
        Ok(Response::new(stream))
    }
}

/// Sentio Processor gRPC Server
pub struct Server<T> {
    handler: T,
    args: Option<ServerArgs>,
}

impl Server<DefaultProcessorV3Handler> {
    /// Create a new Server with the default handler
    pub fn new() -> Self {
        Self {
            handler: DefaultProcessorV3Handler::new(),
            args: None,
        }
    }
}

impl Default for Server<DefaultProcessorV3Handler> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ProcessorV3Handler> Server<T> {
    /// Create a new Server with a custom handler
    pub fn with_handler(handler: T) -> Self {
        Self {
            handler,
            args: None,
        }
    }

    /// Set custom server arguments (useful for testing or custom configurations)
    pub fn with_args(mut self, args: ServerArgs) -> Self {
        self.args = Some(args);
        self
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
    pub fn start(self) -> Result<()> {
        // Parse command line arguments or use provided args
        let args = self.args.unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        let processor_impl = ProcessorV3Impl {
            handler: self.handler,
        };

        info!("Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);

        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            TonicServer::builder()
                .add_service(TonicProcessorV3Server::new(processor_impl))
                .serve(addr)
                .await
        })?;

        Ok(())
    }

    /// Start the gRPC server with graceful shutdown support (blocking)  
    /// This method creates its own Tokio runtime and blocks until the server stops
    /// Parses command line arguments for port and debug settings
    pub fn start_with_shutdown<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        // Parse command line arguments or use provided args
        let args = self.args.unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        let processor_impl = ProcessorV3Impl {
            handler: self.handler,
        };

        info!("Starting Sentio Processor server on {} with shutdown support", addr);
        debug!("Server configuration: {:?}", args);

        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            TonicServer::builder()
                .add_service(TonicProcessorV3Server::new(processor_impl))
                .serve_with_shutdown(addr, shutdown_signal)
                .await
        })?;

        Ok(())
    }

    /// Start the gRPC server asynchronously (for use within existing async contexts)
    /// This is the async version for when you already have a Tokio runtime
    pub async fn start_async(self) -> Result<()> {
        // Parse command line arguments or use provided args
        let args = self.args.unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        let processor_impl = ProcessorV3Impl {
            handler: self.handler,
        };

        info!("Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);

        TonicServer::builder()
            .add_service(TonicProcessorV3Server::new(processor_impl))
            .serve(addr)
            .await?;

        Ok(())
    }

    /// Start the gRPC server asynchronously with shutdown support
    /// This is the async version for when you already have a Tokio runtime
    pub async fn start_async_with_shutdown<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()>,
    {
        // Parse command line arguments or use provided args
        let args = self.args.unwrap_or_else(|| ServerArgs::parse());
        
        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        
        let processor_impl = ProcessorV3Impl {
            handler: self.handler,
        };

        info!("Starting Sentio Processor server on {} with shutdown support", addr);
        debug!("Server configuration: {:?}", args);

        TonicServer::builder()
            .add_service(TonicProcessorV3Server::new(processor_impl))
            .serve_with_shutdown(addr, shutdown_signal)
            .await?;

        Ok(())
    }
}