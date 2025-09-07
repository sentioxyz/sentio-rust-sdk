use std::collections::HashMap;
use std::env;

/// Loads test providers from environment variables
///
/// This function mirrors the TypeScript `loadTestProvidersFromEnv` functionality,
/// allowing test environments to configure blockchain endpoints through environment variables.
/// 
/// Environment variables should be named: `TEST_ENDPOINT_{CHAIN_ID}`
/// For example: `TEST_ENDPOINT_1` for Ethereum mainnet.
///
/// # Arguments
///
/// * `required_chain_ids` - Chain IDs that must have endpoints configured
///
/// # Returns
///
/// `true` if all required chain IDs have endpoints configured, `false` otherwise
///
/// # Example
///
/// ```rust
/// use sentio_sdk::testing::load_test_providers_from_env;
///
/// // Set environment variable in your test
/// std::env::set_var("TEST_ENDPOINT_1", "https://eth.llamarpc.com");
///
/// assert!(load_test_providers_from_env(&["1"]));
/// ```
pub fn load_test_providers_from_env(required_chain_ids: &[&str]) -> bool {
    let mut found = Vec::new();
    let mut endpoints = HashMap::new();
    
    // Scan all environment variables for TEST_ENDPOINT_ prefix
    for (key, value) in env::vars() {
        if let Some(chain_id) = key.strip_prefix("TEST_ENDPOINT_") {
            found.push(chain_id.to_string());
            endpoints.insert(chain_id.to_string(), value);
        }
    }
    
    // Check if all required chain IDs are found
    for &required_id in required_chain_ids {
        if !found.contains(&required_id.to_string()) {
            return false;
        }
    }
    
    // TODO: Set endpoints in the global endpoint manager
    // This would integrate with the runtime's endpoint configuration
    
    true
}

/// Test environment configuration
#[derive(Debug, Clone)]
pub struct TestEnvironment {
    pub endpoints: HashMap<String, String>,
    pub timeout_ms: u64,
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self {
            endpoints: HashMap::new(),
            timeout_ms: 30000, // 30 seconds default timeout
        }
    }
}

impl TestEnvironment {
    /// Create a new test environment
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add an endpoint for a specific chain
    pub fn with_endpoint(mut self, chain_id: impl Into<String>, endpoint: impl Into<String>) -> Self {
        self.endpoints.insert(chain_id.into(), endpoint.into());
        self
    }
    
    /// Set the timeout for test operations
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
    
    /// Load endpoints from environment variables
    pub fn from_env(required_chain_ids: &[&str]) -> Option<Self> {
        if load_test_providers_from_env(required_chain_ids) {
            let mut env = Self::new();
            
            for (key, value) in env::vars() {
                if let Some(chain_id) = key.strip_prefix("TEST_ENDPOINT_") {
                    env.endpoints.insert(chain_id.to_string(), value);
                }
            }
            
            Some(env)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_providers_from_env() {
        // Set up test environment variable
        unsafe {
            env::set_var("TEST_ENDPOINT_1", "https://example.com");
            env::set_var("TEST_ENDPOINT_137", "https://polygon.example.com");
        }
        
        // Test successful case
        assert!(load_test_providers_from_env(&["1"]));
        assert!(load_test_providers_from_env(&["1", "137"]));
        
        // Test failure case
        assert!(!load_test_providers_from_env(&["1", "137", "999"]));
        
        // Clean up
        unsafe {
            env::remove_var("TEST_ENDPOINT_1");
            env::remove_var("TEST_ENDPOINT_137");
        }
    }
    
    #[test]
    fn test_environment_builder() {
        let env = TestEnvironment::new()
            .with_endpoint("1", "https://eth.example.com")
            .with_endpoint("137", "https://polygon.example.com")
            .with_timeout(60000);
            
        assert_eq!(env.endpoints.get("1"), Some(&"https://eth.example.com".to_string()));
        assert_eq!(env.endpoints.get("137"), Some(&"https://polygon.example.com".to_string()));
        assert_eq!(env.timeout_ms, 60000);
    }
}