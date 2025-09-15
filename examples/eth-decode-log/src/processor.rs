use crate::abi_client::AbiClient;
use crate::bench;
use crate::generated::entities::DecodedLogBuilder;
use alloy::dyn_abi::DynSolValue;
use anyhow::anyhow;
use sentio_sdk::core::{Context, Event};
use sentio_sdk::entity::ID;
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::{async_trait, Entity};
use std::env;
use std::time::Instant;
use tracing::{debug, info, warn};


pub struct LogDecoderProcessor {
    address: String,
    chain_id: String,
    name: String,
    abi_client: AbiClient
}

impl LogDecoderProcessor {
    pub fn new() -> Self {
        // Initialize ABI client with environment variables
        let sentio_host = env::var("SENTIO_HOST")
            .unwrap_or_else(|_| "https://app.sentio.xyz".to_string());
        let chain_id = env::var("CHAIN_ID").unwrap_or_else(|_| "1".to_string());
        let abi_client = AbiClient::new(sentio_host, chain_id.clone());

        Self {
            address: "".to_string(), // Empty means all contracts
            chain_id: env::var("CHAIN_ID").unwrap_or_else(|_| "1".to_string()),
            name: "Ethereum Log Decoder".to_string(),
            abi_client,
        }
    }
}

impl EthProcessor for LogDecoderProcessor {
    fn address(&self) -> &str {
        &self.address
    }

    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub struct AllEventsMarker;

impl EventMarker for AllEventsMarker {
    fn filter() -> Vec<EventFilter> {
        // Return empty filters to capture all events
        vec![]
    }
}

#[async_trait]
impl EthEventHandler<AllEventsMarker> for LogDecoderProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        let handler_start = Instant::now();
        // Can use context directly in async closure now!
        let ctx_chain_id = ctx.chain_id();
        let ctx_transaction_hash = ctx.transaction_hash();

        debug!("processing {}, {}", ctx_chain_id, ctx_transaction_hash);

        let log_id =  format!("{}_{}_{}", ctx.block_number(), ctx.transaction_index(), ctx.log_index());

        let other_start : Instant;
        match decode_log(&event, &self.abi_client).await {
            Ok(event_with_decoded) => {
                bench::record_decode(handler_start.elapsed());
                other_start = Instant::now();
                // Access the decoded data directly from the event
                if let Some(decoded) = &event_with_decoded.decoded {
                    // Pull parameter names and types from ABI to map values by param name
                    let (arg_key_mappings, arg_types, indexed_flags) = extract_param_info(&event, &self.abi_client)
                        .await
                        .unwrap_or_else(|_| (vec![], vec![], vec![]));

                    // Convert decoded data to a name->value JSON map
                    let args = format_decoded_args(decoded, &arg_key_mappings, &indexed_flags);
                    let event_name = extract_event_name(&event, &self.abi_client).await.unwrap_or("unknown".to_string());
                    let signature = format!("{:?}", event.log.topics()[0]);

                    let decoded_log = DecodedLogBuilder::default()
                        .id(ID::from(log_id.clone()))
                        .chain_id(ctx_chain_id)
                        .transaction_hash(ctx.transaction_hash())
                        .transaction_index(ctx.transaction_index())
                        .timestamp(ctx.timestamp()) // Approximate timestamp
                        .block_number(ctx.block_number() as i32)
                        .block_hash("".to_string())
                        .log_index(ctx.log_index())
                        .log_address(format!("{:?}", event.log.address()))
                        .data(event.log.data().data.clone().into())
                        .topics(event
                            .log
                            .topics()
                            .iter()
                            .map(|t| format!("{:?}", t))
                            .collect())
                        .args(args)
                        .arg_key_mappings(arg_key_mappings)
                        .signature(signature)
                        .event_name(event_name.clone())
                        .arg_types(arg_types)
                        .build()
                        .expect("Failed to build DecodedLog entity");
                    decoded_log.save().await.expect("Failed to save decoded log entity");

                    info!(
                        "Successfully decoded and stored log: {} - {}",
                        event_name, log_id
                    );
                    debug!("Decoded log data: {:?}", decoded_log);
                } else {
                    debug!("Event was processed but no decoded data available: {}", log_id);
                }
            }
            Err(e) => {
                bench::record_decode_failed(handler_start.elapsed());
                other_start = Instant::now();
                // Create event logger to emit structured error event
                let event_logger = ctx.base_context().event_logger();
                
                // Create a structured event for the decode failure
                let decode_error_event = Event::name("log_decode_error")
                    .distinct_id(&log_id)
                    .message(&format!("Failed to decode log: {}", e))
                    .attr("log_id", log_id.clone())
                    .attr("transaction_hash", ctx.transaction_hash())
                    .attr("transaction_index", ctx.transaction_index() as i64)
                    .attr("block_number", ctx.block_number() as i64)
                    .attr("log_index", ctx.log_index() as i64)
                    .attr("log_address", format!("{:?}", event.log.address()))
                    .attr("data", format!("{:?}", event.log.data()))
                    .attr("topics", serde_json::to_string(
                        &event
                            .log
                            .topics()
                            .iter()
                            .map(|t| format!("{:?}", t))
                            .collect::<Vec<_>>(),
                    ).unwrap_or_else(|_| "[]".to_string()))
                    .attr("error", e.to_string())
                    .attr("chain_id", ctx.chain_id());

                // Emit the event and keep the warning for backward compatibility
                if let Err(emit_error) = event_logger.emit(&decode_error_event).await {
                    warn!("Failed to emit decode error event: {}", emit_error);
                }
                
                warn!("Failed to decode log {}: {}", log_id, e);
            }
        }
        bench::record_handler(handler_start.elapsed());
        bench::record_other(other_start.elapsed());
        bench::print_if_due();
    }
}

// Helper: format decoded args into a name->value JSON using ABI param ordering
fn format_decoded_args(
    decoded: &alloy::dyn_abi::DecodedEvent,
    param_names: &[String],
    indexed_flags: &[bool],
) -> String {
    use std::collections::HashMap;

    // If we don't have names, fall back to the previous positional format
    if param_names.is_empty() || indexed_flags.len() != param_names.len() {
        let mut fallback = HashMap::new();
        for (i, v) in decoded.indexed.iter().enumerate() {
            fallback.insert(format!("indexed_{}", i), format_dyn_sol_value(v));
        }
        for (i, v) in decoded.body.iter().enumerate() {
            fallback.insert(format!("body_{}", i), format_dyn_sol_value(v));
        }
        return serde_json::to_string(&fallback).unwrap_or_else(|_| "{}".to_string());
    }

    let mut args_map: HashMap<String, String> = HashMap::with_capacity(param_names.len());
    let mut idx_i = 0usize;
    let mut body_i = 0usize;
    for (pos, name) in param_names.iter().enumerate() {
        let is_indexed = indexed_flags.get(pos).copied().unwrap_or(false);
        let value_opt = if is_indexed {
            decoded.indexed.get(idx_i).map(|v| (true, v))
        } else {
            decoded.body.get(body_i).map(|v| (false, v))
        };

        if let Some((from_indexed, v)) = value_opt {
            args_map.insert(name.clone(), format_dyn_sol_value(v));
            if from_indexed { idx_i += 1; } else { body_i += 1; }
        }
    }

    serde_json::to_string(&args_map).unwrap_or_else(|_| "{}".to_string())
}

// Extract event parameter information
async fn extract_param_info(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<(Vec<String>, Vec<String>, Vec<bool>)> {
    if event.log.topics().is_empty() {
        return Ok((vec![], vec![], vec![]));
    }

    let signature = &format!("{:?}", event.log.topics()[0]);
    
    if let Some(json_event) = abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address()), None, None)
        .await?
    {
        let je = json_event.as_ref();
        let arg_key_mappings: Vec<String> = je.inputs.iter().map(|input| input.name.clone()).collect();
        let arg_types: Vec<String> = je.inputs.iter().map(|input| input.ty.to_string()).collect();
        let indexed_flags: Vec<bool> = je.inputs.iter().map(|input| input.indexed).collect();
        Ok((arg_key_mappings, arg_types, indexed_flags))
    } else {
        Ok((vec![], vec![], vec![]))
    }
}

// Extract event name
async fn extract_event_name(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<String> {
    if event.log.topics().is_empty() {
        return Ok("unknown".to_string());
    }

    let signature = &format!("{:?}", event.log.topics()[0]);
    
    if let Some(json_event) = abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address()), None, None)
        .await?
    {
        Ok(json_event.as_ref().name.clone())
    } else {
        Ok("unknown".to_string())
    }
}

async fn decode_log(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<EthEvent> {
    if event.log.topics().is_empty() {
        return Err(anyhow!("Log does not contain a valid signature topic"));
    }

    let signature = &format!("{:?}", event.log.topics()[0]);

    // Try to get ABI from signature
     match abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address()), None, None)
        .await?
    {
        Some(json_event) => {
            match event.decode(json_event.as_ref()) {
                Ok(decoded_event) => {
                    Ok(decoded_event)
                },
                Err(e) => {
                    if e.to_string().contains("data out-of-bounds")
                        || e.to_string().contains("insufficient")
                    {
                        // Try again with topics and data
                        let topics: Vec<String> = event
                            .log
                            .topics()
                            .iter()
                            .map(|t| format!("{:?}", t))
                            .collect();
                        let data_hex =  hex::encode(event.log.data().data.clone());
                        match abi_client
                            .get_abi_from_signature(
                                signature,
                                &format!("{:?}", event.log.address()),
                                Some(&topics),
                                Some(&data_hex),
                            )
                            .await?
                        {
                            Some(json_event) => {
                                let event_clone = event.clone();
                                let decoded_event = event_clone.decode(json_event.as_ref())?;
                                Ok(decoded_event)
                            }
                            None => Err(anyhow!(
                                "No ABI found for signature {} after retry",
                                signature
                            )),
                        }
                    } else {
                        Err(e)
                    }
                }
            }
        }
        None => Err(anyhow!("No ABI found for signature {}", signature)),
    }
}

// This function is no longer needed as we use the SDK's decode_from_abi_str method

fn format_dyn_sol_value(value: &DynSolValue) -> String {
    match value {
        DynSolValue::Address(addr) => format!("0x{:x}", addr),
        DynSolValue::Bool(b) => b.to_string(),
        DynSolValue::Bytes(bytes) => format!("0x{}", hex::encode(bytes)),
        DynSolValue::FixedBytes(bytes, _) => format!("0x{}", hex::encode(bytes)),
        DynSolValue::Int(val, _) => val.to_string(),
        DynSolValue::Uint(val, _) => val.to_string(),
        DynSolValue::String(s) => s.clone(),
        DynSolValue::Array(arr) => {
            let formatted: Vec<String> = arr.iter().map(format_dyn_sol_value).collect();
            format!("[{}]", formatted.join(","))
        }
        DynSolValue::FixedArray(arr) => {
            let formatted: Vec<String> = arr.iter().map(format_dyn_sol_value).collect();
            format!("[{}]", formatted.join(","))
        }
        DynSolValue::Tuple(tuple) => {
            let formatted: Vec<String> = tuple.iter().map(format_dyn_sol_value).collect();
            format!("({})", formatted.join(","))
        }
        // Handle any additional variants
        _ => "[unsupported type]".to_string(),
    }
}


#[cfg(test)]
mod Test {
    use crate::processor::{AllEventsMarker, LogDecoderProcessor};
    use sentio_sdk::eth::eth_processor::EthProcessor;
    use sentio_sdk::eth::Log;
    use sentio_sdk::testing::chain_ids;
    use sentio_sdk::TestProcessorServer;

    #[tokio::test]
    async fn test_event_logging() {
        let mut test_server = TestProcessorServer::new();

        // Create and configure our processor
        let processor = LogDecoderProcessor::new();

        // Configure the processor for both Transfer and Approval events in a single chain
        processor
            .configure_event::<AllEventsMarker>(None)
            .bind(&test_server);

        // Start the test server
        test_server.start().await.expect("Failed to start test server");
        let eth_facet = test_server.eth();

        println!("📝 Testing event logging functionality...");

       let log_json = r#"{
  "address": "0x43d2b8827218752ffe5a35cefc3bbe50ca79af47",
  "topics": [
    "0x8ece6e03e20ee8b641312cdcad8dc887f10b28212c37c7f4e6149f1bcf534c1c"
  ],
  "data": "0x000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000140a7760a00000000000000000000000000000000000000000000000000090c495d4141d960000000000000000000000000000000000000000000000000000000005c3e3b83000000000000000000000000000000000000000000000000000000005c40de83",
  "blockNumber": "0x6c1532",
  "transactionHash": "0x2a4a590b383b6ca794191fe3baf417b285ebd54c66934f0e0bccc75cd9972334",
  "transactionIndex": "0x3a",
  "blockHash": "0x1444858712263a98769db8db97fc03f3f2a4256999c3d4d38736256d583f8056",
  "logIndex": "0x35",
  "removed": false
}"#;
        let log: Log = serde_json::from_str(log_json).unwrap();

        // Process the transfer event
        let result = eth_facet.test_log(log.clone(), Some(chain_ids::ETHEREUM)).await;

        // Verify event logs are recorded
        assert!(result.events.is_empty());
    }
}