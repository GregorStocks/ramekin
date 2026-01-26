mod export;
mod generate_test_urls;
mod import;
mod load_test;
mod parse_html;
mod pipeline;
mod pipeline_orchestrator;
mod screenshot;
mod seed;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::testing_api;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// What to do when HTML fetch fails
#[derive(Clone, Copy, Default, ValueEnum)]
pub enum OnFetchFail {
    /// Mark as failed and continue to next URL (default)
    #[default]
    Continue,
    /// Skip the URL entirely (don't record as failure)
    Skip,
    /// Prompt user to manually save HTML from browser
    Prompt,
}

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
    /// Generate a list of test URLs from top recipe sites
    GenerateTestUrls {
        /// Output file path
        #[arg(short, long, default_value = "data/test-urls.json")]
        output: PathBuf,
        /// Number of sites to include
        #[arg(long, default_value = "50")]
        num_sites: usize,
        /// Number of URLs per site
        #[arg(long, default_value = "20")]
        urls_per_site: usize,
        /// Merge with existing file instead of replacing
        #[arg(long)]
        merge: bool,
    },
    /// Run the full pipeline for all test URLs
    PipelineTest {
        /// Path to test-urls.json
        #[arg(long, default_value = "data/test-urls.json")]
        test_urls: PathBuf,
        /// Output directory for runs
        #[arg(long, default_value = "data/pipeline-runs")]
        output_dir: PathBuf,
        /// Limit number of URLs to process
        #[arg(long)]
        limit: Option<usize>,
        /// Filter to URLs from a specific site (domain)
        #[arg(long)]
        site: Option<String>,
        /// Delay in milliseconds between URL fetches
        #[arg(long, default_value = "1000")]
        delay_ms: u64,
        /// Re-fetch all HTML even if cached
        #[arg(long)]
        force_fetch: bool,
        /// What to do when fetch fails: continue (default), skip, or prompt
        #[arg(long, value_enum, default_value = "continue")]
        on_fetch_fail: OnFetchFail,
    },
    /// Show HTML cache statistics
    PipelineCacheStats {
        /// Cache directory (defaults to ~/.ramekin/pipeline-cache/html)
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
    /// Clear HTML cache
    PipelineCacheClear {
        /// Cache directory (defaults to ~/.ramekin/pipeline-cache/html)
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
    /// Generate a summary report from the latest pipeline run
    PipelineSummary {
        /// Directory containing pipeline run results
        #[arg(long, default_value = "data/pipeline-runs")]
        runs_dir: PathBuf,
        /// Output file for the summary (default: print to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with debug level by default for CLI
    // Can be overridden with RUST_LOG environment variable
    // Filter out extremely verbose html5ever tokenizer debug logs
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug,html5ever=info,selectors=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();

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
        Commands::GenerateTestUrls {
            output,
            num_sites,
            urls_per_site,
            merge,
        } => {
            generate_test_urls::generate_test_urls(&output, num_sites, urls_per_site, merge)
                .await?;
        }
        Commands::PipelineTest {
            test_urls,
            output_dir,
            limit,
            site,
            delay_ms,
            force_fetch,
            on_fetch_fail,
        } => {
            let config = pipeline_orchestrator::OrchestratorConfig {
                test_urls_file: test_urls,
                output_dir,
                limit,
                site_filter: site,
                delay_ms,
                force_fetch,
                on_fetch_fail,
            };
            pipeline_orchestrator::run_pipeline_test(config).await?;
        }
        Commands::PipelineCacheStats { cache_dir } => {
            let cache_dir = cache_dir.unwrap_or_else(ramekin_core::http::DiskCache::default_dir);
            pipeline_orchestrator::print_cache_stats(&cache_dir);
        }
        Commands::PipelineCacheClear { cache_dir } => {
            let cache_dir = cache_dir.unwrap_or_else(ramekin_core::http::DiskCache::default_dir);
            pipeline_orchestrator::clear_cache(&cache_dir)?;
        }
        Commands::PipelineSummary { runs_dir, output } => {
            let (run_id, results) = pipeline_orchestrator::load_latest_results(&runs_dir)?;
            let report = pipeline_orchestrator::generate_summary_report(&results);

            if let Some(output_path) = output {
                std::fs::write(&output_path, &report)?;
                println!("Summary saved to: {}", output_path.display());
                println!("(from run: {})", run_id);
            } else {
                println!("Run: {}\n", run_id);
                print!("{}", report);
            }
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
