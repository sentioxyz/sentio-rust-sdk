#![allow(non_snake_case)]

//! Generated entity: Account

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};
use crate::entities::{Approval, Transfer};



/// Entity: Account
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Account {
    #[serde(rename = "firstSeen")]
    first_seen: Timestamp,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    #[serde(rename = "lastActive")]
    last_active: Timestamp,
    id: ID,
    #[serde(rename = "totalTransferred")]
    total_transferred: BigDecimal,
    address: String,
}



impl Entity for Account {
    type Id = ID;
    const TABLE_NAME: &'static str = "account";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Account {
    /// Get approvals (derived relation)
    pub async fn approvals(&self, store: Store) -> EntityResult<Vec<Approval>> {
        // TODO: Implement derived field query
        todo!("Derived field queries not yet implemented")
    }

    /// Get received (derived relation)
    pub async fn received(&self, store: Store) -> EntityResult<Vec<Transfer>> {
        // TODO: Implement derived field query
        todo!("Derived field queries not yet implemented")
    }

    /// Get transfers (derived relation)
    pub async fn transfers(&self, store: Store) -> EntityResult<Vec<Transfer>> {
        // TODO: Implement derived field query
        todo!("Derived field queries not yet implemented")
    }
}