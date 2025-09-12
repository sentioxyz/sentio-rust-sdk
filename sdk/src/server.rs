use crate::processor::processor_v3_server::ProcessorV3Server as TonicProcessorV3Server;
use crate::service::ProcessorService;
use anyhow::Result;
use clap::Parser;
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

    /// Process binding timeout in seconds (default 600). Also via env PROCESS_BINDING_TIMEOUT or legacy PROCESS_TIMEOUT_SECS
    #[arg(long, default_value = "600")]
    pub process_binding_timeout: u64,

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
    execution_config: Option<crate::processor::ExecutionConfig>,
}

impl Server {
    /// Create a new Server with standard Ethereum configuration
    pub fn new() -> Self {
        Self {
            args: None,
            service: ProcessorService::new(),
            execution_config: None,
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

    /// Configure execution settings. If `process_binding_timeout` is 0, the value from CLI/env/default is used.
    pub fn set_execution_config(&mut self, config: crate::processor::ExecutionConfig) {
        self.execution_config = Some(config);
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
    fn try_start(mut self) -> Result<()> {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(ServerArgs::parse);
        // Initialize logging
        Self::init_logging(args.debug);

        // Preserve parsed args for async start
        self.args = Some(args);
        // Create and block on the Tokio runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.start_async())
    }

    /// Start the gRPC server asynchronously (for use within existing async contexts)
    /// This is the async version for when you already have a Tokio runtime
    /// Returns Result for manual error handling (unlike the blocking start() method)
    pub async fn start_async(self) -> Result<()> {
        // Parse command line arguments or use provided args
        let args = self.args.clone().unwrap_or_else(ServerArgs::parse);

        // Initialize logging
        Self::init_logging(args.debug);

        // Initialize benchmark reporter (requires Tokio runtime; safe here)
        crate::core::benchmark::init_if_enabled();

        // execution_config will be constructed below before serving

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

        info!("🚀 Starting Sentio Processor server on {}", addr);
        debug!("Server configuration: {:?}", args);

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

        // Construct execution config once (override > env > cli > default)
        let default_timeout = 600i32;
        let env_timeout = std::env::var("PROCESS_BINDING_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .or_else(|| {
                std::env::var("PROCESS_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse::<i32>().ok())
            });
        let cli_timeout = args.process_binding_timeout as i32;
        let selected_timeout = env_timeout.unwrap_or(cli_timeout);
        let selected_timeout = if selected_timeout > 0 {
            selected_timeout
        } else {
            default_timeout
        };
        let exec_cfg = if let Some(mut cfg) = self.execution_config.clone() {
            if cfg.process_binding_timeout <= 0 {
                cfg.process_binding_timeout = selected_timeout;
            }
            cfg
        } else {
            crate::processor::ExecutionConfig {
                sequential: false,
                force_exact_block_time: false,
                handler_order_inside_transaction: 0,
                process_binding_timeout: selected_timeout,
                skip_start_block_validation: false,
                rpc_retry_times: 3,
                eth_abi_decoder_config: None,
            }
        };

        let service = ProcessorService::new_with_plugin_and_config(
            self.service.plugin_manager.clone(),
            exec_cfg,
        );

        let mut server = TonicProcessorV3Server::new(service)
            .accept_compressed(tonic::codec::CompressionEncoding::Gzip);
        if std::env::var("GRPC_ENABLE_COMPRESS").is_ok()
            && std::env::var("GRPC_ENABLE_COMPRESS")? == "true"
        {
            server = server.send_compressed(tonic::codec::CompressionEncoding::Gzip);
        }

        TonicServer::builder()
            .tcp_keepalive(Some(std::time::Duration::from_secs(10)))
            .http2_keepalive_timeout(Some(std::time::Duration::from_secs(10)))
            .add_service(server)
            .serve(addr)
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
