use anyhow::Result;
use clap::{Parser, Subcommand};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::test_api;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ping { server } => {
            ping(&server).await?;
        }
    }

    Ok(())
}

async fn ping(server: &str) -> Result<()> {
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    let response = test_api::unauthed_ping(&config).await?;

    println!("{}", response.message);

    Ok(())
}
