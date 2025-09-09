use std::env;
use anyhow::anyhow;
use alloy::json_abi::Event as JsonEvent;
use alloy::dyn_abi::DynSolValue;
use tracing::{debug, info, warn};
use sentio_sdk::{async_trait, Entity};
use sentio_sdk::core::{Context, Event};
use sentio_sdk::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::entity::ID;
use crate::abi_client::AbiClient;
use crate::generated::entities::DecodedLogBuilder;


pub(crate) struct LogDecoderProcessor {
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
        // Can use context directly in async closure now!
        let ctx_chain_id = ctx.chain_id();
        let ctx_transaction_hash = ctx.transaction_hash();

        debug!("processing {}, {}", ctx_chain_id, ctx_transaction_hash);

         let log_id =  format!("{}_{}_{}", ctx.block_number(), ctx.transaction_index(), ctx.log_index());

        match decode_log(&event, &self.abi_client).await {
            Ok(Some(event_with_decoded)) => {
                // Access the decoded data directly from the event
                if let Some(decoded) = &event_with_decoded.decoded {
                    // Convert decoded data to the format expected by the entity
                    let args = format_decoded_args(decoded);
                    let (arg_key_mappings, arg_types) = extract_param_info(&event, &self.abi_client).await.unwrap_or_default();
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
            Ok(None) => {
                debug!("No ABI found for log: {}", log_id);
            }
            Err(e) => {
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
    }
}

// Helper function to format decoded args from alloy DecodedEvent
fn format_decoded_args(decoded: &alloy::dyn_abi::DecodedEvent) -> String {
    let mut args_map = std::collections::HashMap::new();
    
    // Add indexed values
    for (i, value) in decoded.indexed.iter().enumerate() {
        args_map.insert(format!("indexed_{}", i), format_dyn_sol_value(value));
    }
    
    // Add body values  
    for (i, value) in decoded.body.iter().enumerate() {
        args_map.insert(format!("body_{}", i), format_dyn_sol_value(value));
    }
    
    serde_json::to_string(&args_map).unwrap_or_else(|_| "{}".to_string())
}

// Extract event parameter information
async fn extract_param_info(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    if event.log.topics().is_empty() {
        return Ok((vec![], vec![]));
    }

    let signature = &format!("{:?}", event.log.topics()[0]);
    
    if let Some(abi_item) = abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address()), None, None)
        .await?
    {
        let json_event: JsonEvent = serde_json::from_str(&abi_item)?;
        let arg_key_mappings: Vec<String> = json_event.inputs.iter().map(|input| input.name.clone()).collect();
        let arg_types: Vec<String> = json_event.inputs.iter().map(|input| input.ty.to_string()).collect();
        Ok((arg_key_mappings, arg_types))
    } else {
        Ok((vec![], vec![]))
    }
}

// Extract event name
async fn extract_event_name(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<String> {
    if event.log.topics().is_empty() {
        return Ok("unknown".to_string());
    }

    let signature = &format!("{:?}", event.log.topics()[0]);
    
    if let Some(abi_item) = abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address()), None, None)
        .await?
    {
        let json_event: JsonEvent = serde_json::from_str(&abi_item)?;
        Ok(json_event.name)
    } else {
        Ok("unknown".to_string())
    }
}

async fn decode_log(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<Option<EthEvent>> {
    if event.log.topics().is_empty() {
        return Err(anyhow!("Log does not contain a valid signature topic"));
    }

    let signature = &format!("{:?}", event.log.topics()[0]);

    // Try to get ABI from signature
    match abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address()), None, None)
        .await?
    {
        Some(abi_item) => {
            match event.decode_from_abi_str(&abi_item) {
                Ok(decoded_event) => Ok(Some(decoded_event)),
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
                        let data = format!("{:?}", event.log.data());
                        match abi_client
                            .get_abi_from_signature(
                                signature,
                                &format!("{:?}", event.log.address()),
                                Some(&topics),
                                Some(&data),
                            )
                            .await?
                        {
                            Some(abi_item) => {
                                let event_clone = event.clone();
                                let decoded_event = event_clone.decode_from_abi_str(&abi_item)?;
                                Ok(Some(decoded_event))
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
