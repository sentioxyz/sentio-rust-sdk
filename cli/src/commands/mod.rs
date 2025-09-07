pub mod build;
pub mod init;
pub mod generate;
pub mod upload;
pub mod auth;
pub mod contract;
pub mod test;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Command {
    async fn execute(&self) -> Result<()>;
}