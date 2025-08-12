use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure for Sentio projects
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SentioConfig {
    pub name: String,
    pub version: String,
    pub target_network: String,
    pub contracts: Vec<ContractConfig>,
    pub build: BuildConfig,
}

/// Configuration for individual contracts
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContractConfig {
    pub address: String,
    pub name: String,
    pub network: String,
    pub abi_path: Option<String>,
    pub added_at: String,
}

/// Build configuration settings
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildConfig {
    pub target: String,
    pub optimization_level: String,
    pub features: Vec<String>,
}


/// Configuration manager that handles loading project configurations
pub struct ConfigManager {
    project_config: Option<SentioConfig>,
    project_path: PathBuf,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            target: "x86_64-unknown-linux-gnu".to_string(),
            optimization_level: "release".to_string(),
            features: vec![],
        }
    }
}

impl Default for SentioConfig {
    fn default() -> Self {
        Self {
            name: "sentio-processor".to_string(),
            version: "0.1.0".to_string(),
            target_network: "ethereum".to_string(),
            contracts: vec![],
            build: BuildConfig::default(),
        }
    }
}


impl SentioConfig {
    /// Load configuration from a specific path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_path = path.as_ref().join("sentio.yaml");

        if !config_path.exists() {
            return Err(anyhow!(
                "Configuration file not found at: {}",
                config_path.display()
            ));
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: SentioConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

        Ok(config)
    }

    /// Save configuration to a specific path
    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let config_path = path.as_ref().join("sentio.yaml");

        let content = serde_yaml::to_string(self).context("Failed to serialize configuration")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Add a contract to the configuration
    pub fn add_contract(&mut self, contract: ContractConfig) -> Result<()> {
        // Check if contract already exists
        if self
            .contracts
            .iter()
            .any(|c| c.address == contract.address && c.network == contract.network)
        {
            return Err(anyhow!(
                "Contract {} already exists on network {}",
                contract.address,
                contract.network
            ));
        }

        self.contracts.push(contract);
        Ok(())
    }

    /// Remove a contract from the configuration
    pub fn remove_contract(&mut self, address: &str, network: Option<&str>) -> Result<bool> {
        let initial_len = self.contracts.len();

        self.contracts.retain(|c| {
            if let Some(net) = network {
                !(c.address == address && c.network == net)
            } else {
                c.address != address
            }
        });

        Ok(self.contracts.len() < initial_len)
    }

    /// Get contracts for a specific network
    pub fn get_contracts_for_network(&self, network: &str) -> Vec<&ContractConfig> {
        self.contracts
            .iter()
            .filter(|c| c.network == network)
            .collect()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("Project name cannot be empty"));
        }

        if self.version.is_empty() {
            return Err(anyhow!("Project version cannot be empty"));
        }

        if self.target_network.is_empty() {
            return Err(anyhow!("Target network cannot be empty"));
        }

        // Validate contract addresses
        for contract in &self.contracts {
            if contract.address.is_empty() {
                return Err(anyhow!("Contract address cannot be empty"));
            }

            if contract.name.is_empty() {
                return Err(anyhow!("Contract name cannot be empty"));
            }

            if contract.network.is_empty() {
                return Err(anyhow!("Contract network cannot be empty"));
            }
        }

        Ok(())
    }
}


impl ConfigManager {
    /// Create a new configuration manager for a project
    pub fn new<P: AsRef<Path>>(project_path: P) -> Self {
        Self {
            project_config: None,
            project_path: project_path.as_ref().to_path_buf(),
        }
    }

    /// Load project configuration
    pub fn load(&mut self) -> Result<()> {
        // Load project config if it exists
        if self.project_path.join("sentio.yaml").exists() {
            self.project_config = Some(SentioConfig::load_from_path(&self.project_path)?);
        }

        Ok(())
    }

    /// Get the effective configuration with environment variable overrides
    pub fn get_effective_config(&self) -> Result<SentioConfig> {
        let mut config = self
            .project_config
            .clone()
            .unwrap_or_else(|| SentioConfig::default());

        // Apply environment variable overrides
        if let Ok(target_network) = env::var("SENTIO_TARGET_NETWORK") {
            config.target_network = target_network;
        }

        if let Ok(build_target) = env::var("SENTIO_BUILD_TARGET") {
            config.build.target = build_target;
        }

        if let Ok(optimization) = env::var("SENTIO_OPTIMIZATION_LEVEL") {
            config.build.optimization_level = optimization;
        }

        config.validate()?;
        Ok(config)
    }

    /// Get the project configuration
    pub fn get_project_config(&self) -> Option<&SentioConfig> {
        self.project_config.as_ref()
    }


    /// Save the current project configuration
    pub fn save_project_config(&self, config: &SentioConfig) -> Result<()> {
        config.save_to_path(&self.project_path)
    }

    /// Update project configuration
    pub fn update_project_config<F>(&mut self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut SentioConfig) -> Result<()>,
    {
        let mut config = self
            .project_config
            .clone()
            .unwrap_or_else(|| SentioConfig::default());

        updater(&mut config)?;
        config.validate()?;

        self.save_project_config(&config)?;
        self.project_config = Some(config);

        Ok(())
    }

    /// Check if we're in a Sentio project directory
    pub fn is_sentio_project(&self) -> bool {
        self.project_path.join("sentio.yaml").exists()
            || self.project_path.join("Cargo.toml").exists()
    }

    /// Find the project root by looking for sentio.yaml or Cargo.toml
    pub fn find_project_root<P: AsRef<Path>>(start_path: P) -> Option<PathBuf> {
        let mut current = start_path.as_ref().to_path_buf();

        loop {
            if current.join("sentio.yaml").exists() || current.join("Cargo.toml").exists() {
                return Some(current);
            }

            if !current.pop() {
                break;
            }
        }

        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> SentioConfig {
        SentioConfig {
            name: "test-processor".to_string(),
            version: "1.0.0".to_string(),
            target_network: "ethereum".to_string(),
            contracts: vec![ContractConfig {
                address: "0x1234567890123456789012345678901234567890".to_string(),
                name: "TestContract".to_string(),
                network: "ethereum".to_string(),
                abi_path: Some("abis/TestContract.json".to_string()),
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
            build: BuildConfig {
                target: "x86_64-unknown-linux-gnu".to_string(),
                optimization_level: "release".to_string(),
                features: vec!["default".to_string()],
            },
        }
    }

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();
        let yaml_str = serde_yaml::to_string(&config).unwrap();
        let deserialized: SentioConfig = serde_yaml::from_str(&yaml_str).unwrap();

        assert_eq!(config.name, deserialized.name);
        assert_eq!(config.version, deserialized.version);
        assert_eq!(config.contracts.len(), deserialized.contracts.len());
    }

    #[test]
    fn test_config_validation() {
        let mut config = create_test_config();
        assert!(config.validate().is_ok());

        // Test empty name
        config.name = "".to_string();
        assert!(config.validate().is_err());

        // Reset and test empty version
        config = create_test_config();
        config.version = "".to_string();
        assert!(config.validate().is_err());

        // Reset and test empty target network
        config = create_test_config();
        config.target_network = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_add_contract() {
        let mut config = create_test_config();
        let initial_count = config.contracts.len();

        let new_contract = ContractConfig {
            address: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
            name: "NewContract".to_string(),
            network: "ethereum".to_string(),
            abi_path: None,
            added_at: "2024-01-02T00:00:00Z".to_string(),
        };

        assert!(config.add_contract(new_contract).is_ok());
        assert_eq!(config.contracts.len(), initial_count + 1);

        // Test adding duplicate contract
        let duplicate_contract = ContractConfig {
            address: "0x1234567890123456789012345678901234567890".to_string(),
            name: "DuplicateContract".to_string(),
            network: "ethereum".to_string(),
            abi_path: None,
            added_at: "2024-01-03T00:00:00Z".to_string(),
        };

        assert!(config.add_contract(duplicate_contract).is_err());
    }

    #[test]
    fn test_remove_contract() {
        let mut config = create_test_config();
        let initial_count = config.contracts.len();

        // Remove existing contract
        assert!(config
            .remove_contract(
                "0x1234567890123456789012345678901234567890",
                Some("ethereum")
            )
            .unwrap());
        assert_eq!(config.contracts.len(), initial_count - 1);

        // Try to remove non-existent contract
        assert!(!config
            .remove_contract("0xnonexistent", Some("ethereum"))
            .unwrap());
    }

    #[test]
    fn test_get_contracts_for_network() {
        let config = create_test_config();
        let ethereum_contracts = config.get_contracts_for_network("ethereum");
        assert_eq!(ethereum_contracts.len(), 1);

        let polygon_contracts = config.get_contracts_for_network("polygon");
        assert_eq!(polygon_contracts.len(), 0);
    }

    #[test]
    fn test_config_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();

        // Test saving config
        assert!(config.save_to_path(temp_dir.path()).is_ok());

        // Test loading config
        let loaded_config = SentioConfig::load_from_path(temp_dir.path()).unwrap();
        assert_eq!(config.name, loaded_config.name);
        assert_eq!(config.version, loaded_config.version);
        assert_eq!(config.contracts.len(), loaded_config.contracts.len());
    }

    #[test]
    fn test_config_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let result = SentioConfig::load_from_path(temp_dir.path());
        assert!(result.is_err());
    }


    #[test]
    fn test_config_manager() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();

        // Save a config file
        config.save_to_path(temp_dir.path()).unwrap();

        // Create config manager and load
        let mut manager = ConfigManager::new(temp_dir.path());
        assert!(manager.load().is_ok());

        // Test effective config
        let effective_config = manager.get_effective_config().unwrap();
        assert_eq!(effective_config.name, config.name);

        // Test project config access
        assert!(manager.get_project_config().is_some());
    }

    #[test]
    fn test_find_project_root() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        let nested_dir = project_dir.join("src").join("nested");

        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(project_dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let found_root = ConfigManager::find_project_root(&nested_dir);
        assert!(found_root.is_some());
        assert_eq!(found_root.unwrap(), project_dir);
    }

    #[test]
    fn test_is_sentio_project() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path());

        // Initially not a Sentio project
        assert!(!manager.is_sentio_project());

        // Create Cargo.toml
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        assert!(manager.is_sentio_project());
    }

    #[test]
    fn test_environment_variable_overrides() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        config.save_to_path(temp_dir.path()).unwrap();

        let mut manager = ConfigManager::new(temp_dir.path());
        manager.load().unwrap();

        // Set environment variables
        env::set_var("SENTIO_TARGET_NETWORK", "polygon");
        env::set_var("SENTIO_BUILD_TARGET", "aarch64-unknown-linux-gnu");

        let effective_config = manager.get_effective_config().unwrap();
        assert_eq!(effective_config.target_network, "polygon");
        assert_eq!(effective_config.build.target, "aarch64-unknown-linux-gnu");

        // Clean up environment variables
        env::remove_var("SENTIO_TARGET_NETWORK");
        env::remove_var("SENTIO_BUILD_TARGET");
    }

    #[test]
    fn test_update_project_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config();
        config.save_to_path(temp_dir.path()).unwrap();

        let mut manager = ConfigManager::new(temp_dir.path());
        manager.load().unwrap();

        // Update config
        manager
            .update_project_config(|config| {
                config.name = "updated-processor".to_string();
                Ok(())
            })
            .unwrap();

        let updated_config = manager.get_project_config().unwrap();
        assert_eq!(updated_config.name, "updated-processor");
    }
}
