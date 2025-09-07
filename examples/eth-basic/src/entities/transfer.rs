#![allow(non_snake_case)]

//! Generated entity: Transfer

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Entity: Transfer
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Transfer {
    #[serde(rename = "blockNumber")]
    block_number: BigInt,
    contract: String,
    to: String,
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    timestamp: Timestamp,
    value: BigDecimal,
    from: String,
    #[serde(rename = "logIndex")]
    log_index: i32,
    id: ID,
}



impl Entity for Transfer {
    type Id = ID;
    const TABLE_NAME: &'static str = "transfer";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Transfer {
}