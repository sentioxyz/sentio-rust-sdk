#![allow(non_snake_case)]

//! Generated entity: Approval

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Entity: Approval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Approval {
    spender: String,
    timestamp: Timestamp,
    contract: String,
    value: BigDecimal,
    owner: String,
    #[serde(rename = "blockNumber")]
    block_number: BigInt,
    id: ID,
    #[serde(rename = "logIndex")]
    log_index: i32,
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
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