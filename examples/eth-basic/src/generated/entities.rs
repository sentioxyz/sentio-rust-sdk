// Generated entities
// This file is auto-generated. Do not edit manually.

use sentio_sdk::entity::*;
use derive_builder::Builder;
use serde::{Serialize, Deserialize};

/// Indexed field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Token {
    pub chain: String,
    pub name: String,
    pub symbol: String,
    pub decimals: i32,
    pub id: ID,
    pub address: String,
}



impl Entity for Token {
    type Id = ID;
    const NAME: &'static str = "Token";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Token {
    /// Get transfers (derived relation)
    pub async fn transfers(&self) -> EntityResult<Vec<Transfer>> {
        Ok(Transfer::find().where_eq("token", self.id.clone()).list().await?)
    }
}

/// Indexed field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
pub struct Transfer {
    /// Relation field
    #[serde(rename = "token")]
    pub token_id: ID,
    pub chain: String,
    pub id: ID,
    pub from: String,
    pub to: String,
    #[serde(rename = "tokenAddress")]
    pub token_address: String,
    #[serde(serialize_with = "sentio_sdk::entity::serde_with::bigdecimal")]
    pub value: BigDecimal,
    #[serde(rename = "valueRaw")]
    #[serde(serialize_with = "sentio_sdk::entity::serde_with::bigint")]
    pub value_raw: BigInt,
    pub timestamp: Timestamp,
    #[serde(rename = "blockNumber")]
    pub block_number: i32,
    #[serde(rename = "txHash")]
    pub tx_hash: String,
    #[serde(rename = "logIndex")]
    pub log_index: i32,
}



impl Entity for Transfer {
    type Id = ID;
    const NAME: &'static str = "Transfer";

    fn id(&self) -> &Self::Id {
        &self.id
    }
}



impl Transfer {
    /// Get token relation
    pub async fn token(&self) -> EntityResult<Option<Token>> {
        let id = <Token as Entity>::Id::from_string(&self.token_id.to_string())?;
        Ok(Token::get(&id).await?)
    }
}

