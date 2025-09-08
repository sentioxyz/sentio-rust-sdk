use super::Command;
use crate::utils::{
    api_client::SentioApiClient, config::ConfigManager, host_config::get_finalized_host,
    storage::CredentialStore,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use dialoguer::Confirm;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct UploadCommand {
    pub path: String,
    pub host: Option<String>,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub api_key: Option<String>,
    pub token: Option<String>,
    pub continue_from: Option<u32>,
    pub nobuild: bool,
    pub debug: bool,
    pub silent_overwrite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectConfig {
    pub name: String,
    pub host: String,
    pub project: String,
    pub debug: bool,
    pub build: bool,
    pub silent_overwrite: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct InitUploadResponse {
    pub url: String,
    pub warning: Option<String>,
    pub replacing_version: Option<u32>,
    pub multi_version: bool,
    pub project_id: String,
}

impl Default for InitUploadResponse {
    fn default() -> Self {
        Self {
            url: String::new(),
            warning: None,
            replacing_version: None,
            multi_version: false,
            project_id: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct FinishUploadResponse {
    pub project_full_slug: String,
    pub processor_id: String,
    pub version: u32,
}

impl Default for FinishUploadResponse {
    fn default() -> Self {
        Self {
            project_full_slug: String::new(),
            processor_id: String::new(),
            version: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct ProcessorStatus {
    pub version: u32,
    pub version_state: String,
}

impl Default for ProcessorStatus {
    fn default() -> Self {
        Self {
            version: 0,
            version_state: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct ProcessorsResponse {
    pub processors: Vec<ProcessorStatus>,
}

impl Default for ProcessorsResponse {
    fn default() -> Self {
        Self {
            processors: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct UserInfo {
    pub username: String,
}

impl Default for UserInfo {
    fn default() -> Self {
        Self {
            username: String::new(),
        }
    }
}

#[async_trait]
impl Command for UploadCommand {
    async fn execute(&self) -> Result<()> {
        println!("Preparing to upload...");

        // Load and finalize configuration
        let config = self.load_config().await?;

        // Set up authentication
        let auth_headers = self.setup_authentication(&config).await?;

        // Try to find binary file first
        let binary_path = match self.try_find_binary_file() {
            Ok(path) => {
                println!("Found existing binary: {}", path);
                path
            }
            Err(_) => {
                // Binary not found, build if needed and allowed
                if config.build && !self.nobuild {
                    println!("Binary not found, building processor...");
                    self.build_processor().await?;

                    // Now find the binary that should have been created
                    self.find_binary_file()?
                } else {
                    return Err(anyhow!("Binary not found and building is disabled (use --nobuild=false or ensure binary exists)"));
                }
            }
        };

        // Upload the processor
        self.upload_processor(&config, &auth_headers, &binary_path)
            .await?;

        Ok(())
    }
}

impl UploadCommand {
    async fn load_config(&self) -> Result<ProjectConfig> {
        // Try to load sentio.yaml configuration from the project path
        let mut config_manager = ConfigManager::new(&self.path);
        let _ = config_manager.load(); // Load configs if available

        // Get host
        let host = get_finalized_host(self.host.as_deref());

        // Get project name with priority: --name > --owner/default > cargo package name > directory name
        let project_name = if let Some(name) = &self.name {
            name.clone()
        } else if let Some(owner) = &self.owner {
            format!("{}/{}", owner, "default-project")
        } else {
            // Try to get package name from Cargo.toml first
            match self.get_package_name_from_cargo_toml().await {
                Ok(package_name) => package_name,
                Err(_) => {
                    // Fall back to directory name if Cargo.toml parsing fails
                    Path::new(&self.path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("default-project")
                        .to_string()
                }
            }
        };

        Ok(ProjectConfig {
            name: project_name.clone(),
            host: host.clone(),
            project: project_name,
            debug: self.debug,
            build: !self.nobuild,
            silent_overwrite: self.silent_overwrite,
        })
    }

    async fn get_package_name_from_cargo_toml(&self) -> Result<String> {
        let cargo_toml_path = Path::new(&self.path).join("Cargo.toml");
        let cargo_toml_content = tokio::fs::read_to_string(&cargo_toml_path)
            .await
            .context("Failed to read Cargo.toml")?;
        let cargo_toml: toml::Value =
            toml::from_str(&cargo_toml_content).context("Failed to parse Cargo.toml")?;
        
        if let Some(package) = cargo_toml.get("package") {
            if let Some(name) = package.get("name").and_then(|n| n.as_str()) {
                return Ok(name.to_string());
            }
        }
        
        Err(anyhow!("No package name found in Cargo.toml"))
    }

    async fn setup_authentication(
        &self,
        config: &ProjectConfig,
    ) -> Result<HashMap<String, String>> {
        let mut headers = HashMap::new();

        // Priority: api_key > token > stored credentials
        if let Some(api_key) = &self.api_key {
            headers.insert("api-key".to_string(), api_key.clone());
        } else if let Some(token) = &self.token {
            headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        } else {
            // Try to get stored credentials
            let store = CredentialStore::new();
            if let Some(stored_api_key) = store.get_credentials(&config.host)? {
                headers.insert("api-key".to_string(), stored_api_key);
            } else {
                let is_prod = config.host == "https://app.sentio.xyz";
                let cmd = if is_prod {
                    "sentio auth login"
                } else {
                    &format!("sentio auth login --host={}", config.host)
                };
                return Err(anyhow!(
                    "No credentials found for {}. Please run `{}`.",
                    config.host,
                    cmd
                ));
            }
        }

        Ok(headers)
    }

    async fn build_processor(&self) -> Result<()> {
        use crate::commands::build::BuildCommand;

        let build_cmd = BuildCommand {
            path: self.path.clone(),
            skip_validation: false,
            target: Some("x86_64-unknown-linux-musl".to_string()), // Default to x86-linux
            optimization_level: Some("release".to_string()),
            features: Vec::new(),
            verbose: false,
        };

        build_cmd.execute().await
    }

    /// Try to find the binary file, returns Err if not found (used for conditional building)
    fn try_find_binary_file(&self) -> Result<String> {
        use crate::commands::build::{BuildOptions, CrossCompiler};

        // Use the project path to find binary
        let project_path = Path::new(&self.path);
        let build_options = BuildOptions::default();

        let compiler = CrossCompiler::new(build_options.target.clone());

        // Use async runtime to call the async locate_binary method
        let binary_path_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { compiler.locate_binary(project_path, &build_options).await })
        });

        match binary_path_result {
            Ok(path) => Ok(path.to_string_lossy().to_string()),
            Err(_) => Err(anyhow!("Binary not found, will need to build")),
        }
    }

    /// Find the binary file, fails with clear error if not found (used after building)
    fn find_binary_file(&self) -> Result<String> {
        use crate::commands::build::{BuildOptions, CrossCompiler};

        // Use the project path to find binary
        let project_path = Path::new(&self.path);
        let build_options = BuildOptions::default();

        let compiler = CrossCompiler::new(build_options.target.clone());

        // Use async runtime to call the async locate_binary method
        let binary_path = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { compiler.locate_binary(project_path, &build_options).await })
        })?;

        Ok(binary_path.to_string_lossy().to_string())
    }

    async fn upload_processor(
        &self,
        config: &ProjectConfig,
        auth_headers: &HashMap<String, String>,
        binary_path: &str,
    ) -> Result<()> {
        let _client = SentioApiClient::new();

        // Read and hash the binary file
        let binary_data = fs::read(binary_path)
            .with_context(|| format!("Failed to read binary file: {}", binary_path))?;

        let file_size = binary_data.len();
        println!("Binary file size: {}KB", file_size / 1024);

        // Calculate SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(&binary_data);
        let sha256_hash = format!("{:x}", hasher.finalize());

        // Get user info to construct full project path
        println!("Debug: config.project = '{}'", config.project);
        let full_project_name = if !config.project.contains('/') {
            let user_info = self.get_user_info(config, auth_headers).await?;
            println!("Debug: user_info.username = '{}'", user_info.username);
            let full_name = format!("{}/{}", user_info.username, config.project);
            println!("Debug: full_project_name = '{}'", full_name);
            full_name
        } else {
            println!("Debug: using config.project as-is = '{}'", config.project);
            config.project.clone()
        };

        // Check continue_from if specified
        if let Some(version) = self.continue_from {
            self.validate_continue_from(config, auth_headers, &full_project_name, version)
                .await?;
        }

        // Initialize upload
        let init_response = self
            .init_upload(config, auth_headers, &full_project_name)
            .await?;

        // Handle version replacement confirmation
        if let Some(replacing_version) = init_response.replacing_version {
            if self.continue_from.is_none() && !config.silent_overwrite {
                let version_type = if init_response.multi_version {
                    "pending"
                } else {
                    "active"
                };
                let confirmed = Confirm::new()
                    .with_prompt(&format!(
                        "Create new version and deactivate {} version {}?",
                        version_type, replacing_version
                    ))
                    .interact()?;

                if !confirmed {
                    println!("Upload cancelled.");
                    return Ok(());
                }
            }
        }

        if let Some(warning) = &init_response.warning {
            if !warning.is_empty() {
                println!("⚠️  Warning: {}", warning);
            }
        }

        // Upload the binary to the presigned URL with retry
        self.upload_with_retry(&init_response.url, &binary_data)
            .await?;

        // Get git information
        let (commit_sha, git_url) = self.get_git_info();

        // Finish the upload
        let finish_response = self
            .finish_upload(
                config,
                auth_headers,
                &full_project_name,
                &sha256_hash,
                &commit_sha,
                &git_url,
                self.continue_from,
                init_response.warning.as_ref().map(|w| vec![w.clone()]),
            )
            .await?;

        // Print success information
        println!("✅ Upload successful!");
        println!("   SHA256: {}", sha256_hash);
        if !commit_sha.is_empty() {
            println!("   Git commit: {}", commit_sha);
        }
        println!("   Project: {}", full_project_name);
        println!("   Version: {}", finish_response.version);
        println!(
            "   Status URL: {}/{}/datasource/{}",
            config.host, full_project_name, finish_response.processor_id
        );

        Ok(())
    }

    async fn get_user_info(
        &self,
        config: &ProjectConfig,
        auth_headers: &HashMap<String, String>,
    ) -> Result<UserInfo> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/users", config.host);

        let response = client
            .get(&url)
            .headers(self.headers_to_reqwest(auth_headers)?)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get user info: {}", response.status()));
        }

        let user_info: UserInfo = response.json().await?;
        Ok(user_info)
    }

    async fn validate_continue_from(
        &self,
        config: &ProjectConfig,
        auth_headers: &HashMap<String, String>,
        project: &str,
        version: u32,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/processors/{}/status?version=ALL",
            config.host, project
        );

        let response = client
            .get(&url)
            .headers(self.headers_to_reqwest(auth_headers)?)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get processor status: {}",
                response.status()
            ));
        }

        let status: ProcessorsResponse = response.json().await?;
        let found = status.processors.iter().find(|p| p.version == version);

        if let Some(processor) = found {
            if !config.silent_overwrite {
                let confirmed = Confirm::new()
                    .with_prompt(&format!("Continue from version {} (status: {})?", version, processor.version_state))
                    .interact()?;

                if !confirmed {
                    std::process::exit(0);
                }
            }
        } else {
            let latest = status.processors.iter().map(|p| p.version).max();
            let latest_msg = if let Some(latest) = latest {
                format!(", latest is {}", latest)
            } else {
                String::new()
            };

            return Err(anyhow!(
                "Failed to find existing version {} in {}{}",
                version,
                project,
                latest_msg
            ));
        }

        Ok(())
    }

    async fn init_upload(
        &self,
        config: &ProjectConfig,
        auth_headers: &HashMap<String, String>,
        project: &str,
    ) -> Result<InitUploadResponse> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/processors/init_upload", config.host);

        let payload = serde_json::json!({
            "project_slug": project,
            "sdk_version": "2.0.0-development", // TODO: Get actual SDK version
            "sequence": 0,
            "contentType": "application/zip"
        });

        let response = client
            .post(&url)
            .headers(self.headers_to_reqwest(auth_headers)?)
            .json(&payload)
            .send()
            .await?;

        // Handle 404 or 500 with "record not found" - both indicate project doesn't exist
        let response_status = response.status();
        let body = response.text().await.unwrap_or_default();
        let should_create_project = if response_status == 404 {
            true
        } else if response_status == 500 {
            body.to_lowercase().contains("record not found")
        } else {
            false
        };

        if should_create_project {
            // Project not found, try to create it
            self.create_project_if_needed(config, auth_headers, project)
                .await?;
            // Retry init_upload
            let response = client
                .post(&url)
                .headers(self.headers_to_reqwest(auth_headers)?)
                .json(&payload)
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(anyhow!(
                    "Failed to initialize upload after project creation: {}",
                    response.status()
                ));
            }

            let init_response: InitUploadResponse = response.json().await?;
            return Ok(init_response);
        }

        if !response_status.is_success() {
            let status_text = response_status;
             return Err(anyhow!(
                "Failed to initialize upload: {}, body: {}",
                status_text,
                body
            ));
        }

        let init_response: InitUploadResponse = serde_json::from_str(&body)?;
        Ok(init_response)
    }

    async fn create_project_if_needed(
        &self,
        config: &ProjectConfig,
        auth_headers: &HashMap<String, String>,
        project: &str,
    ) -> Result<()> {
        let create_project = if config.silent_overwrite {
            true
        } else {
            Confirm::new()
                .with_prompt(&format!("Project not found for '{}', do you want to create it?", project))
                .interact()?
        };

        if !create_project {
            return Err(anyhow!("Upload cancelled - project '{}' not found", project));
        }

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/projects", config.host);

        let (owner_name, slug) = if project.contains('/') {
            let parts: Vec<&str> = project.split('/').collect();
            (Some(parts[0].to_string()), parts[1].to_string())
        } else {
            (None, project.to_string())
        };

        let payload = serde_json::json!({
            "slug": slug,
            "ownerName": owner_name,
            "visibility": "PRIVATE"
        });

        let response = client
            .post(&url)
            .headers(self.headers_to_reqwest(auth_headers)?)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to create project: {}", error_text));
        }

        println!("✅ Project created");
        Ok(())
    }

    async fn upload_file(&self, upload_url: &str, data: &[u8]) -> Result<()> {
        let client = reqwest::Client::new();

        // Create a ZIP file in memory
        let mut zip_buffer = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_buffer);
            let mut zip = zip::ZipWriter::new(cursor);

            let mut options: zip::write::FileOptions<()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            // Set the permissions to 755 (executable)
            options = options.unix_permissions(0o755);

            // Always rename the binary file to "main" in the zip package
            zip.start_file("main", options)?;
            std::io::Write::write_all(&mut zip, data)?;
            zip.finish()?;
        }

        println!(
            "Uploading binary ({} bytes compressed)...",
            zip_buffer.len()
        );

        let response = client
            .put(upload_url)
            .header("Content-Type", "application/zip")
            .header("Content-Length", zip_buffer.len().to_string())
            .body(zip_buffer)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to upload to GCS: {}", error_text));
        }

        Ok(())
    }

    async fn upload_with_retry(&self, upload_url: &str, data: &[u8]) -> Result<()> {
        const MAX_RETRIES: usize = 5;
        const BASE_DELAY_MS: u64 = 1000;

        for attempt in 1..=MAX_RETRIES {
            match self.upload_file(upload_url, data).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt == MAX_RETRIES {
                        return Err(e);
                    }

                    let delay_ms = BASE_DELAY_MS * attempt as u64;
                    println!(
                        "Upload attempt {} failed, retrying in {}ms...",
                        attempt, delay_ms
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        unreachable!()
    }

    fn get_git_info(&self) -> (String, String) {
        let commit_sha = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout).ok()
                } else {
                    None
                }
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let git_url = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout).ok()
                } else {
                    None
                }
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        (commit_sha, git_url)
    }

    async fn finish_upload(
        &self,
        config: &ProjectConfig,
        auth_headers: &HashMap<String, String>,
        project: &str,
        sha256: &str,
        commit_sha: &str,
        git_url: &str,
        continue_from: Option<u32>,
        warnings: Option<Vec<String>>,
    ) -> Result<FinishUploadResponse> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/processors/finish_upload", config.host);

        let payload = serde_json::json!({
            "project_slug": project,
            "cli_version": "2.0.0-development", // TODO: Get actual CLI version
            "sdk_version": "2.0.0-development", // TODO: Get actual SDK version
            "sha256": sha256,
            "commit_sha": commit_sha,
            "git_url": git_url,
            "debug": config.debug,
            "sequence": 0,
            "continueFrom": continue_from,
            "warnings": warnings,
            "binary": true
        });

        let response = client
            .post(&url)
            .headers(self.headers_to_reqwest(auth_headers)?)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to finish upload: {}", error_text));
        }

        let finish_response: FinishUploadResponse = response.json().await?;
        Ok(finish_response)
    }

    fn headers_to_reqwest(
        &self,
        headers: &HashMap<String, String>,
    ) -> Result<reqwest::header::HeaderMap> {
        let mut header_map = reqwest::header::HeaderMap::new();

        for (key, value) in headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
            let header_value = reqwest::header::HeaderValue::from_str(value)?;
            header_map.insert(header_name, header_value);
        }

        Ok(header_map)
    }
}
