use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ApiKeys {
    Single(String),
    Multiple(HashMap<String, String>),
}

#[derive(Debug, Serialize, Deserialize)]
struct HostConfig {
    api_keys: ApiKeys,
}

#[derive(Debug, Serialize, Deserialize)]
struct SentioConfig {
    #[serde(flatten)]
    hosts: HashMap<String, HostConfig>,
}

pub struct CredentialStore {
    config_dir: PathBuf,
    config_file: PathBuf,
}

impl CredentialStore {
    pub fn new() -> Self {
        let home_dir = dirs::home_dir().expect("Unable to get home directory");
        let config_dir = home_dir.join(".sentio");
        let config_file = config_dir.join("config.json");

        Self {
            config_dir,
            config_file,
        }
    }

    /// Store API key for a given host
    pub fn store_credentials(&self, host: &str, api_key: &str) -> Result<()> {
        // Ensure the config directory exists
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)
                .context("Failed to create .sentio config directory")?;
        }

        // Load existing config or create new one
        let mut config = if self.config_file.exists() {
            let content = fs::read_to_string(&self.config_file)
                .context("Failed to read config file")?;
            serde_json::from_str::<SentioConfig>(&content)
                .unwrap_or_else(|_| SentioConfig { hosts: HashMap::new() })
        } else {
            SentioConfig { hosts: HashMap::new() }
        };

        // Update the host configuration - use Single format for new entries
        config.hosts.insert(host.to_string(), HostConfig {
            api_keys: ApiKeys::Single(api_key.to_string()),
        });

        // Write the updated config
        let config_json = serde_json::to_string_pretty(&config)
            .context("Failed to serialize config")?;
        
        fs::write(&self.config_file, config_json)
            .context("Failed to write config file")?;

        Ok(())
    }

    /// Retrieve API key for a given host
    pub fn get_credentials(&self, host: &str) -> Result<Option<String>> {
        if !self.config_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.config_file)
            .context("Failed to read config file")?;
        
        let config: SentioConfig = serde_json::from_str(&content)
            .context("Failed to parse config file")?;

        if let Some(host_config) = config.hosts.get(host) {
            match &host_config.api_keys {
                ApiKeys::Single(key) => Ok(Some(key.clone())),
                ApiKeys::Multiple(keys) => {
                    // For multiple keys, return the first one (this is a legacy format)
                    Ok(keys.values().next().cloned())
                }
            }
        } else {
            Ok(None)
        }
    }


    /// Remove credentials for a given host
    pub fn remove_credentials(&self, host: &str) -> Result<bool> {
        if !self.config_file.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&self.config_file)
            .context("Failed to read config file")?;
        
        let mut config: SentioConfig = serde_json::from_str(&content)
            .context("Failed to parse config file")?;

        let removed = config.hosts.remove(host).is_some();

        if removed {
            let config_json = serde_json::to_string_pretty(&config)
                .context("Failed to serialize config")?;
            
            fs::write(&self.config_file, config_json)
                .context("Failed to write config file")?;
        }

        Ok(removed)
    }
}