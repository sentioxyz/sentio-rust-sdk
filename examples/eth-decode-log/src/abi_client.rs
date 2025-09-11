use alloy::json_abi::Event as JsonEvent;
use anyhow::Result;
use moka::future::Cache;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error};
use std::time::Instant;
use crate::bench;

#[derive(Debug, Deserialize)]
struct SentioApiResponse {
    #[serde(rename = "abiItem")]
    abi_item: Option<String>,
}

pub struct AbiClient {
    client: Client,
    base_url: String,
    chain_id: String,
    cache: Cache<String, Option<Arc<JsonEvent>>>,
}

impl AbiClient {
    pub fn new(sentio_host: String, chain_id: String) -> Self {
        Self {
            client: Client::new(),
            base_url: format!("{}/api/v1/solidity/signature", sentio_host),
            chain_id,
            cache: Cache::new(1000_000),
        }
    }

    pub async fn get_abi_from_signature(
        &self,
        signature: &str,
        address: &str,
        topics: Option<&[String]>,
        data: Option<&str>,
    ) -> Result<Option<Arc<JsonEvent>>> {
        // Build cache key: include topics if provided
        let cache_key = if let Some(t) = topics {
            let mut key = String::from(signature);
            key.push('|');
            key.push_str(&t.join(","));
            key
        } else {
            signature.to_string()
        };

        // Mark a total call and update cache size snapshot
        bench::mark_call();
        bench::update_cache_size(self.cache.entry_count());

        // Measure cache lookup time
        let cache_start = Instant::now();
        if let Some(cached_opt) = self.cache.get(&cache_key).await {
            bench::record_cache_hit(cache_start.elapsed());
            debug!("Cache hit for key: {} (present={})", cache_key, cached_opt.is_some());
            return Ok(cached_opt.clone());
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

        let api_start = Instant::now();
        let response = self
            .client
            .get(&self.base_url)
            .query(&all_params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                error!(
                    "Failed to fetch ABI for signature {}: 404 Not Found (cached negative)",
                    signature
                );
                // Only cache negative result on 404
                self.cache.insert(cache_key, None).await;
            } else {
                error!(
                    "Failed to fetch ABI for signature {}: {} (not cached)",
                    signature,
                    status
                );
            }
            return Ok(None);
        }

        let api_response: SentioApiResponse = response.json().await?;
        bench::record_api_call(api_start.elapsed());

        if let Some(abi_item) = api_response.abi_item {
            // Parse abi_item string into JsonEvent
            let json_event: JsonEvent = serde_json::from_str(&abi_item)?;
            let json_event = Arc::new(json_event);

            self.cache.insert(cache_key, Some(json_event.clone())).await;

            Ok(Some(json_event))
        } else {
            self.cache.insert(cache_key,None).await;
            Ok(None)
        }
    }
}
