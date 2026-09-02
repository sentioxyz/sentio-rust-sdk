//! Multichain ERC20 transfer processor.
//!
//! Rust port of the TypeScript `erc20-transfer-multichain` processor: one wildcard
//! (`address = "*"`) binding per chain receives every `Transfer` log on that chain,
//! token metadata is read over RPC once per token, and each transfer is stored as
//! `Token`/`Transfer` entities (no metrics).

use crate::chains_config::ChainsConfig;
use crate::generated::entities::{TokenBuilder, TransferBuilder};
use alloy::primitives::{Address, U256};
use alloy::providers::RootProvider;
use alloy::rpc::client::RpcClient;
use alloy::sol;
use bigdecimal::BigDecimal;
use moka::sync::Cache;
use num_bigint::{BigInt, Sign};
use sentio_sdk::core::Context;
use sentio_sdk::entity::ID;
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::{EthEvent, EthProcessor, EventFilter};
use sentio_sdk::eth::{EthEventHandler, EventMarker, Log};
use sentio_sdk::{async_trait, EntityStore};
use tracing::{debug, warn};

/// keccak256("Transfer(address,address,uint256)")
pub const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

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

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
        function name() external view returns (string);
    }

    /// Pre-standard tokens (MKR, SAI, ...) return `bytes32` for symbol/name.
    #[sol(rpc)]
    interface IERC20Bytes32 {
        function symbol() external view returns (bytes32);
        function name() external view returns (bytes32);
    }
}

/// Storage limits of the platform's `BigDecimal!` / `BigInt!` columns. They depend
/// on the project's entity schema version (project variable
/// `SENTIO_ENTITY_SCHEMA_VERSION`, a bit set; driver `BuildFeatures`):
/// bit 8 → BigDecimal is `Decimal512(60)` instead of `Decimal256(30)`,
/// bit 4 → BigInt is `Int256` instead of the `[-2^256, 2^256-1]` tuple encoding.
/// Values outside these ranges fail the whole binding (driver `check_value.go`).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnLimits {
    /// Fractional digits kept by the BigDecimal column
    pub decimal_scale: i64,
    /// Largest |value| the BigDecimal column accepts
    pub decimal_max: BigDecimal,
    /// BigInt column range (inclusive)
    pub bigint_min: BigInt,
    pub bigint_max: BigInt,
}

impl ColumnLimits {
    pub fn for_schema_version(version: u32) -> Self {
        let (precision, decimal_scale) = if version & 8 != 0 { (154u32, 60i64) } else { (76u32, 30i64) };
        let decimal_max = BigDecimal::new(BigInt::from(10u32).pow(precision) - 1, decimal_scale);
        let (bigint_min, bigint_max) = if version & 4 != 0 {
            (-(BigInt::from(1u32) << 255u32), (BigInt::from(1u32) << 255u32) - 1)
        } else {
            (-(BigInt::from(1u32) << 256u32), (BigInt::from(1u32) << 256u32) - 1)
        };
        Self { decimal_scale, decimal_max, bigint_min, bigint_max }
    }

    /// Limits for this deployment: project variables reach the processor as env vars.
    pub fn from_env() -> Self {
        let version = std::env::var("SENTIO_ENTITY_SCHEMA_VERSION")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0);
        Self::for_schema_version(version)
    }

    pub fn fits_bigdecimal(&self, v: &BigDecimal) -> bool {
        v.abs() <= self.decimal_max
    }

    pub fn fits_bigint(&self, v: &BigInt) -> bool {
        *v >= self.bigint_min && *v <= self.bigint_max
    }

    /// Round to the column's scale when the value carries more fractional digits.
    pub fn round(&self, v: BigDecimal) -> BigDecimal {
        if v.fractional_digit_count() > self.decimal_scale {
            v.with_scale_round(self.decimal_scale, bigdecimal::RoundingMode::HalfEven)
        } else {
            v
        }
    }
}

/// Decode a `bytes32` symbol/name: UTF-8 padded with trailing NULs.
pub fn bytes32_to_string(raw: alloy::primitives::FixedBytes<32>) -> String {
    String::from_utf8_lossy(raw.as_slice()).trim_end_matches('\0').to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenInfo {
    pub decimals: u8,
    pub symbol: String,
    pub name: String,
}

impl Default for TokenInfo {
    /// What a token looks like when its metadata cannot be read.
    fn default() -> Self {
        Self { decimals: 0, symbol: "unknown".to_string(), name: "unknown".to_string() }
    }
}

pub struct Erc20TransferProcessor {
    chain_id: String,
    start_block: u64,
    name: String,
    rpc: Option<RootProvider>,
    /// (chain, token) pairs whose Token row is already written, so metadata is read
    /// over RPC and upserted once per token instead of once per transfer.
    token_cache: Cache<String, TokenInfo>,
    limits: ColumnLimits,
}

impl Erc20TransferProcessor {
    pub fn new(chain_id: &str, start_block: u64, chains: &ChainsConfig) -> Self {
        let rpc = chains.rpc_url(chain_id).and_then(|url| match url.parse() {
            Ok(url) => Some(RootProvider::new(RpcClient::new_http(url))),
            Err(e) => {
                eprintln!("invalid RPC url for chain {}: {}", chain_id, e);
                None
            }
        });
        Self {
            chain_id: chain_id.to_string(),
            start_block,
            name: format!("ERC20 transfers (chain {})", chain_id),
            rpc,
            token_cache: Cache::new(100_000),
            limits: ColumnLimits::from_env(),
        }
    }

    pub fn with_column_limits(mut self, limits: ColumnLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn has_rpc(&self) -> bool {
        self.rpc.is_some()
    }

    async fn fetch_token_info(&self, token: &str) -> anyhow::Result<TokenInfo> {
        let provider = self.rpc.as_ref().ok_or_else(|| anyhow::anyhow!("no RPC endpoint configured"))?;
        let address = token.parse::<Address>()?;
        let contract = IERC20::new(address, provider.clone());
        // Bind the call builders first: `call()` borrows them for the future's lifetime.
        let (decimals, symbol, name) = (contract.decimals(), contract.symbol(), contract.name());
        let (decimals, symbol, name) = tokio::join!(decimals.call(), symbol.call(), name.call());
        // Without decimals there is no usable metadata at all.
        let decimals = decimals?;

        // `string` decoding fails on bytes32 tokens; retry with the legacy ABI.
        let legacy = IERC20Bytes32::new(address, provider.clone());
        let symbol = match symbol {
            Ok(s) => s,
            Err(_) => bytes32_to_string(legacy.symbol().call().await?),
        };
        let name = match name {
            Ok(s) => s,
            Err(_) => bytes32_to_string(legacy.name().call().await?),
        };
        Ok(TokenInfo { decimals, symbol, name })
    }

    /// The wildcard binding makes the context address literally `"*"`, so the token
    /// address has to come from the log itself.
    async fn get_or_create_token(&self, ctx: &EthContext, token: &str) -> TokenInfo {
        let id = format!("{}-{}", self.chain_id, token);
        if let Some(info) = self.token_cache.get(&id) {
            return info;
        }

        let info = self.fetch_token_info(token).await.unwrap_or_else(|e| {
            warn!("failed to read token metadata for {} on {}: {}", token, self.chain_id, e);
            TokenInfo::default()
        });

        let entity = TokenBuilder::default()
            .id(ID::from(id.clone()))
            .chain(self.chain_id.clone())
            .address(token.to_string())
            .symbol(info.symbol.clone())
            .name(info.name.clone())
            .decimals(info.decimals as i32)
            .build()
            .expect("Token entity");
        match ctx.store().upsert(&entity).await {
            Ok(()) => self.token_cache.insert(id, info.clone()),
            Err(e) => warn!("failed to save token {}: {}", id, e),
        }
        info
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
    async fn on_event(&self, event: EthEvent, ctx: EthContext) {
        let Some((from, to, value)) = decode_transfer(&event.log) else {
            debug!("skipping non-ERC20 Transfer log {}:{}", ctx.transaction_hash(), ctx.log_index());
            return;
        };
        let token = alloy::hex::encode_prefixed(event.log.address());
        let info = self.get_or_create_token(&ctx, &token).await;

        let value_raw = BigInt::from_bytes_be(Sign::Plus, &value.to_be_bytes::<32>());
        // decimals 0 also means "metadata unavailable", in which case the raw value stands.
        let amount = self.limits.round(if info.decimals > 0 {
            BigDecimal::new(value_raw.clone(), info.decimals as i64)
        } else {
            BigDecimal::from(value_raw.clone())
        });

        // Values the platform's columns cannot hold would fail the whole binding;
        // skip the row for such (invariably junk) tokens.
        if !self.limits.fits_bigdecimal(&amount) || !self.limits.fits_bigint(&value_raw) {
            warn!(
                "skipping transfer {}:{} of {}: value {} exceeds the BigDecimal/BigInt column range",
                ctx.transaction_hash(), ctx.log_index(), token, value_raw
            );
            return;
        }

        let transfer = TransferBuilder::default()
            .id(ID::from(format!("{}-{}-{}", self.chain_id, ctx.transaction_hash(), ctx.log_index())))
            .chain(self.chain_id.clone())
            .token_id(ID::from(format!("{}-{}", self.chain_id, token)))
            .token_address(token)
            .from(alloy::hex::encode_prefixed(from))
            .to(alloy::hex::encode_prefixed(to))
            .value(amount)
            .value_raw(value_raw)
            .block_number(ctx.block_number() as i32)
            .timestamp(ctx.timestamp())
            .tx_hash(ctx.transaction_hash())
            .log_index(ctx.log_index())
            .build()
            .expect("Transfer entity");
        if let Err(e) = ctx.store().upsert(&transfer).await {
            warn!("failed to save transfer {}: {}", transfer.id, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_column_limits_match_decimal256_and_the_tuple_bigint() {
        let limits = ColumnLimits::for_schema_version(0);
        // 65 integer digits: the value from ERR320 in production.
        let junk: BigDecimal = "19272561691502883147561569842966314044707217750399251012769692253.235345185".parse().unwrap();
        assert!(!limits.fits_bigdecimal(&junk));
        // (10^76 - 1) / 10^30: 46 integer digits and 30 fractional digits, all nines.
        let max: BigDecimal = format!("{}.{}", "9".repeat(46), "9".repeat(30)).parse().unwrap();
        assert!(limits.fits_bigdecimal(&max));
        assert!(!limits.fits_bigdecimal(&(max + BigDecimal::from(1u32))));
        assert!(limits.fits_bigdecimal(&"-1000000000000000000".parse().unwrap()));

        // Without bit 4 the BigInt column holds [-2^256, 2^256-1]: any uint256 fits.
        let u256_max = BigInt::from_bytes_be(Sign::Plus, &U256::MAX.to_be_bytes::<32>());
        assert!(limits.fits_bigint(&u256_max));
        assert!(!limits.fits_bigint(&(u256_max + 1)));
        assert_eq!(limits.round("1.5".parse().unwrap()), "1.5".parse::<BigDecimal>().unwrap());
        assert_eq!(limits.round(BigDecimal::new(BigInt::from(15u32), 31)).fractional_digit_count(), 30);
    }

    #[test]
    fn schema_version_8_enables_decimal512_and_4_enables_int256() {
        let v8 = ColumnLimits::for_schema_version(8);
        let junk: BigDecimal = "19272561691502883147561569842966314044707217750399251012769692253.235345185".parse().unwrap();
        assert!(v8.fits_bigdecimal(&junk), "Decimal512(60) holds 94 integer digits");
        let max: BigDecimal = format!("{}.{}", "9".repeat(94), "9".repeat(60)).parse().unwrap();
        assert!(v8.fits_bigdecimal(&max));
        assert!(!v8.fits_bigdecimal(&(max + BigDecimal::from(1u32))));
        assert_eq!(v8.decimal_scale, 60);

        let v4 = ColumnLimits::for_schema_version(4);
        let two_pow_255 = BigInt::from(1u32) << 255u32;
        assert!(v4.fits_bigint(&(two_pow_255.clone() - 1)));
        assert!(!v4.fits_bigint(&two_pow_255));
        assert!(v4.fits_bigint(&-two_pow_255));
    }

    #[test]
    fn bytes32_symbols_decode_without_padding() {
        // "MKR" as returned by 0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2
        let raw: alloy::primitives::FixedBytes<32> =
            "0x4d4b520000000000000000000000000000000000000000000000000000000000".parse().unwrap();
        assert_eq!(bytes32_to_string(raw), "MKR");
        assert_eq!(bytes32_to_string(alloy::primitives::FixedBytes::ZERO), "");
    }
}
