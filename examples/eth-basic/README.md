# Multichain ERC20 Transfer Processor

For every chain in `CHAINS` this processor binds a wildcard (`address = "*"`) ERC20
`Transfer` handler and emits one `Transfer` **event log** per transfer with:

| attribute | value |
|---|---|
| `chain` | chain id |
| `token` | the emitting contract, i.e. the token address, straight from the log |
| `from`, `to` | decoded indexed topics |
| `value` | raw `uint256` amount as `BigInt` (no `decimals` scaling); only when it fits `Int256` |
| `value_str` | the raw amount as text, only for the rare amounts ≥ 2^255 that `Int256` cannot hold |
| `block_number`, `tx_hash`, `log_index` | log position |

The distinct id is `chainId-txHash-logIndex`.

There are deliberately **no RPC calls and no entities**: an earlier version read
`decimals`/`symbol`/`name` per token over RPC and upserted `Token`/`Transfer` rows,
which made the driver spend more than half its time committing entity batches to
ClickHouse while every handler waited on the store lock. Event logs are appended in
bulk by the driver and need no existence checks, so the handler is fire-and-forget.

Logs that share the `Transfer` topic but are not ERC20 transfers (ERC721, tokens
that don't index `from`/`to`) are skipped before any work is done.

## Layout

| File | Purpose |
|---|---|
| `src/processor.rs` | `Erc20TransferProcessor`, the `CHAINS` list, transfer decoding, event emission |
| `tests/processor_test.rs` | config shape, emitted events, skip logic |

## Running

```bash
cargo run --bin eth-basic -- --port 4000
```

Other server flags: `--host`, `--debug`, `--process-binding-timeout`, `--worker <n>`
(listen on `n` consecutive ports; the platform passes this when the processor was
uploaded with `--num-workers n`). `--chains-config` is accepted and ignored.

## Uploading

```bash
cargo sentio upload --path examples/eth-basic --name <owner>/<project> --num-workers 8
```

`--num-workers n`: the platform starts the binary with `--worker=n` and the driver
spreads its streams over `n` ports.

## Adding chains

Uncomment or add entries in `CHAINS` (`src/processor.rs`). Start blocks are
deliberately approximate — a wildcard ERC20 processor from genesis is very
expensive, so tune them per chain before uploading.

## Tests

```bash
cargo test -p eth-basic
```
