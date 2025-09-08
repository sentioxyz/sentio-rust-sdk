#![allow(non_snake_case)]

//! Generated entity: TokenContract

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};
use crate::entities::Transfer;



/// Relation field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct TokenContract {
    #[serde(rename = "holderCount")]
    holder_count: BigInt,
    #[builder(default)]
    symbol: String,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    #[serde(rename = "relatedTransfers")]
    related_transfers_ids: Vec<ID>,
    id: ID,
    #[serde(rename = "totalSupply")]
    #[builder(default)]
    total_supply: BigDecimal,
    #[builder(default)]
    name: String,
    #[serde(rename = "createdAt")]
    created_at: Timestamp,
    address: String,
    #[builder(default)]
    decimals: i32,
}



impl Entity for TokenContract {
    type Id = ID;
    const TABLE_NAME: &'static str = "tokencontract";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl TokenContract {
    /// Get relatedTransfers relation
    pub async fn related_transfers(&self) -> EntityResult<Vec<Transfer>> {
        let ids = self.related_transfers_ids.iter().map(|id| <Transfer as Entity>::Id::from_string(&id.to_string())).collect::<Result<Vec<_>, _>>()?;
        Ok(Transfer::get_many(&ids).await?)
    }
}