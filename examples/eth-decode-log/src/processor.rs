use std::env;
use anyhow::anyhow;
use alloy::json_abi::Event as JsonEvent;
use alloy::dyn_abi::{DynSolEvent, DynSolValue};
use alloy::primitives::{LogData, B256};
use tracing::{debug, info, warn};
use sentio_sdk::{async_trait, Entity};
use sentio_sdk::core::{Context, Event};
use sentio_sdk::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::entity::{ID, Timestamp};
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
            Ok(Some(decoded)) => {
                let decoded_log = DecodedLogBuilder::default()
                    .id(ID::from(log_id.clone()))
                    .chain_id(ctx_chain_id)
                    .transaction_hash(ctx.transaction_hash())
                    .transaction_index(ctx.transaction_index())
                    .timestamp(Timestamp::from_timestamp_millis(ctx.block_number() as i64 * 15000).unwrap_or_default()) // Approximate timestamp
                    .block_number(ctx.block_number() as i32)
                    .block_hash("".to_string())
                    .log_index(ctx.log_index())
                    .log_address(format!("{:?}", event.log.address))
                    .data( event.log.data.0)
                    .topics(event
                        .log
                        .topics
                        .iter()
                        .map(|t| format!("{:?}", t))
                        .collect())
                    .args(decoded.args.clone())
                    .arg_key_mappings(decoded.arg_key_mappings.clone())
                    .signature(decoded.signature.clone())
                    .event_name(decoded.event_name.clone())
                    .arg_types(decoded.arg_types.clone())
                    .build()
                    .expect("Failed to build DecodedLog entity");
                decoded_log.save().await.expect("Failed to save decoded log entity");

                info!(
                    "Successfully decoded and stored log: {} - {}",
                    decoded.event_name, log_id
                );
                debug!("Decoded log data: {:?}", decoded_log);
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
                    .attr("log_address", format!("{:?}", event.log.address))
                    .attr("data", format!("{:?}", event.log.data))
                    .attr("topics", serde_json::to_string(
                        &event
                            .log
                            .topics
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



#[derive(Debug)]
struct DecodedLogResult {
    args: String,
    arg_key_mappings: Vec<String>,
    signature: String,
    event_name: String,
    arg_types: Vec<String>,
}

async fn decode_log(event: &EthEvent, abi_client: &AbiClient) -> anyhow::Result<Option<DecodedLogResult>> {
    if event.log.topics.is_empty() {
        return Err(anyhow!("Log does not contain a valid signature topic"));
    }

    let signature = &format!("{:?}", event.log.topics[0]);

    // Try to get ABI from signature
    match abi_client
        .get_abi_from_signature(signature, &format!("{:?}", event.log.address), None, None)
        .await?
    {
        Some(abi_item) => {
            match parse_log_with_alloy(&abi_item, event) {
                Ok(decoded) => Ok(Some(decoded)),
                Err(e) => {
                    if e.to_string().contains("data out-of-bounds")
                        || e.to_string().contains("insufficient")
                    {
                        // Try again with topics and data
                        let topics: Vec<String> = event
                            .log
                            .topics
                            .iter()
                            .map(|t| format!("{:?}", t))
                            .collect();
                        let data = format!("{:?}", event.log.data);
                        match abi_client
                            .get_abi_from_signature(
                                signature,
                                &format!("{:?}", event.log.address),
                                Some(&topics),
                                Some(&data),
                            )
                            .await?
                        {
                            Some(abi_item) => {
                                let decoded = parse_log_with_alloy(&abi_item, event)?;
                                Ok(Some(decoded))
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

fn parse_log_with_alloy(abi_item: &str, event: &EthEvent) -> anyhow::Result<DecodedLogResult> {
    // Parse the ABI item as a JsonEvent first
    let json_event: JsonEvent = serde_json::from_str(abi_item)?;
    
    // Convert JsonEvent inputs to DynSolTypes for dynamic decoding
    let mut indexed_params: Vec<alloy::dyn_abi::DynSolType> = Vec::new();
    let mut non_indexed_params: Vec<alloy::dyn_abi::DynSolType> = Vec::new();
    
    for param in &json_event.inputs {
        let dyn_type = param.ty.to_string().parse::<alloy::dyn_abi::DynSolType>()
            .map_err(|e| anyhow::anyhow!("Failed to parse type '{}': {}", param.ty, e))?;
        
        if param.indexed {
            indexed_params.push(dyn_type);
        } else {
            non_indexed_params.push(dyn_type);
        }
    }
    
    // Create the body type (tuple of non-indexed parameters)
    let body_type = if non_indexed_params.is_empty() {
        alloy::dyn_abi::DynSolType::Tuple(vec![])
    } else if non_indexed_params.len() == 1 {
        non_indexed_params.into_iter().next().unwrap()
    } else {
        alloy::dyn_abi::DynSolType::Tuple(non_indexed_params)
    };

    let topics: Vec<B256> = event.log.topics.iter()
        .map(|topic| B256::from_slice(topic.as_bytes()))
        .collect();

    // Create DynSolEvent with proper parameters (topic_0, indexed_types, body_type)
    let dyn_event = DynSolEvent::new_unchecked(
        topics.first().copied(),
        indexed_params,
        body_type
    );

    // Convert ethers log data to alloy format


    let data = event.log.data.to_vec();
    let log_data = LogData::new(topics, data.into())
        .ok_or_else(|| anyhow::anyhow!("Invalid log data"))?;

    // Decode the log using alloy's dynamic ABI decoding
    let decoded = dyn_event.decode_log_data(&log_data)?;

    // Extract information
    let event_name = json_event.name.clone();
    let signature = format!("{:?}", event.log.topics[0]);

    let mut arg_key_mappings = Vec::new();
    let mut arg_types = Vec::new();
    let mut args_map = std::collections::HashMap::new();

    // Process parameters from both indexed and non-indexed data
    for (i, input) in json_event.inputs.iter().enumerate() {
        let param_name = input.name.clone();
        let param_type = input.ty.to_string();
        
        arg_key_mappings.push(param_name.clone());
        arg_types.push(param_type);

        // Get decoded value from alloy's DecodedEvent structure
        // The DecodedEvent has 'indexed' and 'body' fields containing the decoded values
        let value_str = if input.indexed {
            // For indexed parameters, access from the indexed field
            if let Some(indexed_value) = decoded.indexed.get(i) {
                format_dyn_sol_value(indexed_value)
            } else {
                "[indexed value not found]".to_string()
            }
        } else {
            // For non-indexed parameters, access from the body field
            if let Some(body_value) = decoded.body.get(i) {
                format_dyn_sol_value(body_value)
            } else {
                "[body value not found]".to_string()
            }
        };
        
        args_map.insert(param_name, value_str);
    }

    let args = serde_json::to_string(&args_map)?;

    Ok(DecodedLogResult {
        args,
        arg_key_mappings,
        signature,
        event_name,
        arg_types,
    })
}

// This function is no longer needed as we extract type info directly from JsonEvent

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
