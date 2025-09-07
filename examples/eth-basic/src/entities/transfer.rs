#![allow(non_snake_case)]

//! Generated entity: Transfer

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};
use crate::entities::TokenContract;



/// Relation field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Transfer {
    from: String,
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    value: BigDecimal,
    id: ID,
    #[serde(rename = "blockNumber")]
    block_number: BigInt,
    #[serde(rename = "logIndex")]
    log_index: i32,
    to: String,
    contract: String,
    timestamp: Timestamp,
    #[serde(rename = "tokenContract")]
    token_contract: Option<TokenContract>,
}



impl Entity for Transfer {
    type Id = ID;
    const TABLE_NAME: &'static str = "transfer";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Transfer {
    /// Set tokenContract relation
    pub fn set_token_contract(&mut self, token_contract: Option<TokenContract>) {
        self.token_contract = token_contract;
    }

    /// Clear tokenContract relation
    pub fn clear_token_contract(&mut self) {
        self.token_contract = None;
    }
}