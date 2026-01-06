mod import;
mod load_test;
mod screenshot;
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
        /// Server URL (or set PORT env var)
        #[arg(long, env = "API_BASE_URL")]
        server: Option<String>,
    },
    /// Seed the database with a user and import recipes from file
    Seed {
        /// Server URL (or set PORT env var)
        #[arg(long, env = "API_BASE_URL")]
        server: Option<String>,
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
        /// Server URL (or set PORT env var)
        #[arg(long, env = "API_BASE_URL")]
        server: Option<String>,
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
    /// Run a load test creating many users with recipes and photos
    LoadTest {
        /// Server URL (or set PORT env var)
        #[arg(long, env = "API_BASE_URL")]
        server: Option<String>,
        /// UI URL for browser tests (or set UI_PORT env var)
        #[arg(long, env = "UI_BASE_URL")]
        ui_url: Option<String>,
        /// Number of users to create (default: 10)
        #[arg(long, default_value = "10")]
        users: usize,
        /// Number of concurrent workers (default: 5)
        #[arg(long, default_value = "5")]
        concurrency: usize,
    },
    /// Take screenshots of the app as the test user
    Screenshot {
        /// UI URL (or set UI_PORT env var)
        #[arg(long, env = "UI_BASE_URL")]
        ui_url: Option<String>,
        /// Username for authentication
        #[arg(long, default_value = "t")]
        username: String,
        /// Password for authentication
        #[arg(long, default_value = "t")]
        password: String,
        /// Output directory for screenshots (default: logs)
        #[arg(long, default_value = "logs")]
        output_dir: PathBuf,
        /// Viewport width (default: 1280)
        #[arg(long, default_value = "1280")]
        width: u32,
        /// Viewport height (default: 900)
        #[arg(long, default_value = "900")]
        height: u32,
    },
}

fn require_server_url(server: Option<String>) -> Result<String> {
    server
        .or_else(|| {
            std::env::var("PORT")
                .ok()
                .map(|p| format!("http://localhost:{}", p))
        })
        .ok_or_else(|| anyhow::anyhow!("Server URL required: use --server or set PORT env var"))
}

fn require_ui_url(ui_url: Option<String>) -> Result<String> {
    ui_url
        .or_else(|| {
            std::env::var("UI_PORT")
                .ok()
                .map(|p| format!("http://localhost:{}", p))
        })
        .ok_or_else(|| anyhow::anyhow!("UI URL required: use --ui-url or set UI_PORT env var"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ping { server } => {
            let server = require_server_url(server)?;
            ping(&server).await?;
        }
        Commands::Seed {
            server,
            username,
            password,
            file,
        } => {
            let server = require_server_url(server)?;
            seed::seed(&server, &username, &password, &file).await?;
        }
        Commands::Import {
            server,
            username,
            password,
            file,
        } => {
            let server = require_server_url(server)?;
            import::import(&server, &username, &password, &file).await?;
        }
        Commands::LoadTest {
            server,
            ui_url,
            users,
            concurrency,
        } => {
            let server = require_server_url(server)?;
            let ui_url = require_ui_url(ui_url)?;
            load_test::load_test(&server, &ui_url, users, concurrency).await?;
        }
        Commands::Screenshot {
            ui_url,
            username,
            password,
            output_dir,
            width,
            height,
        } => {
            let ui_url = require_ui_url(ui_url)?;
            screenshot::screenshot(&ui_url, &username, &password, &output_dir, width, height)?;
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
