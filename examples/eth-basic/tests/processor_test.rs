//! Integration tests for the multichain ERC20 transfer processor.
//!
//! The processor makes no RPC calls and writes no entities: every ERC20 `Transfer`
//! log becomes one `Transfer` event log carrying the token address from the log.

use eth_basic::{decode_transfer, fits_int256, Erc20TransferProcessor, TransferEvent, CHAINS, TRANSFER_EVENT, TRANSFER_TOPIC};
use sentio_sdk::core::AttributeValue;
use sentio_sdk::eth::eth_processor::EthProcessor;
use sentio_sdk::testing::{addresses, chain_ids, mock_log, mock_transfer_log, TestProcessorServer};

const ONE_TOKEN: &str = "1000000000000000000";
const TX_HASH: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BLOCK_HASH: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

async fn setup() -> TestProcessorServer {
    let mut server = TestProcessorServer::new();
    for (chain_id, start_block) in CHAINS {
        Erc20TransferProcessor::new(chain_id, *start_block)
            .configure_event::<TransferEvent>(None)
            .bind(&server);
    }
    server.start().await.expect("start test server");
    server
}

fn string_attr(attrs: &std::collections::HashMap<String, AttributeValue>, key: &str) -> String {
    match attrs.get(key) {
        Some(AttributeValue::String(s)) => s.clone(),
        other => panic!("attribute {} should be a string, got {:?}", key, other),
    }
}

#[tokio::test]
async fn binds_a_wildcard_transfer_handler_per_chain() {
    let server = setup().await;
    let config = server.get_config().await;

    assert_eq!(config.contract_configs.len(), CHAINS.len());
    for (cfg, (chain_id, start_block)) in config.contract_configs.iter().zip(CHAINS) {
        let contract = cfg.contract.as_ref().expect("contract info");
        assert_eq!(contract.address, "*");
        assert_eq!(contract.chain_id, *chain_id);
        assert_eq!(cfg.start_block, *start_block);
        assert_eq!(cfg.log_configs.len(), 1, "one Transfer handler per chain");
        for filter in &cfg.log_configs[0].filters {
            assert_eq!(filter.topics[0].hashes, vec![TRANSFER_TOPIC.to_string()]);
        }
    }
}

#[tokio::test]
async fn emits_one_event_log_per_transfer() {
    let server = setup().await;
    let eth = server.eth();

    let log = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::ZERO, addresses::TEST_ADDRESS_1, ONE_TOKEN);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    assert!(result.counters.is_empty() && result.gauges.is_empty(), "event logs only, no metrics");
    assert_eq!(result.db.get_table_count("Transfer").await, 0, "no entity rows");
    assert_eq!(result.db.get_table_count("Token").await, 0, "no token metadata rows");

    assert_eq!(result.events.len(), 1);
    let event = &result.events[0];
    assert_eq!(event.name, TRANSFER_EVENT);
    let attrs = &event.attributes;
    assert_eq!(string_attr(attrs, "token"), addresses::USDC_ETHEREUM.to_lowercase(), "token address taken from the log");
    assert_eq!(string_attr(attrs, "from"), addresses::ZERO.to_lowercase());
    assert_eq!(string_attr(attrs, "to"), addresses::TEST_ADDRESS_1.to_lowercase());
    assert_eq!(string_attr(attrs, "chain"), chain_ids::ETHEREUM.to_string());
    match attrs.get("value") {
        Some(AttributeValue::BigInt(v)) => assert_eq!(v.to_string(), ONE_TOKEN),
        other => panic!("value should be a BigInt attribute, got {:?}", other),
    }
    assert!(matches!(attrs.get("block_number"), Some(AttributeValue::Integer(_))));
    assert!(matches!(attrs.get("log_index"), Some(AttributeValue::Integer(_))));
    assert!(attrs.contains_key("tx_hash"));
}

#[tokio::test]
async fn every_transfer_is_its_own_event() {
    let server = setup().await;
    let eth = server.eth();

    let first = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::ZERO, addresses::TEST_ADDRESS_1, ONE_TOKEN);
    let mut second = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, ONE_TOKEN);
    second.log_index = Some(2);

    assert_eq!(eth.test_log(first, Some(chain_ids::ETHEREUM)).await.events.len(), 1);
    assert_eq!(eth.test_log(second, Some(chain_ids::ETHEREUM)).await.events.len(), 1, "same token still emits");
}

#[tokio::test]
async fn huge_values_are_recorded_raw() {
    let server = setup().await;
    let eth = server.eth();

    // ~1.9e73 raw: used to overflow the Decimal256 entity column; as a BigInt event
    // attribute it is stored as-is.
    let junk = "19272561691502883147561569842966314044707217750399251012769692253235345185";
    let log = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, junk);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    assert_eq!(result.events.len(), 1);
    match result.events[0].attributes.get("value") {
        Some(AttributeValue::BigInt(v)) => assert_eq!(v.to_string(), junk),
        other => panic!("value should be a BigInt attribute, got {:?}", other),
    }
}

#[tokio::test]
async fn values_beyond_int256_are_kept_as_text() {
    let server = setup().await;
    let eth = server.eth();

    // 2^256 - 1000: the ERR321 value. Int256 tops out at 2^255 - 1, so it cannot be a
    // BigInt attribute without failing the binding.
    let huge = "115792089237316195423570985008687907853269984665640564039457584007913129638936";
    let log = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, huge);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    assert_eq!(result.events.len(), 1, "the transfer is still recorded");
    let attrs = &result.events[0].attributes;
    assert!(!attrs.contains_key("value"), "no Int256-typed value for an out-of-range amount");
    assert_eq!(string_attr(attrs, "value_str"), huge);
}

#[test]
fn int256_bound_is_exclusive_at_two_pow_255() {
    use num_bigint::BigInt;
    let two_pow_255 = BigInt::from(1u8) << 255u32;
    assert!(fits_int256(&(two_pow_255.clone() - 1)));
    assert!(!fits_int256(&two_pow_255));
    assert!(fits_int256(&-two_pow_255.clone()));
    assert!(!fits_int256(&(-two_pow_255 - 1)));
}

#[tokio::test]
async fn non_erc20_transfer_logs_are_skipped() {
    let server = setup().await;
    let eth = server.eth();

    // Same topic0 but no indexed from/to: not an ERC20 Transfer, must be ignored.
    let log = mock_log(&[TRANSFER_TOPIC], "0x", TX_HASH, BLOCK_HASH, 1, 0);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    assert!(result.events.is_empty());
}

#[test]
fn decode_transfer_rejects_unexpected_shapes() {
    let ok = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, "42");
    let (from, to, value) = decode_transfer(&ok).expect("well-formed transfer");
    assert_eq!(alloy_hex(from), addresses::TEST_ADDRESS_1);
    assert_eq!(alloy_hex(to), addresses::TEST_ADDRESS_2);
    assert_eq!(value.to_string(), "42");

    // ERC721-style: token id indexed as a 4th topic, empty data.
    let nft = mock_log(&[TRANSFER_TOPIC, &pad(addresses::TEST_ADDRESS_1), &pad(addresses::TEST_ADDRESS_2), &pad("0x01")], "0x", TX_HASH, BLOCK_HASH, 1, 0);
    assert!(decode_transfer(&nft).is_none());
}

fn pad(hex: &str) -> String {
    format!("0x{:0>64}", hex.trim_start_matches("0x"))
}

fn alloy_hex(addr: alloy::primitives::Address) -> String {
    alloy::hex::encode_prefixed(addr)
}
