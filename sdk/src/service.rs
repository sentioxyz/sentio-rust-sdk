use std::sync::Arc;
use anyhow::Result;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

use crate::core::plugin_manager::PluginManager;
use crate::processor::{
    processor_v3_server::ProcessorV3,
    ConfigureHandlersResponse,
    ProcessConfigRequest, ProcessConfigResponse,
    ProcessStreamRequest, ProcessStreamResponseV3,
    StartRequest, UpdateTemplatesRequest,
};

#[derive(Clone)]
pub struct ProcessorService {
    pub plugin_manager: Arc<PluginManager>,
}

impl Default for ProcessorService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessorService {
    pub fn new() -> Self {
        Self { plugin_manager: Arc::new(PluginManager::default()) }
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
}

#[tonic::async_trait]
impl ProcessorV3 for ProcessorService {
    type ProcessBindingsStreamStream = std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<ProcessStreamResponseV3, Status>> + Send>,
    >;

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

        // Build handler configs using existing plugin mechanism
        let mut handler_config = ConfigureHandlersResponse {
            contract_configs: vec![],
            account_configs: vec![],
        };

        // Configure for all chains/processors
        self.plugin_manager.configure_all_plugins(&mut handler_config);

        let mut response = ProcessConfigResponse {
            config: None,
            execution_config: Some(crate::processor::ExecutionConfig {
                sequential: false,
                force_exact_block_time: false,
                handler_order_inside_transaction: 0,
                process_binding_timeout: 30,
                skip_start_block_validation: false,
                rpc_retry_times: 3,
                eth_abi_decoder_config: None,
            }),
            contract_configs: handler_config.contract_configs,
            template_instances: vec![],
            account_configs: handler_config.account_configs,
            metric_configs: vec![],
            event_tracking_configs: vec![],
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
            // new session
            let db_backend =
                Arc::new(crate::entity::store::backend::Backend::remote());
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
                                    let pm = plugin_manager.clone();
                                    let db = db_backend.clone();
                                    let tx_resp = tx_clone.clone();
                                    // Spawn per-binding processing so the stream keeps receiving next requests
                                    tokio::spawn(async move {
                                        // Create RuntimeContext with the tx clone for event logging and empty metadata
                                        let runtime_context = crate::core::RuntimeContext::new_with_empty_metadata(tx_resp.clone(), process_id, db.clone());
                                        // Process and send response
                                        let response = match pm.process(&binding, runtime_context).await {
                                            Ok(result) => {
                                                debug!(
                                                    "Successfully processed binding for chain '{}'",
                                                    binding.chain_id
                                                );
                                                ProcessStreamResponseV3 {
                                                    process_id,
                                                    value: Some(crate::processor::process_stream_response_v3::Value::Result(result)),
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    "Failed to process binding for chain '{}': {}",
                                                    binding.chain_id, e
                                                );
                                                let mut err_result = crate::processor::ProcessResult::default();
                                                err_result.states = Some(crate::processor::StateResult {
                                                    config_updated: false,
                                                    error: Some(e.to_string()),
                                                });
                                                ProcessStreamResponseV3 {
                                                    process_id,
                                                    value: Some(crate::processor::process_stream_response_v3::Value::Result(err_result)),
                                                }
                                            }
                                        };
                                        // session ended, reset the db
                                        db.reset();
                                        // this is the last response to end the session.
                                        if let Err(e) = tx_resp.send(Ok(response)).await {
                                            error!("Failed to send response: {}", e);
                                        }
                                    });
                                }
                                process_stream_request::Value::DbResult(db_result) => {
                                    db_backend.receive_db_result(db_result)
                                }
                                process_stream_request::Value::Start(start) => {
                                    debug!("Received start signal: {}", start);
                                    todo!()
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
