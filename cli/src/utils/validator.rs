use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::fs;
use crate::utils::config::ConfigManager;

/// Validation issue severity
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

/// Validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
    pub suggestion: Option<String>,
    pub file_path: Option<String>,
}

impl ValidationIssue {
    pub fn error(message: String) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            message,
            suggestion: None,
            file_path: None,
        }
    }

    pub fn warning(message: String) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            message,
            suggestion: None,
            file_path: None,
        }
    }

    pub fn info(message: String) -> Self {
        Self {
            severity: ValidationSeverity::Info,
            message,
            suggestion: None,
            file_path: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    pub fn with_file(mut self, file_path: String) -> Self {
        self.file_path = Some(file_path);
        self
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity_str = match self.severity {
            ValidationSeverity::Error => "ERROR",
            ValidationSeverity::Warning => "WARNING",
            ValidationSeverity::Info => "INFO",
        };

        write!(f, "[{}] {}", severity_str, self.message)?;

        if let Some(file_path) = &self.file_path {
            write!(f, " (in {})", file_path)?;
        }

        if let Some(suggestion) = &self.suggestion {
            write!(f, "\n  Suggestion: {}", suggestion)?;
        }

        Ok(())
    }
}

/// Validation results
#[derive(Debug)]
pub struct ValidationResults {
    pub issues: Vec<ValidationIssue>,
}

impl Default for ValidationResults {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationResults {
    pub fn new() -> Self {
        Self { issues: vec![] }
    }

    pub fn add_issue(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == ValidationSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.issues.iter().any(|i| i.severity == ValidationSeverity::Warning)
    }

    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == ValidationSeverity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == ValidationSeverity::Warning).count()
    }

}

impl std::fmt::Display for ValidationResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.issues.is_empty() {
            return write!(f, "No validation issues found");
        }

        for issue in &self.issues {
            writeln!(f, "{}", issue)?;
        }

        let error_count = self.error_count();
        let warning_count = self.warning_count();

        write!(f, "\nSummary: {} error(s), {} warning(s)", error_count, warning_count)
    }
}

pub struct ProjectValidator;

impl Default for ProjectValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate a Sentio processor project
    pub fn validate_project(&self, project_path: &str) -> Result<()> {
        let results = self.validate_project_detailed(project_path)?;
        
        // Print all issues
        if !results.issues.is_empty() {
            println!("{}", results);
        }

        if results.has_errors() {
            return Err(anyhow!("Project validation failed with {} error(s)", results.error_count()));
        }

        if results.has_warnings() {
            println!("⚠️  Project validation completed with {} warning(s)", results.warning_count());
        }

        Ok(())
    }

    /// Validate project and return detailed results
    pub fn validate_project_detailed(&self, project_path: &str) -> Result<ValidationResults> {
        let mut results = ValidationResults::new();
        let project_path = Path::new(project_path);

        // Check if path exists
        if !project_path.exists() {
            results.add_issue(ValidationIssue::error(
                format!("Project path does not exist: {}", project_path.display())
            ));
            return Ok(results);
        }

        // Validate Cargo.toml
        self.validate_cargo_toml(project_path, &mut results)?;

        // Validate Sentio configuration
        self.validate_sentio_config(project_path, &mut results)?;

        // Validate source structure
        self.validate_source_structure(project_path, &mut results)?;

        // Validate dependencies
        self.validate_dependencies(project_path, &mut results)?;

        // Check for common issues
        self.check_common_issues(project_path, &mut results)?;

        Ok(results)
    }

    /// Validate Cargo.toml file
    fn validate_cargo_toml(&self, project_path: &Path, results: &mut ValidationResults) -> Result<()> {
        let cargo_toml_path = project_path.join("Cargo.toml");
        
        if !cargo_toml_path.exists() {
            results.add_issue(
                ValidationIssue::error("Cargo.toml not found".to_string())
                    .with_suggestion("Run 'cargo init' to create a new Rust project".to_string())
            );
            return Ok(());
        }

        let cargo_content = fs::read_to_string(&cargo_toml_path)
            .context("Failed to read Cargo.toml")?;

        let cargo_toml: toml::Value = match toml::from_str(&cargo_content) {
            Ok(value) => value,
            Err(e) => {
                results.add_issue(
                    ValidationIssue::error(format!("Invalid Cargo.toml format: {}", e))
                        .with_file("Cargo.toml".to_string())
                );
                return Ok(());
            }
        };

        // Check for required package fields
        if let Some(package) = cargo_toml.get("package") {
            if package.get("name").is_none() {
                results.add_issue(
                    ValidationIssue::error("Package name is missing in Cargo.toml".to_string())
                        .with_file("Cargo.toml".to_string())
                );
            }

            if package.get("version").is_none() {
                results.add_issue(
                    ValidationIssue::error("Package version is missing in Cargo.toml".to_string())
                        .with_file("Cargo.toml".to_string())
                );
            }
        } else {
            results.add_issue(
                ValidationIssue::error("Package section is missing in Cargo.toml".to_string())
                    .with_file("Cargo.toml".to_string())
            );
        }

        // Check for Sentio SDK dependency
        if let Some(dependencies) = cargo_toml.get("dependencies") {
            let has_sentio_sdk = dependencies.as_table()
                .map(|deps| deps.keys().any(|key| key.contains("sentio")))
                .unwrap_or(false);

            if !has_sentio_sdk {
                results.add_issue(
                    ValidationIssue::warning("No Sentio SDK dependency found".to_string())
                        .with_suggestion("Add sentio-sdk to your dependencies".to_string())
                        .with_file("Cargo.toml".to_string())
                );
            }
        }

        Ok(())
    }

    /// Validate Sentio configuration
    fn validate_sentio_config(&self, project_path: &Path, results: &mut ValidationResults) -> Result<()> {
        let mut config_manager = ConfigManager::new(project_path);
        
        match config_manager.load() {
            Ok(_) => {
                match config_manager.get_effective_config() {
                    Ok(config) => {
                        // Validate configuration
                        if let Err(e) = config.validate() {
                            results.add_issue(
                                ValidationIssue::error(format!("Invalid Sentio configuration: {}", e))
                                    .with_file("sentio.yaml".to_string())
                            );
                        }

                        // Check for empty contracts list
                        if config.contracts.is_empty() {
                            results.add_issue(
                                ValidationIssue::info("No contracts configured".to_string())
                                    .with_suggestion("Add contracts using 'sentio contract add <address>'".to_string())
                            );
                        }

                        // Validate build configuration
                        if config.build.target.is_empty() {
                            results.add_issue(
                                ValidationIssue::warning("Build target not specified".to_string())
                                    .with_suggestion("Specify a build target in sentio.yaml".to_string())
                            );
                        }
                    }
                    Err(e) => {
                        results.add_issue(
                            ValidationIssue::error(format!("Failed to load effective configuration: {}", e))
                        );
                    }
                }
            }
            Err(_) => {
                results.add_issue(
                    ValidationIssue::info("No Sentio configuration found, using defaults".to_string())
                        .with_suggestion("Create sentio.yaml to customize build settings and contracts".to_string())
                );
            }
        }

        Ok(())
    }

    /// Validate source code structure
    fn validate_source_structure(&self, project_path: &Path, results: &mut ValidationResults) -> Result<()> {
        let src_dir = project_path.join("src");
        
        if !src_dir.exists() {
            results.add_issue(
                ValidationIssue::error("src directory not found".to_string())
                    .with_suggestion("Create src directory with main.rs or lib.rs".to_string())
            );
            return Ok(());
        }

        let main_rs = src_dir.join("main.rs");
        let lib_rs = src_dir.join("lib.rs");

        if !main_rs.exists() && !lib_rs.exists() {
            results.add_issue(
                ValidationIssue::error("No main.rs or lib.rs found in src directory".to_string())
                    .with_suggestion("Create main.rs for binary or lib.rs for library".to_string())
            );
        }

        // Check for common Rust source files
        let common_files = ["mod.rs", "processor.rs", "handlers.rs"];
        for file in &common_files {
            let file_path = src_dir.join(file);
            if file_path.exists() {
                // Basic syntax check by trying to read the file
                if let Err(e) = fs::read_to_string(&file_path) {
                    results.add_issue(
                        ValidationIssue::warning(format!("Cannot read source file {}: {}", file, e))
                            .with_file(format!("src/{}", file))
                    );
                }
            }
        }

        Ok(())
    }

    /// Validate project dependencies
    fn validate_dependencies(&self, project_path: &Path, results: &mut ValidationResults) -> Result<()> {
        let cargo_lock = project_path.join("Cargo.lock");
        
        if !cargo_lock.exists() {
            results.add_issue(
                ValidationIssue::info("Cargo.lock not found".to_string())
                    .with_suggestion("Run 'cargo build' to generate Cargo.lock".to_string())
            );
        }

        // Check if target directory exists (indicates previous builds)
        let target_dir = project_path.join("target");
        if !target_dir.exists() {
            results.add_issue(
                ValidationIssue::info("No previous builds found".to_string())
                    .with_suggestion("This is the first build for this project".to_string())
            );
        }

        Ok(())
    }

    /// Check for common issues and anti-patterns
    fn check_common_issues(&self, project_path: &Path, results: &mut ValidationResults) -> Result<()> {
        // Check for .gitignore
        let gitignore = project_path.join(".gitignore");
        if !gitignore.exists() {
            results.add_issue(
                ValidationIssue::info("No .gitignore found".to_string())
                    .with_suggestion("Create .gitignore to exclude target/ and other build artifacts".to_string())
            );
        }

        // Check for README
        let readme_files = ["README.md", "README.txt", "README"];
        let has_readme = readme_files.iter().any(|&file| project_path.join(file).exists());
        
        if !has_readme {
            results.add_issue(
                ValidationIssue::info("No README file found".to_string())
                    .with_suggestion("Create README.md to document your processor".to_string())
            );
        }

        // Check for large target directory
        let target_dir = project_path.join("target");
        if target_dir.exists()
            && let Ok(metadata) = fs::metadata(&target_dir) {
                // This is a simple check - in practice you'd want to calculate directory size
                if metadata.is_dir() {
                    results.add_issue(
                        ValidationIssue::info("Target directory exists".to_string())
                            .with_suggestion("Consider running 'cargo clean' periodically to save disk space".to_string())
                    );
                }
            }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project(temp_dir: &TempDir) -> Result<()> {
        let project_path = temp_dir.path();
        
        // Create Cargo.toml
        let cargo_toml = r#"
[package]
name = "test-processor"
version = "0.1.0"
edition = "2021"

[dependencies]
sentio-sdk = "0.1.0"
"#;
        fs::write(project_path.join("Cargo.toml"), cargo_toml)?;

        // Create src directory and main.rs
        fs::create_dir_all(project_path.join("src"))?;
        fs::write(project_path.join("src/main.rs"), "fn main() {}")?;

        // Create sentio.yaml
        let sentio_config = r#"
name: test-processor
version: 0.1.0
target_network: ethereum
contracts: []
build:
  target: x86_64-unknown-linux-musl
  optimization_level: release
  features: []
"#;
        fs::write(project_path.join("sentio.yaml"), sentio_config)?;

        Ok(())
    }

    #[test]
    fn test_validate_valid_project() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(&temp_dir).unwrap();

        let validator = ProjectValidator::new();
        let result = validator.validate_project(temp_dir.path().to_str().unwrap());
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_missing_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        
        let validator = ProjectValidator::new();
        let results = validator.validate_project_detailed(temp_dir.path().to_str().unwrap()).unwrap();
        
        assert!(results.has_errors());
        assert!(results.issues.iter().any(|i| i.message.contains("Cargo.toml not found")));
    }

    #[test]
    fn test_validate_missing_src_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create only Cargo.toml
        let cargo_toml = r#"
[package]
name = "test"
version = "0.1.0"
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml).unwrap();

        let validator = ProjectValidator::new();
        let results = validator.validate_project_detailed(temp_dir.path().to_str().unwrap()).unwrap();
        
        assert!(results.has_errors());
        assert!(results.issues.iter().any(|i| i.message.contains("src directory not found")));
    }

    #[test]
    fn test_validation_issue_display() {
        let issue = ValidationIssue::error("Test error".to_string())
            .with_suggestion("Fix this".to_string())
            .with_file("test.rs".to_string());

        let display = format!("{}", issue);
        assert!(display.contains("ERROR"));
        assert!(display.contains("Test error"));
        assert!(display.contains("Fix this"));
        assert!(display.contains("test.rs"));
    }
}