use sentio_sdk::core::Context;
use crate::generated::entities::TransferBuilder;
use sentio_sdk::eth::context::EthContext;
use sentio_sdk::eth::eth_processor::*;
use sentio_sdk::eth::{EthEventHandler, EventMarker};
use sentio_sdk::{async_trait, EntityStore};
use sentio_sdk::entity::{BigInt, BigDecimal, Timestamp, ID, Entity};
use std::collections::HashMap;

#[derive(Clone)]
pub struct MyEthProcessor {
    address: String,
    chain_id: String,
    name: String,
 }

impl Default for MyEthProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl MyEthProcessor {
    pub fn new() -> Self {
        Self {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            chain_id: "1".to_string(),
            name: "Sentio ETH + Entity Framework Demo".to_string(),
         }
    }
}

impl EthProcessor for MyEthProcessor {
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

// Define different event marker types for the same processor
pub struct TransferEvent;
pub struct ApprovalEvent;

impl EventMarker for TransferEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            topics: vec!["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string()], // Transfer event topic
        }]
    }
}

impl EventMarker for ApprovalEvent {
    fn filter() -> Vec<EventFilter> {
        vec![EventFilter {
            address: None,
            address_type: None,
            topics: vec!["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925".to_string()], // Approval event topic
        }]
    }
}

#[async_trait]
impl EthEventHandler<TransferEvent> for MyEthProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("🔄 Processing TRANSFER event from contract: {:?} on chain: {}",
                 event.log.address, ctx.chain_id());

        println!("Transfer event details - Block: {}, Transaction: {:?}, Log Index: {}",
                 event.log.block_number.unwrap_or_default(),
                 event.log.transaction_hash,
                 event.log.log_index.unwrap_or_default()
        );

        // Extract transfer data from event logs (simplified version)
        let from_address = if event.log.topics.len() > 1 {
            format!("0x{:x}", event.log.topics[1])
        } else {
            "0x0000000000000000000000000000000000000000".to_string()
        };
        
        let to_address = if event.log.topics.len() > 2 {
            format!("0x{:x}", event.log.topics[2])
        } else {
            "0x0000000000000000000000000000000000000000".to_string()
        };

        // Parse value from event data (simplified - real implementation would decode properly)
        let value = if !event.log.data.is_empty() {
            BigDecimal::from(event.log.data.len() as u64) // Placeholder calculation
        } else {
            BigDecimal::from(1000) // Default value
        };

        // Determine transfer type for categorization
        let transfer_type = if from_address.ends_with("0000000000000000000000000000000000000000") {
            "mint"
        } else if to_address.ends_with("0000000000000000000000000000000000000000") {
            "burn" 
        } else {
            "transfer"
        };

        // 📝 EVENT LOGGING: Record structured event data
        let transfer_event = sentio_sdk::core::Event::name("Transfer")
            .attr("contract", format!("{:?}", event.log.address))
            .attr("from", from_address.clone())
            .attr("to", to_address.clone())
            .attr("value", value.clone())
            .attr("blockNumber", event.log.block_number.unwrap_or_default().as_u64() as i64)
            .attr("transactionHash", format!("{:?}", event.log.transaction_hash.unwrap_or_default()))
            .attr("type", transfer_type);

        let event_logger = ctx.base_context().event_logger();
        let _ = event_logger.emit(&transfer_event).await; // Use await and ignore result for now

        // 📈 METRICS: Track counters and gauges
        // Counter: Number of transfer events processed
        let total_counter = ctx.base_context().counter("transfer_events_total");
        let _ = total_counter.add(1.0, None).await;
        
        // Counter: Track transfers by type (mint, burn, normal transfer)
        let type_counter = ctx.base_context().counter("transfers_by_type");
        let mut type_labels = HashMap::new();
        type_labels.insert("type".to_string(), transfer_type.to_string());
        let _ = type_counter.add(1.0, Some(type_labels)).await;

        // Gauge: Track current block number
        let block_gauge = ctx.base_context().gauge("latest_block_processed");
        let _ = block_gauge.record(event.log.block_number.unwrap_or_default().as_u64() as f64, None).await;

        // Gauge: Track transfer value (convert to f64 for gauge)
        let value_f64 = value.to_string().parse::<f64>().unwrap_or(0.0);
        let value_gauge = ctx.base_context().gauge("transfer_value");
        let mut value_labels = HashMap::new();
        value_labels.insert("type".to_string(), transfer_type.to_string());
        let _ = value_gauge.record(value_f64, Some(value_labels)).await;

        // 💾 ENTITY STORAGE: Create and store Transfer entity
        let transfer_id = format!("{:?}-{}", 
            event.log.transaction_hash.unwrap_or_default(), 
            event.log.log_index.unwrap_or_default()
        );

        let transfer = TransferBuilder::default()
            .id(ID::from(transfer_id))
            .transaction_hash(format!("{:?}", event.log.transaction_hash.unwrap_or_default()))
            .block_number(BigInt::from(event.log.block_number.unwrap_or_default().as_u64()))
            .log_index(event.log.log_index.unwrap_or_default().as_u32() as i32)
            .contract(format!("{:?}", event.log.address))
            .from(from_address)
            .to(to_address)
            .value(value)
            .timestamp(Timestamp::from_timestamp_millis(ctx.block_number() as i64 * 15000).unwrap_or_default())
            .build()
            .expect("Failed to build transfer entity");

        // Save entity to store
        ctx.store().upsert(&transfer).await.expect("Failed to save transfer entity");
        println!("💾 Saved Transfer entity with ID: {}", transfer.id);

        println!("✅ Transfer event processing completed");
     }
}

#[async_trait]
impl EthEventHandler<ApprovalEvent> for MyEthProcessor {
    async fn on_event(&self, event: EthEvent, mut ctx: EthContext) {
        println!("🔄 Processing APPROVAL event from contract: {:?} on chain: {}",
                 event.log.address, ctx.chain_id());

        // Extract approval data from event logs
        let owner_address = if event.log.topics.len() > 1 {
            format!("0x{:x}", event.log.topics[1])
        } else {
            "0x0000000000000000000000000000000000000000".to_string()
        };
        
        let spender_address = if event.log.topics.len() > 2 {
            format!("0x{:x}", event.log.topics[2])
        } else {
            "0x0000000000000000000000000000000000000000".to_string()
        };

        let allowance_value = if !event.log.data.is_empty() {
            BigDecimal::from(event.log.data.len() as u64)
        } else {
            BigDecimal::from(0)
        };

        // 📝 EVENT LOGGING: Record structured event data for approval
        let approval_event = sentio_sdk::core::Event::name("Approval")
            .attr("contract", format!("{:?}", event.log.address))
            .attr("owner", owner_address.clone())
            .attr("spender", spender_address.clone())
            .attr("value", allowance_value.clone())
            .attr("blockNumber", event.log.block_number.unwrap_or_default().as_u64() as i64);

        let event_logger = ctx.base_context().event_logger();
        let _ = event_logger.emit(&approval_event).await;

        // 📈 METRICS: Track approval-related metrics
        let approval_counter = ctx.base_context().counter("approval_events_total");
        let _ = approval_counter.add(1.0, None).await;
            
        // Track approval values
        let allowance_f64 = allowance_value.to_string().parse::<f64>().unwrap_or(0.0);
        let approval_gauge = ctx.base_context().gauge("approval_value");
        let _ = approval_gauge.record(allowance_f64, None).await;

        println!("✅ Approval event processing completed - Owner: {}, Spender: {}, Value: {}", 
                owner_address, spender_address, allowance_value);
    }
}
