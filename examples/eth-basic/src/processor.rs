//! Multichain ERC20 transfer processor.
//!
//! One wildcard (`address = "*"`) binding per chain receives every `Transfer` log on
//! that chain and emits it as a `Transfer` event log. No RPC calls and no entity
//! store: the token address on the log is all the token information we record, so
//! the processor never waits on a node and the driver never has to commit entity
//! rows.

use alloy::primitives::{Address, U256};
use num_bigint::{BigInt, Sign};
use sentio_sdk::core::{Context, Event};
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
use sentio_sdk::eth::{EthEventHandler, EventMarker, Log};
use sentio_sdk::async_trait;
use tracing::{debug, warn};

/// keccak256("Transfer(address,address,uint256)")
pub const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Name of the event log emitted per transfer.
pub const TRANSFER_EVENT: &str = "Transfer";

/// Chains to track, as (chain id, start block). Ethereum is indexed from genesis;
/// a wildcard ERC20 processor over a whole chain is expensive, so raise the start
/// block (the commented entries are rough recent values) before uploading if you
/// only need recent history.
pub const CHAINS: &[(&str, u64)] = &[
    ("1", 0), // Ethereum, from genesis
    // ("8453", 22_000_000),   // Base
    // ("42161", 270_000_000), // Arbitrum
    // ("10", 127_000_000),    // Optimism
    // ("137", 64_000_000),    // Polygon
    // ("56", 43_000_000),     // BSC
    // ("43114", 52_000_000),  // Avalanche
];

pub struct Erc20TransferProcessor {
    chain_id: String,
    start_block: u64,
    name: String,
}

impl Erc20TransferProcessor {
    pub fn new(chain_id: &str, start_block: u64) -> Self {
        Self {
            chain_id: chain_id.to_string(),
            start_block,
            name: format!("ERC20 transfers (chain {})", chain_id),
        }
    }
}

impl EthProcessor for Erc20TransferProcessor {
    fn address(&self) -> &str {
        "*"
    }

    fn chain_id(&self) -> &str {
        &self.chain_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn start_block(&self) -> Option<u64> {
        Some(self.start_block)
    }
}

pub struct TransferEvent;

impl EventMarker for TransferEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter { address: None, address_type: None, topics: vec![TRANSFER_TOPIC.to_string()] }]
    }
}

/// Decode an ERC20 `Transfer(address indexed from, address indexed to, uint256 value)`
/// log. Returns `None` for logs that merely share the topic (ERC721 transfers index
/// the token id as a fourth topic, some tokens omit indexing) — those are skipped,
/// like `skipWhenDecodeFailed` does in the TypeScript processor.
pub fn decode_transfer(log: &Log) -> Option<(Address, Address, U256)> {
    let topics = log.topics();
    let data = &log.data().data;
    if topics.len() != 3 || data.len() != 32 {
        return None;
    }
    Some((Address::from_word(topics[1]), Address::from_word(topics[2]), U256::from_be_slice(data)))
}

#[async_trait]
impl EthEventHandler<TransferEvent> for Erc20TransferProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        let Some((from, to, value)) = decode_transfer(&event.log) else {
            debug!("skipping non-ERC20 Transfer log {}:{}", ctx.transaction_hash(), ctx.log_index());
            return;
        };
        let token = alloy::hex::encode_prefixed(event.log.address());
        let from = alloy::hex::encode_prefixed(from);
        let to = alloy::hex::encode_prefixed(to);
        // Raw token units: without `decimals` there is nothing to scale by.
        let value = BigInt::from_bytes_be(Sign::Plus, &value.to_be_bytes::<32>());

        let transfer = Event::name(TRANSFER_EVENT)
            // One id per log keeps the event unique across chains.
            .distinct_id(&format!("{}-{}-{}", self.chain_id, ctx.transaction_hash(), ctx.log_index()))
            .attr("chain", self.chain_id.clone())
            .attr("token", token)
            .attr("from", from)
            .attr("to", to)
            .attr("value", value)
            .attr("block_number", ctx.block_number() as i64)
            .attr("tx_hash", ctx.transaction_hash())
            .attr("log_index", ctx.log_index() as i64);
        if let Err(e) = ctx.base_context().event_logger().emit(&transfer).await {
            warn!("failed to emit transfer {}:{}: {}", ctx.transaction_hash(), ctx.log_index(), e);
        }
    }
}
