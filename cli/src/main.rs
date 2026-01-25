mod enrich_validate;
mod export;
mod generate_test_urls;
mod import;
mod load_test;
mod parse_html;
mod pipeline;
mod pipeline_orchestrator;
mod screenshot;
mod seed;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::{auth_api, testing_api};
use ramekin_client::models::LoginRequest;
use std::path::{Path, PathBuf};
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
    /// Run a single pipeline step for a URL
    PipelineStep {
        /// The step to run: fetch_html, extract_recipe, or save_recipe
        #[arg(long)]
        step: String,
        /// URL to process
        #[arg(long)]
        url: String,
        /// Run directory for artifacts
        #[arg(long, default_value = "data/pipeline-runs/adhoc")]
        run_dir: PathBuf,
        /// Re-fetch HTML even if cached
        #[arg(long)]
        force_fetch: bool,
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
    /// Validate enrichments on recipes from pipeline runs
    EnrichValidate {
        /// Server URL
        #[arg(long, env = "API_BASE_URL")]
        server_url: String,
        /// Username for authentication
        #[arg(long)]
        username: String,
        /// Password for authentication
        #[arg(long)]
        password: String,
        /// Directory containing pipeline run results
        #[arg(long, default_value = "data/pipeline-runs")]
        runs_dir: PathBuf,
        /// Output directory for enrichment test runs
        #[arg(long, default_value = "data/pipeline-runs")]
        output_dir: PathBuf,
        /// Limit number of recipes to process
        #[arg(long)]
        limit: Option<usize>,
        /// Run only a specific enrichment type
        #[arg(long, short = 't')]
        enrichment_type: Option<String>,
        /// Filter to recipes from a specific site (domain)
        #[arg(long)]
        site: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with debug level by default for CLI
    // Can be overridden with RUST_LOG environment variable
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));
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
        Commands::PipelineStep {
            step,
            url,
            run_dir,
            force_fetch,
        } => {
            run_pipeline_step(&step, &url, &run_dir, force_fetch).await?;
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
        Commands::EnrichValidate {
            server_url,
            username,
            password,
            runs_dir,
            output_dir,
            limit,
            enrichment_type,
            site,
        } => {
            // Login to get auth token
            let auth_token = login(&server_url, &username, &password).await?;

            let config = enrich_validate::EnrichValidateConfig {
                server_url,
                auth_token,
                runs_dir,
                output_dir,
                limit,
                enrichment_type,
                site_filter: site,
            };
            enrich_validate::run_enrich_validate(config).await?;
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

async fn run_pipeline_step(step: &str, url: &str, run_dir: &Path, force_fetch: bool) -> Result<()> {
    use pipeline::PipelineStep;
    use ramekin_core::CachingClient;

    let step = PipelineStep::from_str(step)?;
    let client = CachingClient::new()?;

    // Create run directory
    std::fs::create_dir_all(run_dir)?;

    let result = match step {
        PipelineStep::FetchHtml => pipeline::run_fetch_html(url, &client, force_fetch).await,
        PipelineStep::ExtractRecipe => {
            // Ensure HTML is fetched first
            if !client.is_cached(url) && !force_fetch {
                tracing::debug!("HTML not cached, fetching first...");
                let fetch_result = pipeline::run_fetch_html(url, &client, false).await;
                if !fetch_result.success {
                    tracing::warn!(error = ?fetch_result.error, "Fetch failed");
                    return Ok(());
                }
            }
            pipeline::run_extract_recipe(url, &client, run_dir).step_result
        }
        PipelineStep::SaveRecipe => {
            // Ensure previous steps are done
            if !client.is_cached(url) {
                tracing::debug!("HTML not cached, fetching first...");
                let fetch_result = pipeline::run_fetch_html(url, &client, false).await;
                if !fetch_result.success {
                    tracing::warn!(error = ?fetch_result.error, "Fetch failed");
                    return Ok(());
                }
            }
            let extract_result = pipeline::run_extract_recipe(url, &client, run_dir);
            if !extract_result.step_result.success {
                tracing::warn!(error = ?extract_result.step_result.error, "Extract failed");
                return Ok(());
            }
            pipeline::run_save_recipe(url, run_dir)
        }
    };

    if result.success {
        println!(
            "Step {} succeeded in {}ms",
            step.as_str(),
            result.duration_ms
        );
        if result.cached {
            println!("(used cached HTML)");
        }
    } else {
        println!("Step {} failed: {:?}", step.as_str(), result.error);
    }

    Ok(())
}

async fn login(server_url: &str, username: &str, password: &str) -> Result<String> {
    let mut config = Configuration::new();
    config.base_path = server_url.to_string();

    let response = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .context("Failed to login")?;

    Ok(response.token)
}
