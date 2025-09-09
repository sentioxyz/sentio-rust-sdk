use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio::time::Duration;
use warp::Filter;

const OAUTH_PORT: u16 = 20000;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// PKCE (Proof Key for Code Exchange) utilities
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

impl Default for PkceChallenge {
    fn default() -> Self {
        Self::new()
    }
}

impl PkceChallenge {
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

        Self { verifier, challenge }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
}

impl Default for TokenResponse {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            token_type: "Bearer".to_string(),
            expires_in: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct UserResponse {
    #[serde(rename = "emailVerified")]
    pub email_verified: bool,
}


#[derive(Debug, Deserialize, Serialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scopes: Vec<String>,
    pub source: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct CreateApiKeyResponse {
    pub key: String,
    pub username: String,
}


/// OAuth2 login server
pub struct OAuthServer {
    pub host: String,
    pub verifier: String,
}

impl OAuthServer {
    pub fn new(host: String, verifier: String) -> Self {
        Self { host, verifier }
    }

    /// Start the OAuth callback server
    pub async fn start_callback_server(&self) -> Result<String> {
        use std::sync::{Arc, Mutex};

        let host = self.host.clone();
        let verifier = self.verifier.clone();
        let result: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));
        let username: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        
        let result_clone = result.clone();
        let username_clone = username.clone();

        // Start the web server
        let callback_route = warp::path("callback")
            .and(warp::query::<HashMap<String, String>>())
            .and_then(move |params: HashMap<String, String>| {
                let host = host.clone();
                let verifier = verifier.clone();
                let result = result_clone.clone();
                let username = username_clone.clone();

                async move {
                    match Self::handle_callback(host, verifier, params).await {
                        Ok((api_key, user)) => {
                            if let Ok(mut res) = result.lock() {
                                *res = Some(Ok(api_key));
                            }
                            if let Ok(mut user_res) = username.lock() {
                                *user_res = Some(user.clone());
                            }
                            Ok::<warp::reply::Html<String>, std::convert::Infallible>(warp::reply::html(format!(
                                "<h1>🎉 Login Success!</h1><p>You have successfully logged in as <strong>{}</strong>.</p><p>You can now close this window and return to the CLI.</p>",
                                user
                            )))
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            if let Ok(mut res) = result.lock() {
                                *res = Some(Err(e));
                            }
                            Ok::<warp::reply::Html<String>, std::convert::Infallible>(warp::reply::html(format!(
                                "<h1>❌ Login Failed</h1><p><strong>Error:</strong> {}</p><p>Please close this window and try again in the CLI.</p>",
                                error_msg
                            )))
                        }
                    }
                }
            });

        // Create the warp server
        let routes = callback_route;
        let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(
            ([127, 0, 0, 1], OAUTH_PORT),
            async {
                tokio::time::sleep(CALLBACK_TIMEOUT).await;
            }
        );

        println!("Starting OAuth callback server on http://localhost:{}/callback", OAUTH_PORT);

        // Run server in background
        let server_handle = tokio::spawn(server);

        // Poll for result with timeout
        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed() > CALLBACK_TIMEOUT {
                server_handle.abort();
                return Err(anyhow!("OAuth callback timeout after {} seconds", CALLBACK_TIMEOUT.as_secs()));
            }

            if let Ok(mut res) = result.lock()
                && let Some(callback_result) = res.take() {
                    server_handle.abort();
                    return callback_result;
                }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn handle_callback(
        host: String,
        verifier: String,
        params: HashMap<String, String>,
    ) -> Result<(String, String)> {
        // Get the authorization code
        let code = params.get("code").ok_or_else(|| anyhow!("Missing authorization code"))?;

        // Exchange code for access token
        let access_token = Self::exchange_code_for_token(&host, code, &verifier).await?;

        // Verify user account
        Self::verify_user_account(&host, &access_token).await?;

        // Create API key and get username
        let (api_key, username) = Self::create_api_key(&host, &access_token).await?;

        Ok((api_key, username))
    }

    async fn exchange_code_for_token(host: &str, code: &str, verifier: &str) -> Result<String> {
        use crate::utils::host_config::get_auth_config;

        let auth_config = get_auth_config(host);
        if auth_config.domain.is_empty() {
            return Err(anyhow!("Invalid host configuration"));
        }

        let client = reqwest::Client::new();
        let token_url = format!("{}/oauth/token", auth_config.domain);

        let mut form = HashMap::new();
        form.insert("grant_type", "authorization_code");
        form.insert("client_id", &auth_config.client_id);
        form.insert("code_verifier", verifier);
        form.insert("code", code);
        form.insert("redirect_uri", &auth_config.redirect_uri);

        let response = client
            .post(&token_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&form)
            .send()
            .await
            .context("Failed to exchange code for token")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Token exchange failed: {}", error_text));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        Ok(token_response.access_token)
    }

    async fn verify_user_account(host: &str, access_token: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let user_url = format!("{}/api/v1/users", host);

        let response = client
            .get(&user_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("version", "1.0.0") // CLI version
            .send()
            .await
            .context("Failed to get user info")?;

        if response.status() == 401 {
            return Err(anyhow!("Account does not exist, please sign up on Sentio first"));
        }

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to get user info: {}", error_text));
        }

        let user_response: UserResponse = response
            .json()
            .await
            .context("Failed to parse user response")?;

        if !user_response.email_verified {
            return Err(anyhow!("Your account is not verified, please verify your email first"));
        }

        Ok(())
    }

    async fn create_api_key(host: &str, access_token: &str) -> Result<(String, String)> {
        let client = reqwest::Client::new();
        let api_key_url = format!("{}/api/v1/keys", host);

        // Generate a unique API key name
        let hostname = gethostname::gethostname()
            .to_string_lossy()
            .to_string();
        let mut bytes = [0u8; 4];
        rand::thread_rng().fill_bytes(&mut bytes);
        let random_suffix = hex::encode(bytes);
        let api_key_name = format!("{}-{}", hostname, random_suffix);

        let create_request = CreateApiKeyRequest {
            name: api_key_name,
            scopes: vec!["write:project".to_string()],
            source: "sdk_generated".to_string(),
        };

        let response = client
            .post(&api_key_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("version", "1.0.0") // CLI version
            .json(&create_request)
            .send()
            .await
            .context("Failed to create API key")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to create API key: {}", error_text));
        }

        let create_response: CreateApiKeyResponse = response
            .json()
            .await
            .context("Failed to parse create API key response")?;

        println!("✓ Login success with {}", create_response.username);
        Ok((create_response.key, create_response.username))
    }
}

/// Generate OAuth authorization URL
pub fn build_authorization_url(host: &str, challenge: &str) -> Result<String> {
    use crate::utils::host_config::get_auth_config;

    let auth_config = get_auth_config(host);
    if auth_config.domain.is_empty() {
        return Err(anyhow!("Invalid host, try login with an API key if it is a dev env"));
    }

    let redirect_uri = auth_config.redirect_uri;

    let mut url = format!("{}/authorize?", auth_config.domain);
    url.push_str("response_type=code&");
    url.push_str(&format!("code_challenge={}&", challenge));
    url.push_str("code_challenge_method=S256&");
    url.push_str(&format!("client_id={}&", auth_config.client_id));
    url.push_str(&format!("redirect_uri={}&", urlencoding::encode(&redirect_uri)));
    url.push_str(&format!("audience={}&", urlencoding::encode(&auth_config.audience)));
    url.push_str("prompt=login");

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_challenge_generation() {
        let challenge1 = PkceChallenge::new();
        let challenge2 = PkceChallenge::new();

        // Verifiers should be different
        assert_ne!(challenge1.verifier, challenge2.verifier);
        assert_ne!(challenge1.challenge, challenge2.challenge);

        // Both should be base64 URL-safe encoded
        assert!(!challenge1.verifier.contains('+'));
        assert!(!challenge1.verifier.contains('/'));
        assert!(!challenge1.verifier.contains('='));
        
        assert!(!challenge1.challenge.contains('+'));
        assert!(!challenge1.challenge.contains('/'));
        assert!(!challenge1.challenge.contains('='));
    }

    #[test]
    fn test_build_authorization_url() {
        let challenge = "test_challenge";
        let result = build_authorization_url("https://app.sentio.xyz", challenge);
        
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.contains("https://auth.sentio.xyz"));
        assert!(url.contains("code_challenge=test_challenge"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fapp.sentio.xyz%2Fredirect%2Fsdk"));
    }

    #[test]
    fn test_build_authorization_url_invalid_host() {
        let challenge = "test_challenge";
        let result = build_authorization_url("https://invalid.host.com", challenge);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid host"));
    }
}