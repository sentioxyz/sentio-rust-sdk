use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::Server;
use std::env;
use anyhow::{anyhow, Result};
use serde::Serialize;
use ethers::abi::{Event, ParamType, RawLog, Token};
use ethers::types::H256;
use tracing::{debug, info, warn};

mod abi_client;

use abi_client::AbiClient;

#[derive(Debug, Clone, Serialize)]
struct DecodedLogData {
    id: String,
    chain_id: String,
    transaction_hash: String,
    transaction_index: u64,
    timestamp: u64,
    block_number: u64,
    block_hash: String,
    log_index: u64,
    log_address: String,
    data: String,
    topics: Vec<String>,
    args: String,
    arg_key_mappings: Vec<String>,
    signature: String,
    event_name: String,
    arg_types: Vec<String>,
}

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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let server = Server::new();

    // Create a processor that listens to all events (no filters)
    EthProcessor::new()
        .on_event(
            process_log,
            Vec::new(), // Empty filters means listen to all events
            None,
        )
        .bind(
            &server,
            EthBindOptions::new("") // Empty address means process logs from all contracts
                .with_name("Ethereum Log Decoder")
                .with_network(&env::var("CHAIN_ID").unwrap_or_else(|_| "1".to_string())),
        );

    info!("Starting Ethereum log decoder processor...");
    server.start();
    
    Ok(())
}

async fn process_log(
    event: RawEvent, 
    _ctx: sentio_sdk::eth::context::EthContext,
) {
    // Create a basic log identifier from the event itself
    let log_id = format!("{}_{}", 
        event.address,
        event.topics.get(0).unwrap_or(&"unknown".to_string())
    );
    
    // Initialize ABI client with environment variables
    let sentio_host = env::var("SENTIO_HOST").unwrap_or_else(|_| "https://app.sentio.xyz".to_string());
    let chain_id = env::var("CHAIN_ID").unwrap_or_else(|_| "1".to_string());
    let abi_client = AbiClient::new(sentio_host, chain_id.clone());
    
    match decode_log(&event, &abi_client).await {
        Ok(Some(decoded)) => {
            let decoded_log = DecodedLogData {
                id: log_id.clone(),
                chain_id: chain_id.clone(),
                transaction_hash: "".to_string(), // Context doesn't provide this yet
                transaction_index: 0,
                timestamp: 0,
                block_number: 0,
                block_hash: "".to_string(),
                log_index: 0,
                log_address: event.address.clone(),
                data: event.data.clone(),
                topics: event.topics.clone(),
                args: decoded.args,
                arg_key_mappings: decoded.arg_key_mappings,
                signature: decoded.signature,
                event_name: decoded.event_name,
                arg_types: decoded.arg_types,
            };
            
            info!("Successfully decoded log: {} - {}", decoded_log.event_name, log_id);
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
                log_address: event.address.clone(),
                data: event.data.clone(),
                topics: serde_json::to_string(&event.topics).unwrap_or_else(|_| "[]".to_string()),
                error: e.to_string(),
            };
            
            warn!("Failed to decode log: {:?}", error_log);
        }
    }
}

#[derive(Debug)]
struct DecodedLog {
    args: String,
    arg_key_mappings: Vec<String>,
    signature: String,
    event_name: String,
    arg_types: Vec<String>,
}

async fn decode_log(event: &RawEvent, abi_client: &AbiClient) -> Result<Option<DecodedLog>> {
    if event.topics.is_empty() {
        return Err(anyhow!("Log does not contain a valid signature topic"));
    }
    
    let signature = &event.topics[0];
    
    // Try to get ABI from signature
    match abi_client.get_abi_from_signature(signature, &event.address, None, None).await? {
        Some(abi_item) => {
            match parse_log_with_ethers(&abi_item, event) {
                Ok(decoded) => Ok(Some(decoded)),
                Err(e) => {
                    if e.to_string().contains("data out-of-bounds") || e.to_string().contains("insufficient") {
                        // Try again with topics and data
                        match abi_client.get_abi_from_signature(
                            signature, 
                            &event.address, 
                            Some(&event.topics), 
                            Some(&event.data)
                        ).await? {
                            Some(abi_item) => {
                                let decoded = parse_log_with_ethers(&abi_item, event)?;
                                Ok(Some(decoded))
                            }
                            None => Err(anyhow!("No ABI found for signature {} after retry", signature))
                        }
                    } else {
                        Err(e)
                    }
                }
            }
        }
        None => Err(anyhow!("No ABI found for signature {}", signature))
    }
}

fn parse_log_with_ethers(abi_item: &str, event: &RawEvent) -> Result<DecodedLog> {
    // Parse the ABI item as an Event
    let event_abi: Event = serde_json::from_str(abi_item)?;
    
    // Convert topics from hex strings to H256
    let mut topics = Vec::new();
    for topic in &event.topics {
        let clean_topic = topic.strip_prefix("0x").unwrap_or(topic);
        let bytes = hex::decode(clean_topic).map_err(|e| anyhow!("Invalid topic hex: {}", e))?;
        topics.push(H256::from_slice(&bytes));
    }
    
    // Convert data from hex string to bytes
    let data = {
        let clean_data = event.data.strip_prefix("0x").unwrap_or(&event.data);
        if clean_data.is_empty() {
            Vec::new()
        } else {
            hex::decode(clean_data).map_err(|e| anyhow!("Invalid data hex: {}", e))?
        }
    };
    
    // Create RawLog for ethers
    let raw_log = RawLog {
        topics,
        data,
    };
    
    // Decode the log using ethers
    let decoded = event_abi.parse_log(raw_log)?;
    
    // Extract information
    let event_name = event_abi.name.clone();
    let signature = event.topics[0].clone(); // Use the original signature from topics
    
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
    
    Ok(DecodedLog {
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