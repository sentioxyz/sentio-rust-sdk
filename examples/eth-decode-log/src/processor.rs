use std::env;
use anyhow::anyhow;
use ethers::abi::{Event, ParamType, RawLog, Token};
use serde::Serialize;
use tracing::{debug, info, warn};
use sentio_sdk::{async_trait, Entity};
use sentio_sdk::core::Context;
use sentio_sdk::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::entity::{ID, Timestamp};
use sentio_sdk::EntityStore;
use crate::abi_client::AbiClient;
use eth_decode_log::generated::entities::{DecodedLog, DecodedLogBuilder};

// Remove the manual struct - we'll use the generated DecodedLog entity instead

#[derive(Debug, Clone, Serialize)]
struct ErrorLogData {
    distinct_id: String,
    transaction_hash: String,
    transaction_index: u64,
    block_number: u64,
    block_hash: String,
    log_address: String,
    data: String,
    topics: String,
    error: String,
}

pub(crate) struct LogDecoderProcessor {
    address: String,
    chain_id: String,
    name: String,
}

impl LogDecoderProcessor {
    pub fn new() -> Self {
        Self {
            address: "".to_string(), // Empty means all contracts
            chain_id: std::env::var("CHAIN_ID").unwrap_or_else(|_| "1".to_string()),
            name: "Ethereum Log Decoder".to_string(),
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

// Implement Clone so we can use it in configure_event
impl Clone for LogDecoderProcessor {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            chain_id: self.chain_id.clone(),
            name: self.name.clone(),
        }
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
    async fn on_event(&self, event: EthEvent, ctx: EthContext) {
        // Can use context directly in async closure now!
        let ctx_chain_id = ctx.chain_id();
        let ctx_transaction_hash = ctx.transaction_hash();

        debug!("processing {}, {}", ctx_chain_id, ctx_transaction_hash);

        // Create a basic log identifier from the event itself
        let log_id = format!(
            "{}_{}",
            format!("{:?}", event.log.address),
            event
                .log
                .topics
                .get(0)
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "unknown".to_string())
        );

        // Initialize ABI client with environment variables
        let sentio_host = env::var("SENTIO_HOST")
            .unwrap_or_else(|_| "https://app.sentio.xyz".to_string());
        let chain_id = env::var("CHAIN_ID").unwrap_or_else(|_| "1".to_string());
        let abi_client = AbiClient::new(sentio_host, chain_id.clone());

        match decode_log(&event, &abi_client).await {
            Ok(Some(decoded)) => {
                let decoded_log = DecodedLogBuilder::default()
                    .id(ID::from(log_id.clone()))
                    .chain_id(chain_id.clone())
                    .transaction_hash(ctx.transaction_hash())
                    .transaction_index(ctx.transaction_index())
                    .timestamp(Timestamp::from_timestamp_millis(ctx.block_number() as i64 * 15000).unwrap_or_default()) // Approximate timestamp
                    .block_number(ctx.block_number() as i32)
                    .block_hash("".to_string())
                    .log_index(ctx.log_index())
                    .log_address(format!("{:?}", event.log.address))
                    .data(format!("{:?}", event.log.data))
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
                let error_log = ErrorLogData {
                    distinct_id: log_id,
                    transaction_hash: "".to_string(),
                    transaction_index: 0,
                    block_number: 0,
                    block_hash: "".to_string(),
                    log_address: format!("{:?}", event.log.address),
                    data: format!("{:?}", event.log.data),
                    topics: serde_json::to_string(
                        &event
                            .log
                            .topics
                            .iter()
                            .map(|t| format!("{:?}", t))
                            .collect::<Vec<_>>(),
                    )
                        .unwrap_or_else(|_| "[]".to_string()),
                    error: e.to_string(),
                };

                warn!("Failed to decode log: {:?}", error_log);
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
            match parse_log_with_ethers(&abi_item, event) {
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
                                let decoded = parse_log_with_ethers(&abi_item, event)?;
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

fn parse_log_with_ethers(abi_item: &str, event: &EthEvent) -> anyhow::Result<DecodedLogResult> {
    // Parse the ABI item as an Event
    let event_abi: Event = serde_json::from_str(abi_item)?;

    // Use topics directly from the ethers Log structure
    let topics = event.log.topics.clone();

    // Use data directly from the ethers Log structure
    let data = event.log.data.to_vec();

    // Create RawLog for ethers
    let raw_log = RawLog { topics, data };

    // Decode the log using ethers
    let decoded = event_abi.parse_log(raw_log)?;

    // Extract information
    let event_name = event_abi.name.clone();
    let signature = format!("{:?}", event.log.topics[0]); // Use the original signature from topics

    let mut arg_key_mappings = Vec::new();
    let mut arg_types = Vec::new();
    let mut args_map = std::collections::HashMap::new();

    for (i, param) in event_abi.inputs.iter().enumerate() {
        arg_key_mappings.push(param.name.clone());
        arg_types.push(format_param_type(&param.kind));

        if let Some(value) = decoded.params.get(i) {
            args_map.insert(param.name.clone(), format_token(&value.value));
        }
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

fn format_param_type(param_type: &ParamType) -> String {
    match param_type {
        ParamType::Address => "address".to_string(),
        ParamType::Bytes => "bytes".to_string(),
        ParamType::Int(size) => format!("int{}", size),
        ParamType::Uint(size) => format!("uint{}", size),
        ParamType::Bool => "bool".to_string(),
        ParamType::String => "string".to_string(),
        ParamType::Array(inner) => format!("{}[]", format_param_type(inner)),
        ParamType::FixedBytes(size) => format!("bytes{}", size),
        ParamType::FixedArray(inner, size) => format!("{}[{}]", format_param_type(inner), size),
        ParamType::Tuple(types) => {
            let type_strs: Vec<String> = types.iter().map(format_param_type).collect();
            format!("({})", type_strs.join(","))
        }
    }
}

fn format_token(token: &Token) -> String {
    match token {
        Token::Address(addr) => format!("0x{:x}", addr),
        Token::FixedBytes(bytes) | Token::Bytes(bytes) => format!("0x{}", hex::encode(bytes)),
        Token::Int(val) | Token::Uint(val) => val.to_string(),
        Token::Bool(val) => val.to_string(),
        Token::String(val) => val.clone(),
        Token::Array(tokens) | Token::FixedArray(tokens) => {
            let formatted: Vec<String> = tokens.iter().map(format_token).collect();
            format!("[{}]", formatted.join(","))
        }
        Token::Tuple(tokens) => {
            let formatted: Vec<String> = tokens.iter().map(format_token).collect();
            format!("({})", formatted.join(","))
        }
    }
}
