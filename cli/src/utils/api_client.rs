use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Request/Response models for Sentio API
#[derive(Serialize, Debug)]
pub struct UploadRequest {
    pub processor_name: String,
    pub version: String,
    pub target_network: String,
    #[serde(skip_serializing)]
    pub binary_data: Vec<u8>,
}

#[derive(Deserialize, Debug)]
pub struct UploadResponse {
    pub deployment_id: String,
    pub status: String,
    pub url: String,
    pub message: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub code: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct AuthResponse {
    pub token: String,
    pub expires_at: String,
    pub user_id: String,
}

#[derive(Serialize, Debug)]
pub struct AuthRequest {
    pub api_key: String,
}

/// Configuration for the API client
#[derive(Debug, Clone)]
pub struct ApiClientConfig {
    pub base_url: String,
    pub timeout: Duration,
    pub max_retries: u32,
    pub api_key: Option<String>,
}

impl Default for ApiClientConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.sentio.xyz".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            api_key: None,
        }
    }
}

/// HTTP client for Sentio platform API
pub struct SentioApiClient {
    client: reqwest::Client,
    config: ApiClientConfig,
}

impl SentioApiClient {
    /// Create a new API client with default configuration
    pub fn new() -> Self {
        Self::with_config(ApiClientConfig::default())
    }

    /// Create a new API client with custom configuration
    pub fn with_config(config: ApiClientConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent("sentio-cli/1.0.0")
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Create a new API client with base URL
    pub fn with_base_url(base_url: String) -> Self {
        let mut config = ApiClientConfig::default();
        config.base_url = base_url;
        Self::with_config(config)
    }

    /// Set API key for authentication
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.config.api_key = Some(api_key);
        self
    }

    /// Upload a binary to the Sentio platform
    pub async fn upload_binary(&self, request: UploadRequest) -> Result<UploadResponse> {
        let url = format!("{}/v1/processors/upload", self.config.base_url);

        // Create multipart form
        let form = reqwest::multipart::Form::new()
            .text("processor_name", request.processor_name.clone())
            .text("version", request.version.clone())
            .text("target_network", request.target_network.clone())
            .part(
                "binary",
                reqwest::multipart::Part::bytes(request.binary_data)
                    .file_name("processor.wasm")
                    .mime_str("application/wasm")
                    .context("Failed to set MIME type for binary")?,
            );

        let mut req_builder = self.client.post(&url).multipart(form);

        // Add authentication if available
        if let Some(api_key) = &self.config.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = self.execute_with_retry(req_builder).await?;

        if response.status().is_success() {
            let upload_response: UploadResponse = response
                .json()
                .await
                .context("Failed to parse upload response")?;
            Ok(upload_response)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse as API error
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                Err(anyhow!(
                    "API Error: {} - {}",
                    api_error.error,
                    api_error.message
                ))
            } else {
                Err(anyhow!(
                    "Upload failed with status {}: {}",
                    status,
                    error_text
                ))
            }
        }
    }

    /// Authenticate with the Sentio platform
    pub async fn authenticate(&self, api_key: &str) -> Result<AuthResponse> {
        let url = format!("{}/v1/auth/login", self.config.base_url);

        let auth_request = AuthRequest {
            api_key: api_key.to_string(),
        };

        let req_builder = self
            .client
            .post(&url)
            .json(&auth_request)
            .header("Content-Type", "application/json");

        let response = self.execute_with_retry(req_builder).await?;

        if response.status().is_success() {
            let auth_response: AuthResponse = response
                .json()
                .await
                .context("Failed to parse authentication response")?;
            Ok(auth_response)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                Err(anyhow!(
                    "Authentication failed: {} - {}",
                    api_error.error,
                    api_error.message
                ))
            } else {
                Err(anyhow!(
                    "Authentication failed with status {}: {}",
                    status,
                    error_text
                ))
            }
        }
    }

    /// Validate an API token
    pub async fn validate_token(&self, token: &str) -> Result<bool> {
        let url = format!("{}/v1/auth/validate", self.config.base_url);

        let req_builder = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token));

        let response = self.execute_with_retry(req_builder).await?;
        Ok(response.status().is_success())
    }

    /// Get user information
    pub async fn get_user_info(&self) -> Result<HashMap<String, serde_json::Value>> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("API key not set"))?;

        let url = format!("{}/v1/user/info", self.config.base_url);

        let req_builder = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key));

        let response = self.execute_with_retry(req_builder).await?;

        if response.status().is_success() {
            let user_info: HashMap<String, serde_json::Value> = response
                .json()
                .await
                .context("Failed to parse user info response")?;
            Ok(user_info)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow!("Failed to get user info: {}", error_text))
        }
    }

    /// List user's processors
    pub async fn list_processors(&self) -> Result<Vec<HashMap<String, serde_json::Value>>> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("API key not set"))?;

        let url = format!("{}/v1/processors", self.config.base_url);

        let req_builder = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key));

        let response = self.execute_with_retry(req_builder).await?;

        if response.status().is_success() {
            let processors: Vec<HashMap<String, serde_json::Value>> = response
                .json()
                .await
                .context("Failed to parse processors list response")?;
            Ok(processors)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow!("Failed to list processors: {}", error_text))
        }
    }

    /// Get processor deployment status
    pub async fn get_deployment_status(
        &self,
        deployment_id: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("API key not set"))?;

        let url = format!("{}/v1/deployments/{}", self.config.base_url, deployment_id);

        let req_builder = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key));

        let response = self.execute_with_retry(req_builder).await?;

        if response.status().is_success() {
            let status: HashMap<String, serde_json::Value> = response
                .json()
                .await
                .context("Failed to parse deployment status response")?;
            Ok(status)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow!("Failed to get deployment status: {}", error_text))
        }
    }

    /// Execute a request with retry logic
    async fn execute_with_retry(
        &self,
        req_builder: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            let request = req_builder
                .try_clone()
                .ok_or_else(|| anyhow!("Failed to clone request for retry"))?;

            match request.send().await {
                Ok(response) => {
                    // Check if we should retry based on status code
                    if response.status().is_server_error() && attempt < self.config.max_retries {
                        last_error = Some(anyhow!("Server error: {}", response.status()));

                        // Exponential backoff
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(anyhow!("Request failed: {}", e));

                    if attempt < self.config.max_retries {
                        // Exponential backoff
                        let delay = Duration::from_millis(1000 * (2_u64.pow(attempt)));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow!("Request failed after {} retries", self.config.max_retries)))
    }

    /// Check if the API is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.config.base_url);

        let response = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("Health check request failed")?;

        Ok(response.status().is_success())
    }

    /// Get API client configuration
    pub fn config(&self) -> &ApiClientConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_client() -> SentioApiClient {
        let config = ApiClientConfig {
            base_url: "https://api.test.sentio.xyz".to_string(),
            timeout: Duration::from_secs(10),
            max_retries: 1,
            api_key: Some("test-api-key".to_string()),
        };
        SentioApiClient::with_config(config)
    }

    fn create_test_upload_request() -> UploadRequest {
        UploadRequest {
            processor_name: "test-processor".to_string(),
            version: "1.0.0".to_string(),
            target_network: "ethereum".to_string(),
            binary_data: vec![0x00, 0x61, 0x73, 0x6d], // WASM magic bytes
        }
    }

    #[test]
    fn test_api_client_creation() {
        let client = SentioApiClient::new();
        assert_eq!(client.config().base_url, "https://api.sentio.xyz");
        assert_eq!(client.config().timeout, Duration::from_secs(30));
        assert_eq!(client.config().max_retries, 3);
        assert!(client.config().api_key.is_none());
    }

    #[test]
    fn test_api_client_with_base_url() {
        let base_url = "https://custom.api.sentio.xyz".to_string();
        let client = SentioApiClient::with_base_url(base_url.clone());
        assert_eq!(client.config().base_url, base_url);
    }

    #[test]
    fn test_api_client_with_api_key() {
        let api_key = "test-key-123".to_string();
        let client = SentioApiClient::new().with_api_key(api_key.clone());
        assert_eq!(client.config().api_key, Some(api_key));
    }

    #[test]
    fn test_api_client_config() {
        let config = ApiClientConfig {
            base_url: "https://test.api.sentio.xyz".to_string(),
            timeout: Duration::from_secs(60),
            max_retries: 5,
            api_key: Some("custom-key".to_string()),
        };

        let client = SentioApiClient::with_config(config.clone());
        assert_eq!(client.config().base_url, config.base_url);
        assert_eq!(client.config().timeout, config.timeout);
        assert_eq!(client.config().max_retries, config.max_retries);
        assert_eq!(client.config().api_key, config.api_key);
    }

    #[test]
    fn test_upload_request_serialization() {
        let request = create_test_upload_request();

        // Test that we can serialize the request (excluding binary_data)
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test-processor"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("ethereum"));
        // binary_data should be skipped in serialization
        assert!(!json.contains("binary_data"));
    }

    #[test]
    fn test_api_error_deserialization() {
        let error_json = r#"{
            "error": "ValidationError",
            "message": "Invalid processor format",
            "code": 400
        }"#;

        let api_error: ApiError = serde_json::from_str(error_json).unwrap();
        assert_eq!(api_error.error, "ValidationError");
        assert_eq!(api_error.message, "Invalid processor format");
        assert_eq!(api_error.code, Some(400));
    }

    #[test]
    fn test_upload_response_deserialization() {
        let response_json = r#"{
            "deployment_id": "deploy-123",
            "status": "pending",
            "url": "https://sentio.xyz/deployments/deploy-123",
            "message": "Upload successful"
        }"#;

        let upload_response: UploadResponse = serde_json::from_str(response_json).unwrap();
        assert_eq!(upload_response.deployment_id, "deploy-123");
        assert_eq!(upload_response.status, "pending");
        assert_eq!(
            upload_response.url,
            "https://sentio.xyz/deployments/deploy-123"
        );
        assert_eq!(
            upload_response.message,
            Some("Upload successful".to_string())
        );
    }

    #[test]
    fn test_auth_response_deserialization() {
        let auth_json = r#"{
            "token": "jwt-token-123",
            "expires_at": "2024-12-31T23:59:59Z",
            "user_id": "user-456"
        }"#;

        let auth_response: AuthResponse = serde_json::from_str(auth_json).unwrap();
        assert_eq!(auth_response.token, "jwt-token-123");
        assert_eq!(auth_response.expires_at, "2024-12-31T23:59:59Z");
        assert_eq!(auth_response.user_id, "user-456");
    }

    #[test]
    fn test_auth_request_serialization() {
        let auth_request = AuthRequest {
            api_key: "test-api-key".to_string(),
        };

        let json = serde_json::to_string(&auth_request).unwrap();
        assert!(json.contains("test-api-key"));
    }

    #[test]
    fn test_default_config() {
        let config = ApiClientConfig::default();
        assert_eq!(config.base_url, "https://api.sentio.xyz");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert!(config.api_key.is_none());
    }

    // Integration tests would require a mock server
    // These tests verify the structure and basic functionality

    #[tokio::test]
    async fn test_health_check_timeout() {
        // Test with a non-existent URL to verify timeout behavior
        let config = ApiClientConfig {
            base_url: "http://localhost:99999".to_string(), // Non-existent port
            timeout: Duration::from_millis(100),
            max_retries: 0,
            api_key: None,
        };

        let client = SentioApiClient::with_config(config);
        let result = client.health_check().await;

        // Should fail due to connection timeout/error
        assert!(result.is_err());
    }

    #[test]
    fn test_upload_request_validation() {
        let request = create_test_upload_request();

        assert!(!request.processor_name.is_empty());
        assert!(!request.version.is_empty());
        assert!(!request.target_network.is_empty());
        assert!(!request.binary_data.is_empty());

        // Verify WASM magic bytes
        assert_eq!(&request.binary_data[0..4], &[0x00, 0x61, 0x73, 0x6d]);
    }

    #[test]
    fn test_api_client_builder_pattern() {
        let client = SentioApiClient::new().with_api_key("test-key".to_string());

        assert_eq!(client.config().api_key, Some("test-key".to_string()));
        assert_eq!(client.config().base_url, "https://api.sentio.xyz");
    }

    #[test]
    fn test_error_handling_structures() {
        // Test that our error structures can handle various API error formats
        let minimal_error = r#"{"error": "Error", "message": "Something went wrong"}"#;
        let error: Result<ApiError, _> = serde_json::from_str(minimal_error);
        assert!(error.is_ok());

        let error = error.unwrap();
        assert_eq!(error.error, "Error");
        assert_eq!(error.message, "Something went wrong");
        assert!(error.code.is_none());
    }
}
