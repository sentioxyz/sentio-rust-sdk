#![allow(non_snake_case)]

//! Generated entity: Account

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};
use crate::entities::{Transfer, Approval};



/// Entity: Account
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Account {
    #[serde(rename = "totalTransferred")]
    total_transferred: BigDecimal,
    #[serde(rename = "firstSeen")]
    first_seen: Timestamp,
    #[serde(rename = "lastActive")]
    last_active: Timestamp,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    id: ID,
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
    /// Get transfers (derived relation)
    pub async fn transfers(&self) -> EntityResult<Vec<Transfer>> {
        Ok(Transfer::find().where_eq("from", self.id.clone()).list().await?)
    }

    /// Get received (derived relation)
    pub async fn received(&self) -> EntityResult<Vec<Transfer>> {
        Ok(Transfer::find().where_eq("to", self.id.clone()).list().await?)
    }

    /// Get approvals (derived relation)
    pub async fn approvals(&self) -> EntityResult<Vec<Approval>> {
        Ok(Approval::find().where_eq("owner", self.id.clone()).list().await?)
    }
}