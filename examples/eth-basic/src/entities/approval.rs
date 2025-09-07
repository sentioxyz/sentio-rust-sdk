#![allow(non_snake_case)]

//! Generated entity: Approval

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Entity: Approval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Approval {
    contract: String,
    id: ID,
    #[serde(rename = "blockNumber")]
    block_number: BigInt,
    value: BigDecimal,
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    owner: String,
    spender: String,
    #[serde(rename = "logIndex")]
    log_index: i32,
    timestamp: Timestamp,
}



impl Entity for Approval {
    type Id = ID;
    const TABLE_NAME: &'static str = "approval";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Approval {
}