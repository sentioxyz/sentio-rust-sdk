# Sentio Rust Processor

A high-performance blockchain data processing framework written in Rust that enables you to build sophisticated data processors for Ethereum and other EVM-compatible chains. Sentio processors capture, transform, and analyze blockchain data with built-in support for entities, metrics, and structured event logging.

## Overview

Sentio Rust Processor provides a complete toolkit for blockchain data processing:

- **Event-Driven Architecture**: Process blockchain events in real-time with automatic filtering and routing
- **Entity Framework**: Define data schemas using GraphQL and automatically generate Rust structs with relationships
- **Built-in Observability**: Integrated metrics collection and structured event logging
- **Type Safety**: Full Rust type safety with automatic ABI bindings for smart contracts
- **Cloud Integration**: Seamless deployment to Sentio platform with authentication and project management
- **Cross-Platform**: Support for multiple target architectures with cross-compilation

## Requirements

### System Requirements

- **Rust**: Version 1.70.0 or later
- **Target Platform**: Linux x86_64 (for production deployment)
- **Development**: macOS, Linux, or Windows with WSL2

### Dependencies

The framework automatically manages these core dependencies:
- `sentio-sdk`: Core processing framework
- `ethers`: Ethereum blockchain interaction
- `tokio`: Async runtime
- `serde`: Serialization/deserialization
- `tonic`: gRPC communication

## Installation

### Install Sentio CLI

```bash
# Clone the repository
git clone https://github.com/sentioxyz/sentio.git
cd sentio/rust-processor

# Build and install the CLI
cargo install --path cli
```

### Verify Installation

```bash
# Check if sentio CLI is available
cargo sentio --help
```

## Quick Start

### 1. Create a New Project

```bash
# Initialize a new processor project
cargo  sentio init my-processor

# Navigate to the project directory
cd my-processor
```

### 2. Project Structure

Your new project will have this structure:

```
my-processor/
├── Cargo.toml          # Project configuration
├── build.rs            # Build script for code generation
├── schema.graphql      # Entity schema definitions
├── src/
│   ├── main.rs        # Entry point and server setup
│   ├── lib.rs         # Library exports
│   └── processor.rs   # Main processor logic
└── tests/             # Test files
```

### 3. Configure Your Processor

Edit `src/processor.rs` to set your contract address and chain:

```rust
impl MyProcessor {
    pub fn new() -> Self {
        Self {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            chain_id: "1".to_string(), // 1 for Ethereum mainnet
            name: "My Token Processor".to_string(),
        }
    }
}
```

## Authentication & Login

### Login to Sentio Platform

```bash
# Login with browser-based OAuth
cargo sentio auth login

# Or login with API key
cargo sentio auth login --api-key YOUR_API_KEY

# Check authentication status
cargo sentio auth status
```

### Logout

```bash
cargo sentio auth logout
```

## Development Workflow

### Build Your Processor

```bash
# Build the processor (development mode)
cargo sentio build
```

### Upload to Platform

```bash
# Build and upload to Sentio platform
sentio upload

# Override project settings
sentio upload --owner myorg --name myprocessor
```

## Simple Processor Example

Here's a complete example of a minimal ERC-20 transfer processor:

### 1. Define Your Schema (`schema.graphql`)

```graphql
type Transfer @entity {
    id: ID!
    transactionHash: String!
    blockNumber: BigInt!
    timestamp: Timestamp!
    from: String!
    to: String!
    value: BigDecimal!
    contract: String!
}
```

### 2. Implement Your Processor (`src/processor.rs`)

```rust
use sentio_sdk::core::Context;
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::{async_trait, EntityStore};
use sentio_sdk::entity::{BigDecimal, Timestamp, ID};
use crate::generated::entities::TransferBuilder;

#[derive(Clone)]
pub struct TokenProcessor {
    address: String,
    chain_id: String,
    name: String,
}

impl TokenProcessor {
    pub fn new() -> Self {
        Self {
            address: "0xA0b86a33E6441D052c659B0aEeFFE375c5B71bC5".to_string(), // USDT contract
            chain_id: "1".to_string(),
            name: "USDT Transfer Processor".to_string(),
        }
    }
}

impl EthProcessor for TokenProcessor {
    fn address(&self) -> &str { &self.address }
    fn chain_id(&self) -> &str { &self.chain_id }
    fn name(&self) -> &str { &self.name }
}

// Event marker for ERC-20 Transfer events
pub struct TransferEvent;

impl EventMarker for TransferEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            topics: vec![
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string()
            ],
        }]
    }
}

#[async_trait]
impl EthEventHandler<TransferEvent> for TokenProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("Processing Transfer event at block {}", 
                 event.log.block_number.unwrap_or_default());

        // Extract transfer data from event logs
        let from = format!("0x{:x}", event.log.topics[1]);
        let to = format!("0x{:x}", event.log.topics[2]);
        let value = BigDecimal::from(1000); // Simplified - decode from event.log.data in real implementation

        // Create transfer ID
        let transfer_id = format!("{:?}-{}", 
            event.log.transaction_hash.unwrap_or_default(),
            event.log.log_index.unwrap_or_default()
        );

        // Create and save Transfer entity
        let transfer = TransferBuilder::default()
            .id(ID::from(transfer_id))
            .transaction_hash(format!("{:?}", event.log.transaction_hash.unwrap_or_default()))
            .block_number(BigInt::from(event.log.block_number.unwrap_or_default().as_u64()))
            .timestamp(Timestamp::from_timestamp_millis(ctx.block_number() as i64 * 15000).unwrap_or_default())
            .from(from.clone())
            .to(to.clone())
            .value(value.clone())
            .contract(format!("{:?}", event.log.address))
            .build()
            .expect("Failed to build transfer entity");

        // Save to entity store
        ctx.store().upsert(&transfer).await.expect("Failed to save transfer");

        // Emit metrics
        let counter = ctx.base_context().counter("transfers_total");
        let _ = counter.add(1.0, None).await;

        // Log structured event
        let event_log = sentio_sdk::core::Event::name("Transfer")
            .attr("from", from)
            .attr("to", to)
            .attr("value", value);
        
        let _ = ctx.base_context().event_logger().emit(&event_log).await;
        
        println!("Transfer processed successfully");
    }
}
```

### 3. Register Your Processor and Start (`src/main.rs`)

```rust
use sentio_sdk::eth::eth_plugin::EthPlugin;
use sentio_sdk::core::processor_plugin::ProcessorManager;

mod processor;
use processor::{TokenProcessor, TransferEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the processor
    let processor = TokenProcessor::new();
    
    // Create plugin and register event handler
    let mut plugin = EthPlugin::new();
    plugin.register_event_handler::<TokenProcessor, TransferEvent>(processor).await;
    
    // Start the processor server
    let mut manager = ProcessorManager::new();
    manager.add_plugin(Box::new(plugin));
    manager.serve().await?;
    
    Ok(())
}
```

### 4. Build and Test

```bash
# Generate entity code from schema
cargo sentio gen

# Build the processor
cargo sentio build

# Test locally
cargo sentio test
```

## Contributing

Contributions are welcome! Please read our contributing guidelines and submit pull requests to our GitHub repository.