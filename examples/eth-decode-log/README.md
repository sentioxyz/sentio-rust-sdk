# Ethereum Log Decoder

This example demonstrates how to decode Ethereum logs using the Sentio SDK. It attempts to decode every Ethereum log by fetching ABI information from Sentio's API when available.

## Features

- Processes all Ethereum logs
- Fetches ABI information from Sentio API based on event signatures and contract addresses
- Caches ABI data for performance
- Gracefully handles logs that cannot be decoded
- Logs both successful decodes and errors

## Configuration

Set these environment variables:

- `SENTIO_HOST`: Sentio API host (default: "https://app.sentio.xyz")
- `CHAIN_ID`: Ethereum chain ID (default: "1" for Ethereum mainnet)

## Running

```bash
cargo run --bin eth-decode-log
```