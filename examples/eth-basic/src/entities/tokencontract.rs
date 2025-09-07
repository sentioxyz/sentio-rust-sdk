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
    decimals: i32,
    address: String,
    #[serde(rename = "holderCount")]
    holder_count: BigInt,
    #[serde(rename = "createdAt")]
    created_at: Timestamp,
    symbol: String,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    id: ID,
    name: String,
    #[serde(rename = "totalSupply")]
    total_supply: BigDecimal,
    #[serde(rename = "relatedTransfers")]
    related_transfers: Vec<Transfer>,
}



impl Entity for TokenContract {
    type Id = ID;
    const TABLE_NAME: &'static str = "tokencontract";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl TokenContract {
    /// Set relatedTransfers relation collection
    pub fn set_related_transfers(&mut self, related_transfers: Vec<Transfer>) {
        self.related_transfers = related_transfers;
    }

    /// Add single item to relatedTransfers collection
    pub fn add_related_transfer(&mut self, item: Transfer) {
        self.related_transfers.push(item);
    }

    /// Remove item from relatedTransfers collection by ID
    pub fn remove_related_transfer(&mut self, id: &<Self as Entity>::Id) {
        self.related_transfers.retain(|item| item.id() != id);
    }
}