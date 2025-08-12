use std::collections::HashMap;

/// Host mapping similar to TypeScript implementation
pub fn get_host_map() -> HashMap<&'static str, &'static str> {
    let mut hosts = HashMap::new();
    hosts.insert("local", "http://localhost:10000");
    hosts.insert("test", "https://test.sentio.xyz");
    hosts.insert("staging", "https://staging.sentio.xyz");
    hosts.insert("prod", "https://app.sentio.xyz");
    hosts
}

/// Get finalized host URL, defaulting to prod if not specified
pub fn get_finalized_host(host: Option<&str>) -> String {
    let host = host.unwrap_or("prod");
    let host_map = get_host_map();
    
    host_map.get(host)
        .map(|url| url.to_string())
        .unwrap_or_else(|| host.to_string())
}

/// OAuth2/Auth0 configuration for different hosts
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub domain: String,
    pub client_id: String,
    pub audience: String,
    pub redirect_uri: String,
}

/// Get Auth0 configuration for a given host
pub fn get_auth_config(host: &str) -> AuthConfig {
    match host {
        "http://localhost:10000" => AuthConfig {
            domain: "https://sentio-dev.us.auth0.com".to_string(),
            client_id: "JREam3EysMTM49eFbAjNK02OCykpmda3".to_string(),
            audience: "http://localhost:8080/v1".to_string(),
            redirect_uri: "http://localhost:10000/redirect/sdk".to_string(),
        },
        "https://app.sentio.xyz" => AuthConfig {
            domain: "https://auth.sentio.xyz".to_string(),
            client_id: "66oqMrep54LVI9ckH97cw8C4GBA1cpKW".to_string(),
            audience: "https://app.sentio.xyz/api/v1".to_string(),
            redirect_uri: "https://app.sentio.xyz/redirect/sdk".to_string(),
        },
        "https://test.sentio.xyz" | "https://staging.sentio.xyz" => AuthConfig {
            domain: "https://auth.test.sentio.xyz".to_string(),
            client_id: "6SH2S1qJ2yYqyYGCQOcEnGsYgoyONTxM".to_string(),
            audience: "https://test.sentio.xyz/api/v1".to_string(),
            redirect_uri: "https://test.sentio.xyz/redirect/sdk".to_string(),
        },
        _ => AuthConfig {
            domain: String::new(),
            client_id: String::new(),
            audience: String::new(),
            redirect_uri: String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_finalized_host() {
        assert_eq!(get_finalized_host(None), "https://app.sentio.xyz");
        assert_eq!(get_finalized_host(Some("prod")), "https://app.sentio.xyz");
        assert_eq!(get_finalized_host(Some("local")), "http://localhost:10000");
        assert_eq!(get_finalized_host(Some("test")), "https://test.sentio.xyz");
        assert_eq!(get_finalized_host(Some("staging")), "https://staging.sentio.xyz");
        assert_eq!(get_finalized_host(Some("https://custom.sentio.xyz")), "https://custom.sentio.xyz");
    }

    #[test]
    fn test_auth_config_prod() {
        let config = get_auth_config("https://app.sentio.xyz");
        assert_eq!(config.domain, "https://auth.sentio.xyz");
        assert_eq!(config.client_id, "66oqMrep54LVI9ckH97cw8C4GBA1cpKW");
        assert_eq!(config.audience, "https://app.sentio.xyz/api/v1");
        assert_eq!(config.redirect_uri, "https://app.sentio.xyz/redirect/sdk");
    }

    #[test]
    fn test_auth_config_test() {
        let config = get_auth_config("https://test.sentio.xyz");
        assert_eq!(config.domain, "https://auth.test.sentio.xyz");
        assert_eq!(config.client_id, "6SH2S1qJ2yYqyYGCQOcEnGsYgoyONTxM");
    }

    #[test]
    fn test_auth_config_custom() {
        let config = get_auth_config("https://custom.example.com");
        assert_eq!(config.domain, "");
        assert_eq!(config.client_id, "");
    }
}