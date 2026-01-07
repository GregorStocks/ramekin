mod export;
mod import;
mod load_test;
mod parse_html;
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
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
    },
    /// Seed the database with a user and import recipes from file
    Seed {
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
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
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
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
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
        /// UI URL for browser tests
        #[arg(long, env = "UI_BASE_URL")]
        ui_url: String,
        /// Number of users to create (default: 10)
        #[arg(long, default_value = "10")]
        users: usize,
        /// Number of concurrent workers (default: 5)
        #[arg(long, default_value = "5")]
        concurrency: usize,
    },
    /// Take screenshots of the app as the test user
    Screenshot {
        /// UI URL
        #[arg(long, env = "UI_BASE_URL")]
        ui_url: String,
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
    /// Parse a recipe from an HTML file (offline, no server required)
    ParseHtml {
        /// Path to the HTML file
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// Source URL for the recipe (used for source_name extraction)
        #[arg(long)]
        source_url: String,
    },
    /// Export a single recipe to a Paprika .paprikarecipe file
    ExportRecipe {
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
        /// Username to authenticate as
        #[arg(long)]
        username: String,
        /// Password for authentication
        #[arg(long)]
        password: String,
        /// Recipe ID to export
        #[arg(long)]
        id: String,
        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
    /// Export all recipes to a Paprika .paprikarecipes archive
    ExportAll {
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
        /// Username to authenticate as
        #[arg(long)]
        username: String,
        /// Password for authentication
        #[arg(long)]
        password: String,
        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ping { server_url } => {
            ping(&server_url).await?;
        }
        Commands::Seed {
            server_url,
            username,
            password,
            file,
        } => {
            seed::seed(&server_url, &username, &password, &file).await?;
        }
        Commands::Import {
            server_url,
            username,
            password,
            file,
        } => {
            import::import(&server_url, &username, &password, &file).await?;
        }
        Commands::LoadTest {
            server_url,
            ui_url,
            users,
            concurrency,
        } => {
            load_test::load_test(&server_url, &ui_url, users, concurrency).await?;
        }
        Commands::Screenshot {
            ui_url,
            username,
            password,
            output_dir,
            width,
            height,
        } => {
            screenshot::screenshot(&ui_url, &username, &password, &output_dir, width, height)?;
        }
        Commands::ParseHtml { file, source_url } => {
            parse_html::parse_html(&file, &source_url)?;
        }
        Commands::ExportRecipe {
            server_url,
            username,
            password,
            id,
            output,
        } => {
            export::export_recipe(&server_url, &username, &password, &id, &output).await?;
        }
        Commands::ExportAll {
            server_url,
            username,
            password,
            output,
        } => {
            export::export_all(&server_url, &username, &password, &output).await?;
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
