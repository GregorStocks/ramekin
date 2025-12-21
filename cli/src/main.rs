mod import;
mod seed;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::testing_api;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ramekin")]
#[command(about = "Ramekin CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ping the server (unauthenticated)
    Ping {
        /// Server URL (default: http://localhost:3000)
        #[arg(long, default_value = "http://localhost:3000")]
        server: String,
    },
    /// Seed the database with a user and import recipes from file
    Seed {
        /// Server URL (default: http://localhost:3000)
        #[arg(long, default_value = "http://localhost:3000")]
        server: String,
        /// Username for the seed user
        #[arg(long)]
        username: String,
        /// Password for the seed user
        #[arg(long)]
        password: String,
        /// Path to the .paprikarecipes file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Import recipes from a Paprika .paprikarecipes file
    Import {
        /// Server URL (default: http://localhost:3000)
        #[arg(long, default_value = "http://localhost:3000")]
        server: String,
        /// Username to authenticate as
        #[arg(long)]
        username: String,
        /// Password for authentication
        #[arg(long)]
        password: String,
        /// Path to the .paprikarecipes file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ping { server } => {
            ping(&server).await?;
        }
        Commands::Seed {
            server,
            username,
            password,
            file,
        } => {
            seed::seed(&server, &username, &password, &file).await?;
        }
        Commands::Import {
            server,
            username,
            password,
            file,
        } => {
            import::import(&server, &username, &password, &file).await?;
        }
    }

    Ok(())
}

async fn ping(server: &str) -> Result<()> {
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    let response = testing_api::unauthed_ping(&config).await?;

    println!("{}", response.message);

    Ok(())
}
