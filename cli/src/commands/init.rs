use anyhow::Result;
use async_trait::async_trait;
use super::Command;

pub struct InitCommand {
    pub name: String,
}

#[async_trait]
impl Command for InitCommand {
    async fn execute(&self) -> Result<()> {
        println!("Initializing new Sentio processor: {}", self.name);
        // TODO: Implement init logic
        Ok(())
    }
}