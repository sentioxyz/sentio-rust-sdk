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
    id: ID,
    address: String,
    #[serde(rename = "totalTransferred")]
    total_transferred: BigDecimal,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    #[serde(rename = "lastActive")]
    last_active: Timestamp,
    #[serde(rename = "firstSeen")]
    first_seen: Timestamp,
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
    pub async fn approvals(&self) -> EntityResult<Vec<Approval>> {
        let store = Store::from_current_context().await?;
        let mut options = ListOptions::<Approval>::new();
        options.filters.push(Filter::eq("owner", self.id.clone()));
        Ok(store.list(options).await?)
    }

    /// Get transfers (derived relation)
    pub async fn transfers(&self) -> EntityResult<Vec<Transfer>> {
        let store = Store::from_current_context().await?;
        let mut options = ListOptions::<Transfer>::new();
        options.filters.push(Filter::eq("from", self.id.clone()));
        Ok(store.list(options).await?)
    }

    /// Get received (derived relation)
    pub async fn received(&self) -> EntityResult<Vec<Transfer>> {
        let store = Store::from_current_context().await?;
        let mut options = ListOptions::<Transfer>::new();
        options.filters.push(Filter::eq("to", self.id.clone()));
        Ok(store.list(options).await?)
    }
}