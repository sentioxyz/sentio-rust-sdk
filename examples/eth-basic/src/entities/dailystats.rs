#![allow(non_snake_case)]

//! Generated entity: DailyStats

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Timeseries entity - optimized for time-ordered data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct DailyStats {
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    #[serde(rename = "totalVolume")]
    total_volume: BigDecimal,
    id: Int8,
    #[serde(rename = "contractsActive")]
    contracts_active: BigInt,
    #[serde(rename = "uniqueUsers")]
    unique_users: BigInt,
    timestamp: Timestamp,
}



impl Entity for DailyStats {
    type Id = i64;
    const TABLE_NAME: &'static str = "dailystats";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl DailyStats {
}