use crate::processor::processor_v3_server::ProcessorV3Server as TonicProcessorV3Server;
use crate::service::ProcessorService;
use anyhow::Result;
use clap::Parser;
use std::future::Future;
use std::net::SocketAddr;
use tonic::transport::Server as TonicServer;
use tracing::{debug, error, info};

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
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Port for profiling HTTP server
    #[cfg(feature = "profiling")]
    #[arg(long, default_value = "4040")]
    pub profiling_port: u16,

    /// Additional unrecognized arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

/// Sentio Processor gRPC Server
pub struct Server {
    args: Option<ServerArgs>,
    pub service: ProcessorService,
}

impl Server {
    /// Create a new Server with standard Ethereum configuration
    pub fn new() -> Self {
        Self {
            args: None,
            service: ProcessorService::new(),
        }
    }

    /// Register a processor with the appropriate plugin
    /// This method uses tokio runtime to handle async operations synchronously
    pub fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T>
            + crate::core::plugin::FullPlugin
            + Default
            + 'static,
    {
        self.service.register_processor::<T, P>(processor);
    }

    /// Set the global GraphQL schema that the server will advertise in get_config
    pub fn set_gql_schema(&self, schema: &'static str) {
        self.service.set_gql_schema(schema);
    }

    /// Initialize logging based on debug flag
    /// Gracefully handles cases where a global subscriber is already initialized
    fn init_logging(debug: bool) {
        let level = if debug { "debug" } else { "info" };

        let result = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| level.into()),
            )
            .with_target(false)
            .with_thread_ids(debug)
            .with_line_number(debug)
            .with_file(debug)
            .try_init();

        match result {
            Ok(_) => {
                // Successfully initialized logging
            }
            Err(_) => {
                // Global subscriber already set, which is fine
                // This allows users to initialize their own logging if they prefer
            }
        }
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
        let args = self.args.clone().unwrap_or_else(ServerArgs::parse);

        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

        info!("🚀 Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);
        info!("📊 gRPC compression enabled: gzip");
        // Note: We can't easily get processor count here without blocking on async lock
        // This will be logged during init() call instead
        info!("🔧 Starting server with plugin manager initialized");

        #[cfg(feature = "profiling")]
        info!("🔥 Profiling enabled on port {}", args.profiling_port);

        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            // Start profiling server if enabled
            #[cfg(feature = "profiling")]
            {
                let profiler =
                    crate::core::profiling::Profiler::new().with_http_endpoint(args.profiling_port);

                tokio::spawn(async move {
                    if let Err(e) = profiler.start_http_server().await {
                        tracing::error!("Failed to start profiling server: {}", e);
                    }
                });
            }

            TonicServer::builder()
                .add_service(
                    TonicProcessorV3Server::new(self.service.clone())
                        .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                        .send_compressed(tonic::codec::CompressionEncoding::Gzip),
                )
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
        F: Future<Output = ()> + Send + 'static,
    {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(ServerArgs::parse);

        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

        info!(
            "🚀 Starting Sentio Processor server on {} with shutdown support",
            addr
        );
        debug!("Server configuration: {:?}", args);
        info!("📊 gRPC compression enabled: gzip");

        #[cfg(feature = "profiling")]
        info!("🔥 Profiling enabled on port {}", args.profiling_port);

        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            // Start profiling server if enabled
            #[cfg(feature = "profiling")]
            {
                let profiler =
                    crate::core::profiling::Profiler::new().with_http_endpoint(args.profiling_port);

                tokio::spawn(async move {
                    if let Err(e) = profiler.start_http_server().await {
                        tracing::error!("Failed to start profiling server: {}", e);
                    }
                });
            }

            TonicServer::builder()
                .add_service(
                    TonicProcessorV3Server::new(self.service.clone())
                        .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                        .send_compressed(tonic::codec::CompressionEncoding::Gzip),
                )
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
        let args = self.args.clone().unwrap_or_else(ServerArgs::parse);

        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

        info!("🚀 Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);
        info!("📊 gRPC compression enabled: gzip");

        #[cfg(feature = "profiling")]
        {
            info!("🔥 Profiling enabled on port {}", args.profiling_port);
        }

        // Start profiling server if enabled
        #[cfg(feature = "profiling")]
        {
            let profiler =
                crate::core::profiling::Profiler::new().with_http_endpoint(args.profiling_port);

            tokio::spawn(async move {
                if let Err(e) = profiler.start_http_server().await {
                    tracing::error!("Failed to start profiling server: {}", e);
                }
            });
        }

        TonicServer::builder()
            .add_service(
                TonicProcessorV3Server::new(self.service.clone())
                    .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                    .send_compressed(tonic::codec::CompressionEncoding::Gzip),
            )
            .serve(addr)
            .await?;

        Ok(())
    }

    /// Start the gRPC server asynchronously with shutdown support
    /// This is the async version for when you already have a Tokio runtime
    /// Returns Result for manual error handling (unlike the blocking start methods)
    pub async fn start_async_with_shutdown<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: Future<Output = ()>,
    {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(ServerArgs::parse);

        // Initialize logging
        Self::init_logging(args.debug);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

        info!(
            "🚀 Starting Sentio Processor server on {} with shutdown support",
            addr
        );
        debug!("Server configuration: {:?}", args);
        info!("📊 gRPC compression enabled: gzip");

        #[cfg(feature = "profiling")]
        {
            info!("🔥 Profiling enabled on port {}", args.profiling_port);
        }

        // Start profiling server if enabled
        #[cfg(feature = "profiling")]
        {
            let profiler =
                crate::core::profiling::Profiler::new().with_http_endpoint(args.profiling_port);

            tokio::spawn(async move {
                if let Err(e) = profiler.start_http_server().await {
                    tracing::error!("Failed to start profiling server: {}", e);
                }
            });
        }

        TonicServer::builder()
            .add_service(
                TonicProcessorV3Server::new(self.service.clone())
                    .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                    .send_compressed(tonic::codec::CompressionEncoding::Gzip),
            )
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

impl crate::BindableServer for Server {
    fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T>
            + crate::core::plugin::FullPlugin
            + Default
            + 'static,
    {
        self.register_processor::<T, P>(processor);
    }
}
