use clap::{Parser, Subcommand};
use anyhow::Result;

mod commands;
mod utils;

use commands::*;

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
        /// Skip project validation before building
        #[arg(long)]
        no_validate: bool,
        /// Target architecture for cross-compilation
        #[arg(long)]
        target: Option<String>,
        /// Optimization level (debug or release)
        #[arg(long)]
        optimization: Option<String>,
        /// Features to enable
        #[arg(long)]
        features: Vec<String>,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Initialize a new Sentio processor project
    Init {
        /// Name of the new project
        name: String,
    },
    /// Generate code for handlers and contract bindings
    Gen {
        /// Skip generating handlers
        #[arg(long)]
        no_handlers: bool,
        /// Skip generating contract bindings
        #[arg(long)]
        no_contracts: bool,
        /// Generate code only for specific contract
        #[arg(long)]
        contract: Option<String>,
    },
    /// Upload compiled binary to Sentio platform
    Upload {
        /// Path to binary file
        #[arg(long)]
        binary: Option<String>,
    },
    /// Manage authentication with Sentio platform
    Auth {
        #[command(subcommand)]
        action: AuthActions,
    },
    /// Manage contracts in the project
    Contract {
        #[command(subcommand)]
        action: ContractActions,
    },
    /// Run tests for the processor project
    Test {
        /// Filter tests by pattern
        #[arg(long)]
        filter: Option<String>,
        /// Run tests in release mode
        #[arg(long)]
        release: bool,
    },
}

#[derive(Subcommand)]
enum AuthActions {
    /// Login to Sentio platform
    Login,
    /// Logout from Sentio platform
    Logout,
    /// Check authentication status
    Status,
}

#[derive(Subcommand)]
enum ContractActions {
    /// Add a contract to the project
    Add {
        /// Contract address
        address: String,
        /// Custom name for the contract
        #[arg(long)]
        name: Option<String>,
        /// Network for the contract
        #[arg(long)]
        network: Option<String>,
    },
    /// Remove a contract from the project
    Remove {
        /// Contract address
        address: String,
    },
    /// List all contracts in the project
    List,
}

/// Parse command line arguments, handling both direct invocation and cargo subcommand
fn parse_args() -> Cli {
    let mut args: Vec<String> = std::env::args().collect();
    
    // If invoked as `cargo sentio`, remove the "sentio" argument
    if args.len() > 1 && args[1] == "sentio" {
        args.remove(1);
    }
    
    Cli::parse_from(args)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_args();

    match cli.command {
        Commands::Build { path, no_validate, target, optimization, features, verbose } => {
            let command = build::BuildCommand {
                path,
                skip_validation: no_validate,
                target,
                optimization_level: optimization,
                features,
                verbose,
            };
            command.execute().await?;
        }
        Commands::Init { name } => {
            let command = init::InitCommand { name };
            command.execute().await?;
        }
        Commands::Gen { no_handlers, no_contracts, contract } => {
            let command = gen::GenCommand {
                generate_handlers: !no_handlers,
                generate_contracts: !no_contracts,
                target_contract: contract,
            };
            command.execute().await?;
        }
        Commands::Upload { binary } => {
            let command = upload::UploadCommand { binary_path: binary };
            command.execute().await?;
        }
        Commands::Auth { action } => {
            let auth_action = match action {
                AuthActions::Login => auth::AuthAction::Login,
                AuthActions::Logout => auth::AuthAction::Logout,
                AuthActions::Status => auth::AuthAction::Status,
            };
            let command = auth::AuthCommand { action: auth_action };
            command.execute().await?;
        }
        Commands::Contract { action } => {
            let contract_action = match action {
                ContractActions::Add { address, name, network } => {
                    contract::ContractAction::Add { address, name, network }
                }
                ContractActions::Remove { address } => {
                    contract::ContractAction::Remove { address }
                }
                ContractActions::List => contract::ContractAction::List,
            };
            let command = contract::ContractCommand { action: contract_action };
            command.execute().await?;
        }
        Commands::Test { filter, release } => {
            let command = test::TestCommand {
                filter,
                release_mode: release,
            };
            command.execute().await?;
        }
    }

    Ok(())
}