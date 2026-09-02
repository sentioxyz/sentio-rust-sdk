//! RPC endpoints per chain.
//!
//! The Sentio platform starts every processor binary with `--chains-config=<path>`
//! pointing at a `chains-config.json` (the same file the TypeScript processor-runner
//! reads). This is how a processor learns which RPC endpoint to use for a chain.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Default, Deserialize)]
pub struct ChainsConfig(HashMap<String, ChainConfig>);

#[derive(Debug, Default, Deserialize)]
pub struct ChainConfig {
    #[serde(rename = "ChainID", default)]
    pub chain_id: String,
    #[serde(rename = "Https", default)]
    pub https: Vec<String>,
    #[serde(rename = "ChainServer", default)]
    pub chain_server: String,
    /// Present in the driver's own chains config; accepted as a last resort in case
    /// that file (rather than the SDK-facing one) is what reaches the processor.
    #[serde(rename = "Endpoint", default)]
    pub endpoint: String,
    #[serde(rename = "Rpc", default)]
    pub rpc: Option<RpcConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RpcConfig {
    #[serde(rename = "Url", default)]
    pub url: String,
    #[serde(rename = "Headers", default)]
    pub headers: HashMap<String, String>,
}

impl ChainsConfig {
    /// Load the config named by `--chains-config=<path>` (or `--chains-config <path>`)
    /// on the command line; an absent or unreadable file yields an empty config.
    pub fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let path = args.iter().enumerate().find_map(|(i, arg)| {
            arg.strip_prefix("--chains-config=")
                .map(str::to_string)
                .or_else(|| (arg == "--chains-config").then(|| args.get(i + 1).cloned()).flatten())
        });
        match path {
            Some(path) => match Self::from_file(&path) {
                Ok(config) => {
                    eprintln!("chains config {}: {} chain(s) {:?}", path, config.0.len(), config.chain_ids());
                    config
                }
                Err(e) => {
                    eprintln!("failed to read chains config {}: {}", path, e);
                    Self::default()
                }
            },
            None => {
                eprintln!("no --chains-config argument given; RPC endpoints only via TEST_ENDPOINT_<chain>");
                Self::default()
            }
        }
    }

    pub fn chain_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.0.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// RPC URL for a chain, in the order the v4 TypeScript runtime uses: an explicit
    /// `TEST_ENDPOINT_<chain>` env override, then `Rpc.Url`, then the first `Https`
    /// entry, then the legacy `ChainServer`, then the driver-side `Endpoint`.
    pub fn rpc_url(&self, chain_id: &str) -> Option<String> {
        if let Ok(url) = std::env::var(format!("TEST_ENDPOINT_{}", chain_id)) {
            return Some(url);
        }
        let cfg = self.0.get(chain_id)?;
        cfg.rpc
            .as_ref()
            .map(|rpc| rpc.url.clone())
            .filter(|url| !url.is_empty())
            .or_else(|| cfg.https.first().cloned())
            .or_else(|| (!cfg.chain_server.is_empty()).then(|| cfg.chain_server.clone()))
            .or_else(|| (!cfg.endpoint.is_empty()).then(|| cfg.endpoint.clone()))
    }
}
