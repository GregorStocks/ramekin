mod export;
mod generate_test_urls;
mod import;
mod ingredient_tests;
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
        /// Path to a JSON file with tags to create before importing
        #[arg(long)]
        tags_file: Option<PathBuf>,
        /// Whether to preserve categories/tags from the input file (default: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        preserve_tags: bool,
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
        /// Whether to preserve categories/tags from the input file (default: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        preserve_tags: bool,
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
        #[arg(long, default_value = "100")]
        num_sites: usize,
        /// Number of URLs per site
        #[arg(long, default_value = "100")]
        urls_per_site: usize,
        /// Merge with existing file instead of replacing
        #[arg(long)]
        merge: bool,
        /// Filter to a specific site domain (e.g., "smittenkitchen.com")
        #[arg(long)]
        site: Option<String>,
        /// Minimum year for dated blog posts (default: 2016)
        #[arg(long, default_value = "2016")]
        min_year: u32,
        /// Remove URL count limits (process all URLs from sitemaps)
        #[arg(long)]
        no_limit: bool,
        /// Refilter existing URLs through current filter logic (no network requests)
        #[arg(long)]
        refilter: bool,
    },
    /// Run the full pipeline for all test URLs and generate reports
    Pipeline {
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
        /// Run in offline mode (cache only, no network requests)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        offline: bool,
        /// Force re-fetch all URLs, ignoring cache
        #[arg(long)]
        force_refetch: bool,
        /// What to do when fetch fails: continue (default), skip, or prompt
        #[arg(long, value_enum, default_value = "continue")]
        on_fetch_fail: OnFetchFail,
        /// Path to tags JSON file for auto-tag evaluation
        #[arg(long, default_value = "data/eval-tags.json")]
        tags_file: PathBuf,
        /// Number of URLs to process concurrently
        #[arg(long, default_value = "10")]
        concurrency: usize,
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
    /// Generate ingredient parsing test fixtures from pipeline run
    IngredientTestsGenerate {
        /// Directory containing pipeline run results
        #[arg(long, default_value = "data/pipeline-runs")]
        runs_dir: PathBuf,
        /// Fixtures directory (default: ramekin-core/tests/fixtures/ingredient_parsing)
        #[arg(long)]
        fixtures_dir: Option<PathBuf>,
    },
    /// Update ingredient parsing test fixtures to match current parser output
    IngredientTestsUpdate {
        /// Fixtures directory (default: ramekin-core/tests/fixtures/ingredient_parsing)
        #[arg(long)]
        fixtures_dir: Option<PathBuf>,
    },
    /// Generate ingredient parsing test fixtures from a .paprikarecipes file
    IngredientTestsGeneratePaprika {
        /// Path to the .paprikarecipes file
        #[arg(value_name = "FILE", default_value = "data/dev/seed.paprikarecipes")]
        file: PathBuf,
        /// Fixtures directory (default: ramekin-core/tests/fixtures/ingredient_parsing)
        #[arg(long)]
        fixtures_dir: Option<PathBuf>,
    },
    /// Migrate curated fixtures from individual files to category files
    IngredientTestsMigrateCurated {
        /// Fixtures directory (default: ramekin-core/tests/fixtures/ingredient_parsing)
        #[arg(long)]
        fixtures_dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with info level by default for CLI
    // Can be overridden with RUST_LOG environment variable (e.g., RUST_LOG=debug)
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

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
            tags_file,
            preserve_tags,
            file,
        } => {
            seed::seed(
                &server_url,
                &username,
                &password,
                tags_file.as_deref(),
                preserve_tags,
                &file,
            )
            .await?;
        }
        Commands::Import {
            server_url,
            username,
            password,
            preserve_tags,
            file,
        } => {
            import::import(&server_url, &username, &password, preserve_tags, &file).await?;
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
            site,
            min_year,
            no_limit,
            refilter,
        } => {
            if refilter {
                generate_test_urls::refilter_test_urls(&output, min_year)?;
            } else {
                generate_test_urls::generate_test_urls(
                    &output,
                    num_sites,
                    urls_per_site,
                    merge,
                    site.as_deref(),
                    min_year,
                    no_limit,
                )
                .await?;
            }
        }
        Commands::Pipeline {
            test_urls,
            output_dir,
            limit,
            site,
            delay_ms,
            offline,
            force_refetch,
            on_fetch_fail,
            tags_file,
            concurrency,
        } => {
            let config = pipeline_orchestrator::OrchestratorConfig {
                test_urls_file: test_urls,
                output_dir: output_dir.clone(),
                limit,
                site_filter: site,
                delay_ms,
                offline,
                force_refetch,
                on_fetch_fail,
                tags_file,
                concurrency,
            };
            let results = pipeline_orchestrator::run_pipeline_test(config).await?;

            // Generate and save extraction report
            let extraction_report = pipeline_orchestrator::generate_summary_report(&results);
            let extraction_report_path = PathBuf::from("data/extraction-report.md");
            std::fs::write(&extraction_report_path, &extraction_report)?;
            println!(
                "Extraction report saved to: {}",
                extraction_report_path.display()
            );

            // Generate and save tag report
            let (run_id, run_dir) = pipeline_orchestrator::get_latest_run_dir(&output_dir)?;
            let tag_report = pipeline_orchestrator::generate_tag_report(&run_dir)?;
            let tag_report_path = PathBuf::from("data/tag-report.md");
            std::fs::write(&tag_report_path, &tag_report)?;
            println!(
                "Tag report saved to: {} (from run: {})",
                tag_report_path.display(),
                run_id
            );

            // Generate and save density gap report
            let density_report = pipeline_orchestrator::generate_density_gap_report(&results);
            let density_report_path = PathBuf::from("data/density-gap-report.txt");
            std::fs::write(&density_report_path, &density_report)?;
            println!(
                "Density gap report saved to: {}",
                density_report_path.display()
            );

            // Generate and save unique ingredients file
            let unique_ingredients =
                pipeline_orchestrator::generate_unique_ingredients_file(&run_dir)?;
            let unique_ingredients_path = PathBuf::from("data/unique-ingredients.txt");
            std::fs::write(&unique_ingredients_path, &unique_ingredients)?;
            println!(
                "Unique ingredients saved to: {} ({} ingredients)",
                unique_ingredients_path.display(),
                unique_ingredients.lines().count()
            );
        }
        Commands::PipelineCacheStats { cache_dir } => {
            let cache_dir = cache_dir.unwrap_or_else(ramekin_core::http::DiskCache::default_dir);
            pipeline_orchestrator::print_cache_stats(&cache_dir);
        }
        Commands::PipelineCacheClear { cache_dir } => {
            let cache_dir = cache_dir.unwrap_or_else(ramekin_core::http::DiskCache::default_dir);
            pipeline_orchestrator::clear_cache(&cache_dir)?;
        }
        Commands::IngredientTestsGenerate {
            runs_dir,
            fixtures_dir,
        } => {
            ingredient_tests::generate_from_pipeline(&runs_dir, fixtures_dir.as_deref())?;
        }
        Commands::IngredientTestsUpdate { fixtures_dir } => {
            ingredient_tests::update_fixtures(fixtures_dir.as_deref())?;
        }
        Commands::IngredientTestsGeneratePaprika { file, fixtures_dir } => {
            ingredient_tests::generate_from_paprika(&file, fixtures_dir.as_deref())?;
        }
        Commands::IngredientTestsMigrateCurated { fixtures_dir } => {
            ingredient_tests::migrate_curated(fixtures_dir.as_deref())?;
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
