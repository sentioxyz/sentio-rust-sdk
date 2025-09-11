use alloy::json_abi::Event as JsonEvent;
use anyhow::Result;
use moka::future::Cache;
use reqwest::Client;
use serde::Deserialize;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use tracing::{debug, error, info};

#[derive(Debug, Deserialize)]
struct SentioApiResponse {
    #[serde(rename = "abiItem")]
    abi_item: Option<String>,
}

pub struct AbiClient {
    client: Client,
    base_url: String,
    chain_id: String,
    cache: Cache<String, Arc<JsonEvent>>,
    request_count: AtomicU64,
    cache_hits: AtomicU64,
}

impl AbiClient {
    pub fn new(sentio_host: String, chain_id: String) -> Self {
        Self {
            client: Client::new(),
            base_url: format!("{}/api/v1/solidity/signature", sentio_host),
            chain_id,
            cache: Cache::new(1000_000),
            request_count: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
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

        // Increment request counter and print stats every 1000 calls
        let cnt = self.request_count.fetch_add(1, Ordering::Relaxed) + 1;
        if cnt % 1000 == 0 {
            let hits = self.cache_hits.load(Ordering::Relaxed);
            let hit_rate = if cnt > 0 { (hits as f64) / (cnt as f64) * 100.0 } else { 0.0 };
            let entries = self.cache.entry_count();
            info!(
                "AbiClient cache stats: requests={}, hits={}, hit_rate={:.2}%, entries={}",
                cnt, hits, hit_rate, entries
            );
        }

        if let Some(cached) = self.cache.get(&cache_key).await {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            debug!("Cache hit for key: {}", cache_key);
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
            // Parse abi_item string into JsonEvent
            let json_event: JsonEvent = serde_json::from_str(&abi_item)?;
            let json_event = Arc::new(json_event);

            self.cache.insert(cache_key, json_event.clone()).await;

            Ok(Some(json_event))
        } else {
            Ok(None)
        }
    }
}
