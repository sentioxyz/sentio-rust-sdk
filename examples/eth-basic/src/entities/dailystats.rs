#![allow(non_snake_case)]

//! Generated entity: DailyStats

// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};



/// Timeseries entity - optimized for time-ordered data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct DailyStats {
    #[serde(rename = "totalVolume")]
    total_volume: BigDecimal,
    #[serde(rename = "contractsActive")]
    contracts_active: BigInt,
    timestamp: Timestamp,
    #[serde(rename = "transferCount")]
    transfer_count: BigInt,
    id: Int8,
    #[serde(rename = "uniqueUsers")]
    unique_users: BigInt,
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