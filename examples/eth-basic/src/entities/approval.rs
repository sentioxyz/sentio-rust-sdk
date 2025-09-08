#![allow(non_snake_case)]

//! Generated entity: Approval

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Entity: Approval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Approval {
    owner: String,
    value: BigDecimal,
    timestamp: Timestamp,
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    #[serde(rename = "logIndex")]
    log_index: i32,
    spender: String,
    #[serde(rename = "blockNumber")]
    block_number: BigInt,
    contract: String,
    id: ID,
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