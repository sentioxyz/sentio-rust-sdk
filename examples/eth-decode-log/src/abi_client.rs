use anyhow::Result;
use moka::sync::Cache;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error};

#[derive(Debug, Deserialize)]
struct SentioApiResponse {
    #[serde(rename = "abiItem")]
    abi_item: Option<String>,
}

pub struct AbiClient {
    client: Client,
    base_url: String,
    chain_id: String,
    cache: Arc<Cache<String, String>>,
}

impl AbiClient {
    pub fn new(sentio_host: String, chain_id: String) -> Self {
        Self {
            client: Client::new(),
            base_url: format!("{}/api/v1/solidity/signature", sentio_host),
            chain_id,
            cache: Arc::new(Cache::new(100_000)),
        }
    }

    pub async fn get_abi_from_signature(
        &self,
        signature: &str,
        address: &str,
        topics: Option<&[String]>,
        data: Option<&str>,
    ) -> Result<Option<String>> {
        // Check cache first (only if no topics/data for broader caching)
        if topics.is_none() && data.is_none()
            && let Some(cached) = self.cache.get(signature) {
                debug!("Cache hit for signature: {}", signature);
                return Ok(Some(cached));
            }

        debug!("Fetching ABI for signature: {}", signature);

        let params = vec![
            ("chainSpec.chainId", self.chain_id.as_str()),
            ("type", "1"),
            ("hex_signature", signature),
            ("address", address),
        ];

        // Add topics if provided
        let topic_params: Vec<(String, String)> = if let Some(topics) = topics {
            topics
                .iter()
                .map(|topic| ("topics".to_string(), topic.clone()))
                .collect()
        } else {
            Vec::new()
        };

        let mut all_params: Vec<(&str, &str)> = params;
        let topic_str_refs: Vec<(&str, &str)> = topic_params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        all_params.extend(topic_str_refs);

        if let Some(data) = data {
            all_params.push(("data", data));
        }

        let response = self
            .client
            .get(&self.base_url)
            .query(&all_params)
            .send()
            .await?;

        if !response.status().is_success() {
            error!(
                "Failed to fetch ABI for signature {}: {}",
                signature,
                response.status()
            );
            return Ok(None);
        }

        let api_response: SentioApiResponse = response.json().await?;

        if let Some(abi_item) = api_response.abi_item {
            // Cache the result (only if no topics/data)
            if topics.is_none() && data.is_none() {
                self.cache.insert(signature.to_string(), abi_item.clone());
            }
            Ok(Some(abi_item))
        } else {
            Ok(None)
        }
    }
}
