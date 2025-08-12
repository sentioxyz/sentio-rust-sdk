use anyhow::{anyhow, Result};
use async_trait::async_trait;
use super::Command;
use crate::utils::{
    api_client::SentioApiClient,
    host_config::get_finalized_host,
    oauth::{build_authorization_url, OAuthServer, PkceChallenge},
    storage::CredentialStore,
};

pub struct AuthCommand {
    pub action: AuthAction,
    pub host: Option<String>,
    pub api_key: Option<String>,
}

pub enum AuthAction {
    Login,
    Logout,
    Status,
}

impl AuthCommand {
    pub fn new(action: AuthAction) -> Self {
        Self {
            action,
            host: None,
            api_key: None,
        }
    }

    pub fn with_host(mut self, host: Option<String>) -> Self {
        self.host = host;
        self
    }

    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }
}

#[async_trait]
impl Command for AuthCommand {
    async fn execute(&self) -> Result<()> {
        match self.action {
            AuthAction::Login => self.login().await,
            AuthAction::Logout => self.logout().await,
            AuthAction::Status => self.status().await,
        }
    }
}

impl AuthCommand {
    async fn login(&self) -> Result<()> {
        let host = get_finalized_host(self.host.as_deref());
        println!("Logging into {}", host);

        if let Some(api_key) = &self.api_key {
            // Direct API key login
            self.login_with_api_key(&host, api_key).await
        } else {
            // OAuth flow
            self.login_with_oauth(&host).await
        }
    }

    async fn login_with_api_key(&self, host: &str, api_key: &str) -> Result<()> {
        let client = SentioApiClient::new();
        
        match client.check_key(host, api_key).await {
            Ok(response) => {
                let store = CredentialStore::new();
                store.store_credentials(host, api_key)?;
                println!("✓ Login success with {}", response.username);
                Ok(())
            }
            Err(e) => {
                println!("✗ Login failed: {}", e);
                Err(e)
            }
        }
    }

    async fn login_with_oauth(&self, host: &str) -> Result<()> {
        // Generate PKCE challenge
        let pkce = PkceChallenge::new();

        // Build authorization URL
        let auth_url = build_authorization_url(host, &pkce.challenge)?;

        println!("Continue your authorization in the browser");
        
        // Try to open browser
        if let Err(e) = webbrowser::open(&auth_url) {
            println!("Unable to open browser: {}", e);
            println!("Open this url in your browser: {}", auth_url);
        }

        // Start OAuth server and wait for callback
        let oauth_server = OAuthServer::new(host.to_string(), pkce.verifier);
        match oauth_server.start_callback_server().await {
            Ok(api_key) => {
                let store = CredentialStore::new();
                store.store_credentials(host, &api_key)?;
                // Success message is already printed in OAuth server callback
                Ok(())
            }
            Err(e) => {
                println!("✗ OAuth login failed: {}", e);
                Err(e)
            }
        }
    }

    async fn logout(&self) -> Result<()> {
        let host = get_finalized_host(self.host.as_deref());
        let store = CredentialStore::new();
        
        if store.remove_credentials(&host)? {
            println!("✓ Logged out from {}", host);
        } else {
            println!("No credentials found for {}", host);
        }
        
        Ok(())
    }

    async fn status(&self) -> Result<()> {
        let host = get_finalized_host(self.host.as_deref());
        let store = CredentialStore::new();
        
        match store.get_credentials(&host)? {
            Some(api_key) => {
                // Verify the API key is still valid
                let client = SentioApiClient::new();
                match client.check_key(&host, &api_key).await {
                    Ok(response) => {
                        println!("✓ Logged in to {} as {}", host, response.username);
                        Ok(())
                    }
                    Err(_) => {
                        println!("✗ Credentials for {} are invalid or expired", host);
                        store.remove_credentials(&host)?;
                        Err(anyhow!("Invalid credentials"))
                    }
                }
            }
            None => {
                println!("✗ Not logged in to {}", host);
                Err(anyhow!("Not logged in"))
            }
        }
    }
}