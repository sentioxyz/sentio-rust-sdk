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
    timestamp: Timestamp,
    id: ID,
    contract: String,
    #[serde(rename = "logIndex")]
    log_index: i32,
    from: String,
    #[serde(rename = "tokenContract")]
    #[builder(default)]
    token_contract_id: Option<ID>,
    to: String,
    #[serde(rename = "blockNumber")]
    block_number: BigInt,
    value: BigDecimal,
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
}



impl Entity for Transfer {
    type Id = ID;
    const TABLE_NAME: &'static str = "transfer";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Transfer {
    /// Get tokenContract relation
    pub async fn token_contract(&self) -> EntityResult<Option<TokenContract>> {
        if let Some(id) = &self.token_contract_id {
            let id = <TokenContract as Entity>::Id::from_string(&id.to_string())?;
            Ok(TokenContract::get(&id).await?)
        } else {
            Ok(None)
        }
    }
}