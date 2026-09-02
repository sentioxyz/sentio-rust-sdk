use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::core::plugin::HandlerConfigs;
use crate::core::plugin_manager::PluginManager;
use crate::entity::store::backend::Backend;
use crate::processor::{
    processor_v3_server::ProcessorV3,
    process_stream_response::Partitions,
    process_stream_response_v3::Value as ResponseValue,
    DataBinding, ProcessConfigRequest, ProcessConfigResponse, ProcessResult,
    ProcessStreamRequest, ProcessStreamResponseV3, StartRequest, StateResult,
    UpdateTemplatesRequest,
};

/// Environment variable that turns on the binding-data partition handshake.
///
/// Must match the driver's `SENTIO_ENABLE_BINDING_DATA_PARTITION` setting: when
/// enabled, the driver expects a `partitions` answer to every binding and then
/// sends an explicit `start` before we may process it.
pub const ENABLE_PARTITION_ENV: &str = "SENTIO_ENABLE_BINDING_DATA_PARTITION";

type ResponseSender = tokio::sync::mpsc::Sender<Result<ProcessStreamResponseV3, Status>>;

pub struct ProcessorService {
    pub plugin_manager: Arc<PluginManager>,
    execution_config: crate::processor::ExecutionConfig,
    enable_partition: bool,
}

impl Default for ProcessorService {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProcessorService {
    fn clone(&self) -> Self {
        Self {
            plugin_manager: Arc::clone(&self.plugin_manager),
            execution_config: self.execution_config.clone(),
            enable_partition: self.enable_partition,
        }
    }
}

fn partition_enabled_from_env() -> bool {
    std::env::var(ENABLE_PARTITION_ENV)
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

impl ProcessorService {
    pub fn new() -> Self {
        let execution_config = crate::processor::ExecutionConfig {
            sequential: false,
            force_exact_block_time: false,
            handler_order_inside_transaction: 0,
            process_binding_timeout: 600,
            skip_start_block_validation: false,
            rpc_retry_times: 3,
            eth_abi_decoder_config: None,
        };
        Self {
            plugin_manager: Arc::new(PluginManager::default()),
            execution_config,
            enable_partition: partition_enabled_from_env(),
        }
    }

    pub fn new_with_plugin_and_config(
        plugin_manager: Arc<PluginManager>,
        execution_config: crate::processor::ExecutionConfig,
    ) -> Self {
        Self {
            plugin_manager,
            execution_config,
            enable_partition: partition_enabled_from_env(),
        }
    }

    pub fn register_processor<T, P>(&self, processor: T)
    where
        T: crate::core::BaseProcessor + 'static,
        P: crate::core::plugin::PluginRegister<T> + crate::core::plugin::FullPlugin + Default + 'static,
    {
        self.plugin_manager
            .with_plugin_mut::<P, _, _>(|plugin| {
                let _ = plugin.register_processor(processor);
            });
    }

    /// Set the global GraphQL schema that should be returned in get_config
    pub fn set_gql_schema<S: Into<String>>(&self, schema: S) {
        self.plugin_manager.set_gql_schema(schema);
    }

    // No setter for execution_config to keep it immutable after service start.
}

/// Build a `ProcessResult` that only carries a failure state.
fn error_result(error: String) -> ProcessResult {
    ProcessResult {
        states: Some(StateResult {
            error: Some(error),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Default partition answer: partition every handler by block number, which is
/// the driver's default ordering when no user partition key is provided.
fn default_partitions(binding: &DataBinding) -> Partitions {
    use crate::processor::process_stream_response::partitions::{partition, Partition};
    let partitions = binding
        .handler_ids
        .iter()
        .map(|handler_id| {
            (
                *handler_id,
                Partition {
                    value: Some(partition::Value::SysValue(
                        partition::SysValue::BlockNumber as i32,
                    )),
                },
            )
        })
        .collect();
    Partitions { partitions }
}

/// Run one binding in its own task so the stream keeps receiving requests, and
/// send the final `result` message when it finishes (or fails / times out).
fn spawn_binding_processing(
    plugin_manager: Arc<PluginManager>,
    db_backend: Arc<Backend>,
    tx: ResponseSender,
    process_id: i32,
    binding: DataBinding,
    timeout_secs: u64,
    stream_id: i32,
) {
    crate::core::benchmark::on_binding_spawn(stream_id);
    tokio::spawn(async move {
        let runtime_context =
            crate::core::RuntimeContext::new_with_empty_metadata(tx.clone(), process_id, db_backend)
                .with_handler_type(binding.handler_type);
        let start = Instant::now();

        let result = match tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            plugin_manager.process(&binding, runtime_context.clone()),
        )
        .await
        {
            // Buffered timeseries must reach the driver before the `result` message;
            // on failure they are dropped, like the TypeScript runtime does.
            Ok(Ok(result)) => match runtime_context.flush_timeseries().await {
                Ok(()) => {
                    debug!("Successfully processed binding for chain '{}'", binding.chain_id);
                    result
                }
                Err(e) => {
                    error!("Failed to flush timeseries for chain '{}': {}", binding.chain_id, e);
                    error_result(e.to_string())
                }
            },
            Ok(Err(e)) => {
                error!("Failed to process binding for chain '{}': {}", binding.chain_id, e);
                error_result(e.to_string())
            }
            Err(_elapsed) => {
                error!(
                    "Processing binding timed out for chain '{}' after {}s",
                    binding.chain_id, timeout_secs
                );
                error_result(format!("user processor timeout after {}s", timeout_secs))
            }
        };
        let process_time = start.elapsed();

        let send_start = Instant::now();
        let response = ProcessStreamResponseV3 {
            process_id,
            value: Some(ResponseValue::Result(result)),
        };
        if let Err(e) = tx.send(Ok(response)).await {
            error!("Failed to send response: {}", e);
        }
        let send_time = send_start.elapsed();
        let total_time = start.elapsed();

        if std::env::var("SHOW_DETAILED_TIMING").is_ok() {
            debug!(
                "Binding timing: process={}μs, send={}μs, total={}μs",
                process_time.as_micros(),
                send_time.as_micros(),
                total_time.as_micros()
            );
        }

        crate::core::benchmark::record_handler_time(total_time);
        crate::core::benchmark::on_binding_done(stream_id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::ProcessConfigRequest;

    #[tokio::test]
    async fn get_config_includes_db_schema_when_set() {
        let service = ProcessorService::new();
        let schema = "type TestEntity @entity { id: ID! }";
        service.set_gql_schema(schema);

        let req = Request::new(ProcessConfigRequest {});
        let resp = service.get_config(req).await.unwrap().into_inner();

        let db_schema = resp.db_schema.expect("expected db_schema to be set");
        assert!(db_schema.gql_schema.contains("TestEntity"));
    }

    #[tokio::test]
    async fn get_config_is_idempotent_across_calls() {
        use crate::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
        use crate::eth::{EthEventHandler, EventMarker};

        struct P;
        impl EthProcessor for P {
            fn address(&self) -> &str { "*" }
            fn chain_id(&self) -> &str { "1" }
            fn name(&self) -> &str { "p" }
        }
        struct Transfer;
        impl EventMarker for Transfer {
            fn filter() -> Vec<EventFilter> {
                vec![EventFilter { address: None, address_type: None, topics: vec!["0xdd".to_string()] }]
            }
        }
        #[crate::async_trait]
        impl EthEventHandler<Transfer> for P {
            async fn on_event(&self, _: EthEvent, _: crate::eth::context::EthContext) {}
        }

        let service = ProcessorService::new();
        let mut processor_impl = crate::eth::eth_processor::EthProcessorImpl::new(std::sync::Arc::new(P));
        processor_impl.add_event_handler(P, None);
        service.register_processor::<_, crate::EthPlugin>(processor_impl);

        // The driver calls GetConfig once per connection (N workers) and rejects any diff.
        let first = service.get_config(Request::new(ProcessConfigRequest {})).await.unwrap().into_inner();
        let second = service.get_config(Request::new(ProcessConfigRequest {})).await.unwrap().into_inner();
        assert_eq!(first, second);
        assert_eq!(first.contract_configs[0].log_configs[0].handler_id, 0);
    }

    #[test]
    fn default_partitions_cover_every_handler() {
        use crate::processor::process_stream_response::partitions::partition;
        let binding = DataBinding {
            handler_ids: vec![3, 7],
            ..Default::default()
        };
        let partitions = default_partitions(&binding).partitions;
        assert_eq!(partitions.len(), 2);
        for id in [3, 7] {
            assert_eq!(
                partitions[&id].value,
                Some(partition::Value::SysValue(partition::SysValue::BlockNumber as i32))
            );
        }
    }
}

#[tonic::async_trait]
impl ProcessorV3 for ProcessorService {
    async fn start(&self, request: Request<StartRequest>) -> Result<Response<()>, Status> {
        debug!("Received start request from client: {:?}", request.remote_addr());
        let req = request.into_inner();
        info!("Start called with {} template(s)", req.template_instances.len());
        Ok(Response::new(()))
    }

    async fn get_config(
        &self,
        _request: Request<ProcessConfigRequest>,
    ) -> Result<Response<ProcessConfigResponse>, Status> {
        debug!("Received get_config request");

        crate::core::benchmark::init_if_enabled();

        // Collect handler configs from every plugin, then copy them into the response.
        let mut handler_config = HandlerConfigs::default();
        self.plugin_manager.configure_all_plugins(&mut handler_config);

        let mut response = ProcessConfigResponse {
            config: None,
            execution_config: Some(self.execution_config.clone()),
            contract_configs: handler_config.contract_configs,
            template_instances: vec![],
            account_configs: handler_config.account_configs,
            metric_configs: vec![],
            export_configs: vec![],
            event_log_configs: vec![],
            db_schema: None,
        };

        // Attach global GraphQL schema if set on plugin manager
        if let Some(schema) = self.plugin_manager.get_gql_schema() {
            response.db_schema = Some(crate::processor::DataBaseSchema { gql_schema: schema });
        }

        info!("get_config assembled {} contract configs", response.contract_configs.len());
        Ok(Response::new(response))
    }

    async fn update_templates(
        &self,
        request: Request<UpdateTemplatesRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!(
            "UpdateTemplates for chain {} with {} template(s)",
            req.chain_id,
            req.template_instances.len()
        );
        Ok(Response::new(()))
    }

    type ProcessBindingsStreamStream = std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<ProcessStreamResponseV3, Status>> + Send>,
    >;

    async fn process_bindings_stream(
        &self,
        request: Request<tonic::Streaming<ProcessStreamRequest>>,
    ) -> Result<Response<Self::ProcessBindingsStreamStream>, Status> {
        use crate::processor::process_stream_request::Value as RequestValue;
        use tokio_stream::{wrappers::ReceiverStream, StreamExt};
        // Allocate an id for this bindings stream and mark open for benchmarking
        let stream_id = crate::core::benchmark::new_stream_id();
        crate::core::benchmark::on_stream_open(stream_id);
        debug!(
            "Starting process_bindings_stream from client: {:?}",
            request.remote_addr()
        );
        info!("Starting bindings stream processing");

        let mut inbound_stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(1000);

        // Clone the plugin manager Arc for sharing between tasks
        let plugin_manager = self.plugin_manager.clone();
        // Snapshot settings to avoid capturing self in spawned task
        let timeout_secs = (self.execution_config.process_binding_timeout as u64).max(1);
        let enable_partition = self.enable_partition;

        tokio::spawn(async move {
            // new session
            let db_backend = Arc::new(Backend::remote());
            // In partition mode the driver sends the binding, waits for our
            // `partitions` answer, then sends `start`; remember the binding in between.
            let mut last_binding: Option<DataBinding> = None;
            let mut received_start = Instant::now();

            while let Some(stream_request) = inbound_stream.next().await {
                crate::core::benchmark::record_receive_time(received_start.elapsed());
                received_start = Instant::now();

                let req = match stream_request {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Error receiving stream request: {}", e);
                        break;
                    }
                };
                debug!("Received stream request with process_id: {}", req.process_id);
                let process_id = req.process_id;
                let Some(value) = req.value else { continue };

                match value {
                    RequestValue::Binding(binding) => {
                        debug!("Processing binding for chain_id: {}", binding.chain_id);
                        if enable_partition {
                            let partitions = default_partitions(&binding);
                            last_binding = Some(binding);
                            let response = ProcessStreamResponseV3 {
                                process_id,
                                value: Some(ResponseValue::Partitions(partitions)),
                            };
                            if let Err(e) = tx.send(Ok(response)).await {
                                error!("Failed to send partitions: {}", e);
                                break;
                            }
                        } else {
                            spawn_binding_processing(
                                plugin_manager.clone(),
                                db_backend.clone(),
                                tx.clone(),
                                process_id,
                                binding,
                                timeout_secs,
                                stream_id,
                            );
                        }
                    }
                    RequestValue::DbResult(db_result) => db_backend.receive_db_result(db_result),
                    RequestValue::Start(_) => match last_binding.clone() {
                        Some(binding) => spawn_binding_processing(
                            plugin_manager.clone(),
                            db_backend.clone(),
                            tx.clone(),
                            process_id,
                            binding,
                            timeout_secs,
                            stream_id,
                        ),
                        None => {
                            // Without partition mode the binding was already processed on
                            // arrival, so a stray `start` has nothing to do.
                            warn!("start request received without a pending binding; ignoring");
                            if enable_partition {
                                let response = ProcessStreamResponseV3 {
                                    process_id,
                                    value: Some(ResponseValue::Result(error_result(
                                        "start request received without binding".to_string(),
                                    ))),
                                };
                                if let Err(e) = tx.send(Ok(response)).await {
                                    error!("Failed to send response: {}", e);
                                    break;
                                }
                            }
                        }
                    },
                }
            }
            // Stream loop ended, mark stream closed for benchmarking
            crate::core::benchmark::on_stream_close(stream_id);
            debug!("Stream processing task completed");
        });

        let response_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(response_stream)))
    }
}
