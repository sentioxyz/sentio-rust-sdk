//! Integration tests for the multichain ERC20 transfer processor.
//!
//! No RPC endpoint is configured here, so token metadata falls back to
//! `unknown`/0 decimals exactly like the TypeScript processor's catch branch.

use eth_basic::chains_config::ChainsConfig;
use eth_basic::{decode_transfer, ColumnLimits, Erc20TransferProcessor, TransferEvent, CHAINS, TRANSFER_TOPIC};
use sentio_sdk::eth::eth_processor::EthProcessor;
use sentio_sdk::testing::{addresses, chain_ids, mock_log, mock_transfer_log, TestProcessorServer};

const ONE_TOKEN: &str = "1000000000000000000";
const TX_HASH: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BLOCK_HASH: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

async fn setup() -> TestProcessorServer {
    let mut server = TestProcessorServer::new();
    let chains = ChainsConfig::default();
    for (chain_id, start_block) in CHAINS {
        Erc20TransferProcessor::new(chain_id, *start_block, &chains)
            .configure_event::<TransferEvent>(None)
            .bind(&server);
    }
    server.start().await.expect("start test server");
    server
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
async fn records_metrics_and_entities_for_a_transfer() {
    let server = setup().await;
    let eth = server.eth();
    let token = addresses::USDC_ETHEREUM.to_lowercase();

    let log = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::ZERO, addresses::TEST_ADDRESS_1, ONE_TOKEN);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    let counter = result.counters.iter().find(|c| c.name == "erc20_transfers").expect("erc20_transfers counter");
    assert_eq!(counter.value, 1.0);
    assert_eq!(counter.labels["chain"], "1");
    assert_eq!(counter.labels["token"], token);
    assert_eq!(counter.labels["symbol"], "unknown");

    // Without metadata the raw value stands (decimals 0), carried as an exact BigDecimal.
    let gauge = result.gauges.iter().find(|g| g.name == "erc20_transfer_amount").expect("erc20_transfer_amount gauge");
    assert_eq!(gauge.value, 1e18);
    assert_eq!(gauge.labels["symbol"], "unknown");

    assert!(result.db.entity_exists("Token", &format!("1-{}", token)).await, "Token row keyed by chain-address");
    assert_eq!(result.db.get_table_count("Transfer").await, 1);

    // The platform only accepts BigInt/BigDecimal columns in their dedicated encodings.
    use sentio_sdk::common::rich_value::Value;
    use sentio_sdk::entity::{BigDecimal, BigInt, FromRichValue};
    let transfers = result.db.list_table_entities("Transfer").await;
    let data = transfers[0].data.as_ref().expect("entity data");
    assert!(matches!(data.fields["valueRaw"].value, Some(Value::BigintValue(_))), "{:?}", data.fields["valueRaw"]);
    assert!(matches!(data.fields["value"].value, Some(Value::BigdecimalValue(_))), "{:?}", data.fields["value"]);
    assert_eq!(BigInt::from_rich_value(&data.fields["valueRaw"]).unwrap().to_string(), ONE_TOKEN);
    assert_eq!(BigDecimal::from_rich_value(&data.fields["value"]).unwrap().to_string(), ONE_TOKEN);
}

#[tokio::test]
async fn token_metadata_is_written_once_per_token() {
    let server = setup().await;
    let eth = server.eth();

    let first = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::ZERO, addresses::TEST_ADDRESS_1, ONE_TOKEN);
    let mut second = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, ONE_TOKEN);
    second.log_index = Some(2);

    eth.test_log(first, Some(chain_ids::ETHEREUM)).await;
    let result = eth.test_log(second, Some(chain_ids::ETHEREUM)).await;

    assert_eq!(result.db.get_table_count("Token").await, 1, "same token -> one Token row");
    assert_eq!(result.db.get_table_count("Transfer").await, 2, "distinct log index -> two Transfer rows");
}

#[tokio::test]
async fn transfers_outside_the_column_range_keep_metrics_but_skip_the_row() {
    let server = setup().await;
    let eth = server.eth();

    // ~1.9e73 raw with unknown decimals: exceeds Decimal256(30), the ERR320 case.
    let junk = "19272561691502883147561569842966314044707217750399251012769692253235345185";
    let log = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, junk);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    assert_eq!(result.counters.len(), 1, "counter still recorded");
    assert_eq!(result.db.get_table_count("Token").await, 1, "token metadata still written");
    assert_eq!(result.db.get_table_count("Transfer").await, 0, "row that would fail the binding is skipped");

    // With SENTIO_ENTITY_SCHEMA_VERSION=8 (Decimal512) the same transfer is stored.
    let mut wide = TestProcessorServer::new();
    Erc20TransferProcessor::new("1", 0, &ChainsConfig::default())
        .with_column_limits(ColumnLimits::for_schema_version(8))
        .configure_event::<TransferEvent>(None)
        .bind(&wide);
    wide.start().await.expect("start test server");
    let log = mock_transfer_log(addresses::USDC_ETHEREUM, addresses::TEST_ADDRESS_1, addresses::TEST_ADDRESS_2, junk);
    let result = wide.eth().test_log(log, Some(chain_ids::ETHEREUM)).await;
    assert_eq!(result.db.get_table_count("Transfer").await, 1);
}

#[tokio::test]
async fn non_erc20_transfer_logs_are_skipped() {
    let server = setup().await;
    let eth = server.eth();

    // Same topic0 but no indexed from/to: not an ERC20 Transfer, must be ignored.
    let log = mock_log(&[TRANSFER_TOPIC], "0x", TX_HASH, BLOCK_HASH, 1, 0);
    let result = eth.test_log(log, Some(chain_ids::ETHEREUM)).await;

    assert!(result.counters.is_empty());
    assert!(result.gauges.is_empty());
    assert_eq!(result.db.get_table_count("Transfer").await, 0);
    assert_eq!(result.db.get_table_count("Token").await, 0);
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
