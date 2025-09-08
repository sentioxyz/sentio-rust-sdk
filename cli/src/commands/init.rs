use anyhow::{Result, Context, bail};
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use super::Command;

pub struct InitCommand {
    pub name: String,
    pub template: String,
}

#[async_trait]
impl Command for InitCommand {
    async fn execute(&self) -> Result<()> {
        println!("🚀 Initializing new Sentio processor: {}", self.name);
        
        // Validate project name
        if self.name.is_empty() {
            bail!("Project name cannot be empty");
        }
        
        // Check if directory already exists
        if Path::new(&self.name).exists() {
            bail!("Directory '{}' already exists", self.name);
        }
        
        // Get template path
        let template_path = self.get_template_path()?;
        
        // Validate template exists
        if !template_path.exists() {
            bail!("Template '{}' not found", self.template);
        }
        
        println!("📋 Using template: {}", self.template);
        println!("📁 Creating project directory: {}", self.name);
        
        // Create target directory
        fs::create_dir_all(&self.name)
            .with_context(|| format!("Failed to create directory '{}'", self.name))?;
        
        // Copy template files
        self.copy_template_files(&template_path, Path::new(&self.name))?;
        
        // Process template variables
        self.process_template_variables(Path::new(&self.name))?;
        
        println!("✅ Successfully initialized project '{}'", self.name);
        println!("📝 Next steps:");
        println!("   1. cd {}", self.name);
        println!("   2. Update contract address and chain ID in src/processor.rs");
        println!("   3. Define your entities in schema.graphql");
        println!("   4. Implement event handlers in src/processor.rs");
        println!("   5. Run 'sentio build' to compile your processor");
        
        Ok(())
    }
}

impl InitCommand {
    fn get_template_path(&self) -> Result<PathBuf> {
        // Try to find templates directory relative to the executable
        let exe_path = std::env::current_exe()
            .context("Failed to get executable path")?;
        
        // Look for templates in the same directory as the CLI binary
        let templates_dir = exe_path
            .parent()
            .unwrap()
            .join("templates");
            
        if templates_dir.exists() {
            return Ok(templates_dir.join(&self.template));
        }
        
        // Fallback: look relative to current directory (for development)
        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")?;
            
        let dev_templates = current_dir
            .join("cli")
            .join("templates");
            
        if dev_templates.exists() {
            return Ok(dev_templates.join(&self.template));
        }
        
        // Another fallback: look in the parent directory structure
        let mut current = current_dir.as_path();
        for _ in 0..5 {  // Look up to 5 levels up
            let templates_path = current.join("cli").join("templates");
            if templates_path.exists() {
                return Ok(templates_path.join(&self.template));
            }
            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }
        
        bail!("Templates directory not found");
    }
    
    fn copy_template_files(&self, from: &Path, to: &Path) -> Result<()> {
        println!("📂 Copying template files...");
        
        if from.is_dir() {
            for entry in fs::read_dir(from)
                .with_context(|| format!("Failed to read template directory '{}'", from.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                let file_name = entry.file_name();
                
                let dest = to.join(&file_name);
                
                if path.is_dir() {
                    fs::create_dir_all(&dest)?;
                    self.copy_template_files(&path, &dest)?;
                } else {
                    fs::copy(&path, &dest)
                        .with_context(|| format!("Failed to copy '{}' to '{}'", path.display(), dest.display()))?;
                }
            }
        } else {
            fs::copy(from, to)?;
        }
        
        Ok(())
    }
    
    fn process_template_variables(&self, project_dir: &Path) -> Result<()> {
        println!("🔄 Processing template variables...");
        
        let project_name_snake = self.name.replace('-', "_");
        let project_class_name = self.snake_to_pascal_case(&project_name_snake);
        
        // Define replacements
        let replacements = vec![
            ("{{PROJECT_NAME}}", self.name.as_str()),
            ("{{PROJECT_NAME_SNAKE}}", project_name_snake.as_str()),
            ("{{PROJECT_CLASS_NAME}}", project_class_name.as_str()),
        ];
        
        self.process_directory_files(project_dir, &replacements)?;
        
        Ok(())
    }
    
    fn process_directory_files(&self, dir: &Path, replacements: &[(&str, &str)]) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                self.process_directory_files(&path, replacements)?;
            } else if let Some(extension) = path.extension() {
                // Process text files
                if matches!(extension.to_str(), Some("rs") | Some("toml") | Some("md") | Some("graphql")) {
                    self.process_file(&path, replacements)?;
                }
            }
        }
        Ok(())
    }
    
    fn process_file(&self, file_path: &Path, replacements: &[(&str, &str)]) -> Result<()> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file '{}'", file_path.display()))?;
        
        let mut processed_content = content;
        for (placeholder, replacement) in replacements {
            processed_content = processed_content.replace(placeholder, replacement);
        }
        
        fs::write(file_path, processed_content)
            .with_context(|| format!("Failed to write file '{}'", file_path.display()))?;
        
        Ok(())
    }
    
    fn snake_to_pascal_case(&self, snake_str: &str) -> String {
        snake_str
            .split('_')
            .map(|word| {
                let mut chars: Vec<char> = word.chars().collect();
                if !chars.is_empty() {
                    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
                }
                chars.into_iter().collect::<String>()
            })
            .collect()
    }
}