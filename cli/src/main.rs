use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "sentio")]
#[command(about = "A CLI tool for compiling Sentio processors in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a Sentio processor
    Build {
        /// Path to the processor project
        #[arg(short, long, default_value = ".")]
        path: String,
    },
    /// Initialize a new Sentio processor project
    Init {
        /// Name of the new project
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { path } => {
            println!("Building Sentio processor at: {}", path);
            // TODO: Implement build logic
        }
        Commands::Init { name } => {
            println!("Initializing new Sentio processor: {}", name);
            // TODO: Implement init logic
        }
    }

    Ok(())
}