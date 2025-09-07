#![allow(non_snake_case)]

//! Generated entity: TokenContract

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Entity: TokenContract
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct TokenContract {
    #[serde(rename = "createdAt")]
    created_at: Timestamp,
    name: String,
    address: String,
    symbol: String,
    id: ID,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    decimals: i32,
    #[serde(rename = "holderCount")]
    holder_count: BigInt,
    #[serde(rename = "totalSupply")]
    total_supply: BigDecimal,
}



impl Entity for TokenContract {
    type Id = ID;
    const TABLE_NAME: &'static str = "tokencontract";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl TokenContract {
}