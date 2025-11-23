use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "ramekin")]
#[command(about = "Ramekin CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all garbages
    Garbages {
        /// Server URL (default: http://localhost:3000)
        #[arg(long, default_value = "http://localhost:3000")]
        server: String,
    },
}

// Temporary types - will be replaced with generated client
#[derive(Debug, Deserialize)]
struct GarbagesResponse {
    garbages: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Garbages { server } => {
            list_garbages(&server).await?;
        }
    }

    Ok(())
}

async fn list_garbages(server: &str) -> Result<()> {
    let url = format!("{}/api/garbages", server);
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch garbages: {}", response.status());
    }

    let garbages_response: GarbagesResponse = response.json().await?;

    println!("Garbages:");
    for garbage in garbages_response.garbages {
        println!("  - {}", garbage);
    }

    Ok(())
}
