use anyhow::Result;
use async_trait::async_trait;
use super::Command;

pub struct GenCommand {
    pub generate_handlers: bool,
    pub generate_contracts: bool,
    pub target_contract: Option<String>,
}

#[async_trait]
impl Command for GenCommand {
    async fn execute(&self) -> Result<()> {
        println!("Generating code...");
        // TODO: Implement gen logic
        Ok(())
    }
}