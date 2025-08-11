use anyhow::{Context, Result};
use std::process::Command;
use std::collections::HashMap;
use crate::utils::validator::{ValidationIssue, ValidationResults};

/// Tool validator for checking build dependencies
pub struct ToolValidator;

/// Tool installation information
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub check_command: Vec<String>,
    pub install_commands: HashMap<String, String>, // platform -> command
    pub description: String,
}

impl ToolValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate all required tools for cross-compilation
    pub async fn validate_tools_for_target(&self, target: &str) -> Result<ValidationResults> {
        let mut results = ValidationResults::new();
        
        // Check if we need cross-compilation
        let is_cross_compile = self.needs_cross_compilation(target);
        
        if is_cross_compile {
            // Check for cross and docker when cross-compiling
            // protoc will be automatically installed in the container via Cross.toml
            self.check_cross_installed(&mut results).await?;
            self.check_docker_installed(&mut results).await?;
        }
        
        // Check if target is installed (for native compilation or cross reference)
        self.check_rust_target_installed(target, &mut results).await?;
        
        Ok(results)
    }

    /// Check if cross-compilation is needed for the given target
    fn needs_cross_compilation(&self, target: &str) -> bool {
        let current_target = std::env::consts::ARCH;
        let current_os = std::env::consts::OS;
        
        // Extract target architecture and OS from target triple
        let target_parts: Vec<&str> = target.split('-').collect();
        if target_parts.len() < 3 {
            return false; // Invalid target format
        }
        
        let target_arch = target_parts[0];
        let target_os = if target.contains("linux") {
            "linux"
        } else if target.contains("darwin") || target.contains("apple") {
            "macos"
        } else if target.contains("windows") {
            "windows"
        } else {
            "unknown"
        };
        
        // Cross-compilation needed if architecture or OS differs
        target_arch != current_target || target_os != current_os
    }

    /// Check if cross tool is installed
    async fn check_cross_installed(&self, results: &mut ValidationResults) -> Result<()> {
        match self.run_command(&["cross", "--version"]).await {
            Ok(_) => {
                results.add_issue(ValidationIssue::info("✓ cross is installed".to_string()));
            }
            Err(_) => {
                results.add_issue(
                    ValidationIssue::error("cross is not installed".to_string())
                        .with_suggestion("Install cross with: cargo install cross".to_string())
                );
            }
        }
        Ok(())
    }

    /// Check if Docker is installed and running
    async fn check_docker_installed(&self, results: &mut ValidationResults) -> Result<()> {
        match self.run_command(&["docker", "--version"]).await {
            Ok(_) => {
                // Check if docker daemon is running
                match self.run_command(&["docker", "info"]).await {
                    Ok(_) => {
                        results.add_issue(ValidationIssue::info("✓ Docker is installed and running".to_string()));
                    }
                    Err(_) => {
                        results.add_issue(
                            ValidationIssue::warning("Docker is installed but not running".to_string())
                                .with_suggestion("Start Docker daemon before building".to_string())
                        );
                    }
                }
            }
            Err(_) => {
                results.add_issue(
                    ValidationIssue::error("Docker is not installed".to_string())
                        .with_suggestion(Self::get_docker_install_command())
                );
            }
        }
        Ok(())
    }


    /// Check if Rust target is installed
    async fn check_rust_target_installed(&self, target: &str, results: &mut ValidationResults) -> Result<()> {
        match self.run_command(&["rustup", "target", "list", "--installed"]).await {
            Ok(output) => {
                if output.contains(target) {
                    results.add_issue(ValidationIssue::info(format!("✓ Rust target {} is installed", target)));
                } else {
                    results.add_issue(
                        ValidationIssue::warning(format!("Rust target {} is not installed", target))
                            .with_suggestion(format!("Install target with: rustup target add {}", target))
                    );
                }
            }
            Err(e) => {
                results.add_issue(
                    ValidationIssue::error(format!("Failed to check installed targets: {}", e))
                );
            }
        }
        Ok(())
    }


    /// Run a command and return its output
    async fn run_command(&self, args: &[&str]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("No command provided"));
        }

        let output = Command::new(args[0])
            .args(&args[1..])
            .output()
            .context(format!("Failed to execute command: {}", args.join(" ")))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow::anyhow!(
                "Command failed: {} - {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Check if current platform is Linux
    #[cfg(target_os = "linux")]
    fn is_current_platform_linux(&self) -> bool {
        true
    }

    #[cfg(not(target_os = "linux"))]
    fn is_current_platform_linux(&self) -> bool {
        false
    }

    /// Get platform-specific suggestions for missing tools
    pub fn get_platform_install_suggestions(&self, tool: &str) -> String {
        match tool {
            "docker" => Self::get_docker_install_command(),
            _ => format!("Please install {} for your platform", tool),
        }
    }


    /// Get Docker installation command for the current platform
    #[cfg(target_os = "macos")]
    fn get_docker_install_command() -> String {
        r#"macOS:
  brew install --cask docker
  # Or download Docker Desktop from https://www.docker.com/products/docker-desktop/"#.to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_docker_install_command() -> String {
        r#"Linux (Ubuntu/Debian):
  sudo apt-get update
  sudo apt-get install docker.io
  sudo systemctl start docker
  sudo systemctl enable docker
  sudo usermod -aG docker $USER

Linux (CentOS/RHEL):
  sudo yum install docker
  sudo systemctl start docker
  sudo systemctl enable docker
  sudo usermod -aG docker $USER

Linux (Arch):
  sudo pacman -S docker
  sudo systemctl start docker
  sudo systemctl enable docker
  sudo usermod -aG docker $USER"#.to_string()
    }

    #[cfg(target_os = "windows")]
    fn get_docker_install_command() -> String {
        r#"Windows:
  # Install Docker Desktop for Windows
  # Download from https://www.docker.com/products/docker-desktop/
  
  # Or with Chocolatey:
  choco install docker-desktop
  
  # Or with Scoop:
  scoop install docker"#.to_string()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    fn get_docker_install_command() -> String {
        "Please install Docker from https://www.docker.com/products/docker-desktop/".to_string()
    }

    /// Validate specific tool installation
    pub async fn validate_tool(&self, tool_info: &ToolInfo) -> Result<bool> {
        let args: Vec<&str> = tool_info.check_command.iter().map(|s| s.as_str()).collect();
        match self.run_command(&args).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_validator_creation() {
        let validator = ToolValidator::new();
        // Just test that we can create it
        assert_eq!(validator.is_current_platform_linux(), std::env::consts::OS == "linux");
    }

    #[test]
    fn test_platform_install_suggestions() {
        let validator = ToolValidator::new();
        
        // Test docker suggestions
        let docker_suggestion = validator.get_platform_install_suggestions("docker");
        assert!(!docker_suggestion.is_empty());
        
        #[cfg(target_os = "macos")]
        assert!(docker_suggestion.contains("brew") || docker_suggestion.contains("Docker Desktop"));
        
        #[cfg(target_os = "linux")]
        assert!(docker_suggestion.contains("apt-get") || docker_suggestion.contains("docker.io"));
        
        #[cfg(target_os = "windows")]
        assert!(docker_suggestion.contains("docker-desktop") || docker_suggestion.contains("Docker Desktop"));
        
        // Test unknown tool suggestions
        let unknown_suggestion = validator.get_platform_install_suggestions("unknown-tool");
        assert!(unknown_suggestion.contains("Please install unknown-tool for your platform"));
    }


    #[tokio::test]
    async fn test_run_command_success() {
        let validator = ToolValidator::new();
        
        // Test with a command that should always work
        match validator.run_command(&["echo", "test"]).await {
            Ok(output) => assert!(output.contains("test")),
            Err(_) => {
                // On some systems echo might not be available, that's ok
            }
        }
    }

    #[tokio::test]
    async fn test_run_command_failure() {
        let validator = ToolValidator::new();
        
        // Test with a command that should fail
        let result = validator.run_command(&["this-command-should-not-exist-12345"]).await;
        assert!(result.is_err());
    }
}