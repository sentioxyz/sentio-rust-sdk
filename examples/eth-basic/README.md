# Multichain ERC20 Transfer Processor

Rust port of the TypeScript `erc20-transfer-multichain` processor. For every chain
in `CHAINS` it binds a wildcard (`address = "*"`) ERC20 `Transfer` handler and, per
transfer:

- reads the token's `decimals`/`symbol`/`name` over RPC **once per token** (LRU
  cached, 100k entries) and upserts a `Token` row keyed `chainId-address`;
- upserts an immutable `Transfer` row keyed `chainId-txHash-logIndex`, with the
  value scaled by `decimals` (raw when metadata is unavailable).

Logs that share the `Transfer` topic but are not ERC20 transfers (ERC721, tokens
that don't index `from`/`to`) are skipped before any work is done.

## Layout

| File | Purpose |
|---|---|
| `src/processor.rs` | `Erc20TransferProcessor`, the `CHAINS` list, transfer decoding |
| `src/chains_config.rs` | reads the platform's `--chains-config=<json>` to find RPC endpoints |
| `schema.graphql` | `Token` / `Transfer` entities (code generated into `src/generated/` by `build.rs`) |
| `tests/processor_test.rs` | config shape, entities, skip logic |

## Running

```bash
cargo run --bin eth-basic -- --port 4000 --chains-config=/path/to/chains-config.json
```

The Sentio platform passes `--chains-config` automatically. Locally you can also set
`TEST_ENDPOINT_<chainId>` (e.g. `TEST_ENDPOINT_1=https://eth.llamarpc.com`), which
takes precedence over the file. Without any endpoint the processor still runs;
token metadata is recorded as `unknown` with 0 decimals.

Other server flags: `--host`, `--debug`, `--process-binding-timeout`, `--worker <n>`
(listen on `n` consecutive ports; the platform passes this when the processor was
uploaded with `--num-workers n`).

## Uploading

```bash
cargo sentio upload --path examples/eth-basic --name <owner>/<project> \
  --num-workers 8 --variable SENTIO_ENTITY_SCHEMA_VERSION=8
```

- `--num-workers n`: the platform starts the binary with `--worker=n` and the driver
  spreads its streams over `n` ports.
- `--variable KEY=VALUE`: project variables, saved before the processor version is
  created and injected into the pod as env vars. `SENTIO_ENTITY_SCHEMA_VERSION` is a
  bit set read by the platform when creating the version: bit 8 stores `BigDecimal!`
  as `Decimal512(60)` (|v| < 10^94) instead of `Decimal256(30)` (|v| < 10^46), bit 4
  stores `BigInt!` as `Int256`. The processor reads the same env var to size its
  column-range guard (`ColumnLimits`); transfers that still don't fit are skipped
  with a warning.

## Adding chains

Uncomment or add entries in `CHAINS` (`src/processor.rs`). Start blocks are
deliberately approximate — a wildcard ERC20 processor from genesis is very
expensive, so tune them per chain before uploading.

## Tests

```bash
cargo test -p eth-basic
```

Tests run without RPC, so they exercise the "metadata unavailable" path.
