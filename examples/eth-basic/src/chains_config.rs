//! RPC endpoints per chain.
//!
//! The Sentio platform starts every processor binary with `--chains-config=<path>`.
//! For node processors that path holds the SDK-facing `chains-config.json`
//! (`Rpc.Url` / `Https`); for binary processors the prepare step currently does
//! not write it, so we fall back to the driver's own chains config mounted at
//! [`DRIVER_CHAINS_CONFIG`], which carries `ChainServer` / `Https` / `Endpoint`.

use serde::Deserialize;
use std::collections::HashMap;

/// Driver chains config, mounted into the processor container by the platform.
pub const DRIVER_CHAINS_CONFIG: &str = "/etc/sentio/chains-config.json";

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
    /// Driver config: `ChainServer` is the rpc-node proxy, which is what the
    /// platform itself uses for this chain — prefer it over the raw `Https` list.
    #[serde(rename = "ChainServerUseRPCNode", default)]
    pub chain_server_use_rpc_node: bool,
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
        let mut candidates: Vec<String> = path.into_iter().collect();
        if candidates.is_empty() {
            eprintln!("no --chains-config argument given");
        }
        candidates.push(DRIVER_CHAINS_CONFIG.to_string());

        for candidate in &candidates {
            match Self::from_file(candidate) {
                Ok(config) => {
                    eprintln!("chains config {}: {} chain(s)", candidate, config.0.len());
                    return config;
                }
                Err(e) => eprintln!("chains config {} unavailable: {}", candidate, e),
            }
        }
        eprintln!("no chains config found; RPC endpoints only via TEST_ENDPOINT_<chain>");
        Self::default()
    }

    pub fn chain_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.0.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// RPC URL for a chain: an explicit `TEST_ENDPOINT_<chain>` env override, then
    /// `Rpc.Url` (SDK-facing file), then `ChainServer` when it is the rpc-node proxy
    /// (driver file), then the first `Https` entry, then any remaining
    /// `ChainServer` / `Endpoint`.
    pub fn rpc_url(&self, chain_id: &str) -> Option<String> {
        if let Ok(url) = std::env::var(format!("TEST_ENDPOINT_{}", chain_id)) {
            return Some(url);
        }
        let cfg = self.0.get(chain_id)?;
        let non_empty = |s: &str| (!s.is_empty()).then(|| s.to_string());
        cfg.rpc
            .as_ref()
            .and_then(|rpc| non_empty(&rpc.url))
            .or_else(|| cfg.chain_server_use_rpc_node.then(|| non_empty(&cfg.chain_server)).flatten())
            .or_else(|| cfg.https.first().and_then(|s| non_empty(s)))
            .or_else(|| non_empty(&cfg.chain_server))
            .or_else(|| non_empty(&cfg.endpoint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> ChainsConfig {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn sdk_shape_prefers_rpc_url_then_https() {
        let cfg = parse(r#"{"1":{"ChainID":"1","Https":["https://a"],"Rpc":{"Url":"https://rpc"}},
                            "10":{"ChainID":"10","Https":["https://b"]}}"#);
        assert_eq!(cfg.rpc_url("1").as_deref(), Some("https://rpc"));
        assert_eq!(cfg.rpc_url("10").as_deref(), Some("https://b"));
        assert_eq!(cfg.rpc_url("999"), None);
    }

    #[test]
    fn driver_shape_prefers_chain_server_proxy_then_https_then_endpoint() {
        let cfg = parse(r#"{
            "1":{"ChainID":"1","ChainName":"eth-mainnet","Endpoint":"http://ep","Https":["http://h"],"ChainServer":"http://proxy","ChainServerUseRPCNode":true},
            "2":{"ChainID":"2","Endpoint":"http://ep2","Https":["http://h2"],"ChainServer":"http://cs2","ChainServerUseRPCNode":false},
            "3":{"ChainID":"3","Endpoint":"http://ep3","Https":[]}}"#);
        assert_eq!(cfg.rpc_url("1").as_deref(), Some("http://proxy"));
        assert_eq!(cfg.rpc_url("2").as_deref(), Some("http://h2"));
        assert_eq!(cfg.rpc_url("3").as_deref(), Some("http://ep3"));
    }
}
