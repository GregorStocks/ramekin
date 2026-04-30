use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::generate_test_urls::TestUrlsOutput;
use crate::pipeline::{
    build_registry, clear_staging, ensure_staging_dir, find_staged_html, run_all_steps,
    staging_dir, AllStepsResult, ExtractionStats, IngredientStats, PipelineStep, StepResult,
};
use crate::OnFetchFail;
use ramekin_core::http::{CachingClient, DiskCache};
use ramekin_core::pipeline::StepRegistry;

// ============================================================================
// Configuration
// ============================================================================

pub struct OrchestratorConfig {
    /// Identifier for this run. Caller generates this before setting up logging
    /// so the log file and run directory share the same stem.
    pub run_id: String,
    pub test_urls_file: PathBuf,
    pub output_dir: PathBuf,
    pub delay_ms: u64,
    pub force_refetch: bool,
    pub on_fetch_fail: OnFetchFail,
    pub tags_file: PathBuf,
    pub concurrency: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            run_id: Utc::now().format("%Y-%m-%d_%H-%M-%S%.3f").to_string(),
            test_urls_file: PathBuf::from("data/test-urls.json"),
            output_dir: PathBuf::from("data/pipeline-runs"),
            delay_ms: 1000,
            force_refetch: false,
            on_fetch_fail: OnFetchFail::Continue,
            tags_file: PathBuf::from("data/eval-tags.json"),
            concurrency: 10,
        }
    }
}

/// Tags file format for evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsFile {
    pub tags: Vec<String>,
}

/// Load tags from a JSON file.
pub fn load_tags_file(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read tags file: {}", path.display()))?;
    let tags_file: TagsFile =
        serde_json::from_str(&content).with_context(|| "Failed to parse tags file as JSON")?;
    Ok(tags_file.tags)
}

// ============================================================================
// Run manifest
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub run_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub config: ManifestConfig,
    pub status: RunStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestConfig {
    pub test_urls_file: String,
    pub delay_ms: u64,
    pub force_refetch: bool,
    pub concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

// ============================================================================
// Results
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineResults {
    pub total_urls: usize,
    pub completed: usize,
    pub failed_at_fetch: usize,
    pub failed_at_extract: usize,
    pub failed_at_save: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub ai_cache_hits: usize,
    pub ai_cache_misses: usize,
    pub by_site: HashMap<String, SiteResults>,
    pub url_results: Vec<UrlResult>,
    pub extraction_method_stats: ExtractionMethodStats,
    pub ingredient_stats: IngredientParsingStats,
}

/// Aggregated stats about ingredient parsing across all URLs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngredientParsingStats {
    /// Total ingredients parsed across all recipes
    pub total_ingredients: usize,
    /// Volume-to-weight conversions successful
    pub volume_converted: usize,
    /// Volume-to-weight conversions failed (unknown ingredient)
    pub volume_unknown_ingredient: usize,
    /// Volume-to-weight conversions skipped (no volume unit)
    pub volume_no_volume: usize,
    /// Volume-to-weight conversions skipped (already has weight)
    pub volume_already_has_weight: usize,
    /// Metric conversions from oz
    pub metric_converted_oz: usize,
    /// Metric conversions from lb
    pub metric_converted_lb: usize,
    /// Names of ingredients that had volume measurements but no density data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_ingredients: Vec<String>,
}

/// Stats about which extraction methods work across all URLs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionMethodStats {
    /// URLs where HTML was fetched successfully (denominator for extraction stats)
    pub urls_with_html: usize,
    /// URLs where JSON-LD extraction succeeded
    pub jsonld_success: usize,
    /// URLs where microdata extraction succeeded
    pub microdata_success: usize,
    /// URLs where both methods succeeded
    pub both_success: usize,
    /// URLs where neither method succeeded
    pub neither_success: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteResults {
    pub domain: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlResult {
    pub url: String,
    pub site: String,
    pub steps: Vec<StepResult>,
    pub final_status: FinalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_stats: Option<ExtractionStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalStatus {
    Completed,
    FailedAtFetch,
    FailedAtExtract,
    FailedAtSave,
}

// ============================================================================
// Main orchestrator
// ============================================================================

pub async fn run_pipeline_test(config: OrchestratorConfig) -> Result<PipelineResults> {
    let run_id = config.run_id.clone();
    let now = Utc::now();
    let run_dir = config.output_dir.join(&run_id);

    // Create run directory
    fs::create_dir_all(&run_dir)?;

    // Load test URLs plus any snapshot-allowlisted URLs that must also run.
    let load_urls_start = Instant::now();
    let test_urls = load_pipeline_urls(&config.test_urls_file)?;

    // Collect URLs to process
    let mut urls_to_process: Vec<(String, String)> = Vec::new(); // (url, domain)

    for site in &test_urls.sites {
        for url in &site.urls {
            urls_to_process.push((url.clone(), site.domain.clone()));
        }
    }
    tracing::info!(
        phase = "setup.load_urls",
        elapsed_ms = load_urls_start.elapsed().as_millis() as u64,
        urls = urls_to_process.len(),
        sites = test_urls.sites.len(),
        "loaded test URLs"
    );

    // Load tags for auto-tag evaluation
    let load_tags_start = Instant::now();
    let user_tags = load_tags_file(&config.tags_file)?;
    tracing::info!(
        phase = "setup.load_tags",
        elapsed_ms = load_tags_start.elapsed().as_millis() as u64,
        tags = user_tags.len(),
        "loaded tag allowlist"
    );
    tracing::info!(
        "Loaded {} tags from {}",
        user_tags.len(),
        config.tags_file.display()
    );

    // Create manifest
    let manifest = RunManifest {
        run_id: run_id.clone(),
        started_at: now.to_rfc3339(),
        completed_at: None,
        config: ManifestConfig {
            test_urls_file: config.test_urls_file.display().to_string(),
            delay_ms: config.delay_ms,
            force_refetch: config.force_refetch,
            concurrency: config.concurrency,
        },
        status: RunStatus::Running,
    };
    save_manifest(&run_dir, &manifest)?;

    // Initialize HTTP client with caching
    // The CachingClient uses RAMEKIN_HTTP_CACHE env var for cache directory
    // and handles rate limiting internally.
    let client = Arc::new(
        CachingClient::builder()
            .rate_limit_ms(0) // We handle delay ourselves between URLs
            .build()
            .context("Failed to create HTTP client")?,
    );

    let total_urls = urls_to_process.len();
    let start_time = Instant::now();
    let registry_build_start = Instant::now();
    let registry = Arc::new(build_registry(Arc::clone(&client), user_tags));
    tracing::info!(
        phase = "setup.build_registry",
        elapsed_ms = registry_build_start.elapsed().as_millis() as u64,
        "built step registry"
    );

    tracing::info!("Pipeline Test Starting");
    tracing::info!("======================");
    tracing::info!("Run ID: {}", run_id);
    tracing::info!("URLs to process: {}", total_urls);
    tracing::info!("");

    // In prompt mode, ensure staging directory exists and is empty
    // Also force concurrency=1 since interactive prompts don't work well with parallelism
    let concurrency = if matches!(config.on_fetch_fail, OnFetchFail::Prompt) {
        ensure_staging_dir()?;
        clear_staging()?;
        tracing::info!(
            "Interactive mode: save HTML files to {}",
            staging_dir().display()
        );
        tracing::info!("(concurrency forced to 1 for interactive mode)");
        tracing::info!("");
        1
    } else {
        tracing::info!("Concurrency: {}", config.concurrency);
        tracing::info!("");
        config.concurrency
    };

    // Shuffle URLs to interleave domains for better parallelism
    // (avoids all concurrent slots hitting the same domain)
    urls_to_process.shuffle(&mut rand::rng());

    // Shared state for concurrent processing
    let results = Arc::new(Mutex::new(PipelineResults {
        total_urls,
        ..Default::default()
    }));
    let completed_count = Arc::new(AtomicUsize::new(0));
    let run_dir = Arc::new(run_dir);

    // Process URLs concurrently
    let processing_start = Instant::now();
    // Sum of time spent holding the results mutex while serializing and writing
    // intermediate results.json to disk. With 5k+ URLs this can dominate wall
    // clock even when individual saves are small, because it serializes the
    // workers.
    let intermediate_save_total_ms = Arc::new(AtomicUsize::new(0));
    let url_results: Vec<Option<UrlResult>> = stream::iter(urls_to_process.into_iter())
        .map(|(url, domain)| {
            let client = Arc::clone(&client);
            let registry = Arc::clone(&registry);
            let run_dir = Arc::clone(&run_dir);
            let results = Arc::clone(&results);
            let completed_count = Arc::clone(&completed_count);
            let intermediate_save_total_ms = Arc::clone(&intermediate_save_total_ms);
            let on_fetch_fail = config.on_fetch_fail;
            let force_refetch = config.force_refetch;

            async move {
                // Check if we need to fetch (for progress display)
                let needs_fetch = force_refetch || !client.is_cached(&url);

                // Increment and get progress
                let completed = completed_count.fetch_add(1, Ordering::SeqCst) + 1;
                let progress = format!("[{}/{}]", completed, total_urls);

                // Print progress
                if force_refetch {
                    tracing::info!("{} {} (force refetch)", progress, truncate_url(&url, 60));
                } else if needs_fetch {
                    tracing::info!("{} {} (fetching...)", progress, truncate_url(&url, 60));
                } else {
                    tracing::info!("{} {} (cached)", progress, truncate_url(&url, 60));
                }

                // Run all steps
                let mut all_results = run_all_steps(
                    &url,
                    Arc::clone(&client),
                    &run_dir,
                    force_refetch,
                    Arc::clone(&registry),
                )
                .await;

                // Check if fetch failed
                let fetch_failed = all_results
                    .step_results
                    .first()
                    .map(|r| r.step == PipelineStep::FetchHtml && !r.success)
                    .unwrap_or(false);

                if fetch_failed {
                    match on_fetch_fail {
                        OnFetchFail::Skip => {
                            tracing::info!("  -> Skipped (fetch failed)");
                            return None;
                        }
                        OnFetchFail::Prompt => {
                            // Interactive mode: prompt user to save HTML
                            if let Ok(Some(new_results)) = prompt_for_manual_cache(
                                &url,
                                Arc::clone(&client),
                                &run_dir,
                                Arc::clone(&registry),
                            )
                            .await
                            {
                                all_results = new_results;
                            }
                            // If user skipped, all_results still has the failed fetch
                        }
                        OnFetchFail::Continue => {
                            // Default: just continue (already have failed result)
                        }
                    }
                }

                // Determine final status
                let final_status = determine_final_status(&all_results.step_results);

                // Update shared results
                {
                    let mut results = results.lock().await;
                    update_results(
                        &mut results,
                        &all_results.step_results,
                        &final_status,
                        &domain,
                        all_results.extraction_stats.as_ref(),
                        all_results.ingredient_stats.as_ref(),
                        all_results.ai_cached,
                    );

                    // Save intermediate results periodically
                    let save_start = Instant::now();
                    if let Err(e) = save_results(&run_dir, &results) {
                        tracing::warn!(error = %e, "Failed to save intermediate results");
                    }
                    intermediate_save_total_ms
                        .fetch_add(save_start.elapsed().as_millis() as usize, Ordering::Relaxed);
                }

                Some(UrlResult {
                    url,
                    site: domain,
                    steps: all_results.step_results,
                    final_status,
                    extraction_stats: all_results.extraction_stats,
                })
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;
    let processing_elapsed = processing_start.elapsed();
    let intermediate_save_ms = intermediate_save_total_ms.load(Ordering::Relaxed) as u64;
    tracing::info!(
        phase = "processing",
        elapsed_ms = processing_elapsed.as_millis() as u64,
        urls = total_urls,
        intermediate_save_ms,
        "URL processing complete"
    );

    // Collect all URL results into the final results
    let final_save_start = Instant::now();
    {
        let mut results = results.lock().await;
        for url_result in url_results.into_iter().flatten() {
            results.url_results.push(url_result);
        }
        save_results(&run_dir, &results).context("Failed to save final results")?;
    }
    let final_save_elapsed = final_save_start.elapsed();
    tracing::info!(
        phase = "final_save",
        elapsed_ms = final_save_elapsed.as_millis() as u64,
        "wrote final results.json"
    );

    // Extract final results from Arc<Mutex<>>
    let results = Arc::try_unwrap(results)
        .expect("All references should be dropped")
        .into_inner();

    let elapsed = start_time.elapsed();

    // Write allowlisted per-URL snapshots before the final manifest so that a
    // snapshot failure propagates as a failed run. On failure we still record
    // a terminal manifest status so external consumers aren't left observing a
    // run stuck in "running".
    let snapshot_start = Instant::now();
    let snapshot_allowlist = snapshot_allowlist_path(&config.test_urls_file);
    let snapshot_result = write_pipeline_snapshots(&run_dir, &snapshot_allowlist);
    let snapshot_elapsed = snapshot_start.elapsed();
    tracing::info!(
        phase = "snapshots",
        elapsed_ms = snapshot_elapsed.as_millis() as u64,
        ok = snapshot_result.is_ok(),
        "wrote pipeline snapshots"
    );
    if let Err(e) = snapshot_result {
        let failed_manifest = RunManifest {
            completed_at: Some(Utc::now().to_rfc3339()),
            status: RunStatus::Failed,
            ..manifest
        };
        if let Err(save_err) = save_manifest(&run_dir, &failed_manifest) {
            tracing::warn!(
                "Failed to persist failed manifest after snapshot error: {}",
                save_err
            );
        }
        return Err(e);
    }

    // Update manifest with completion
    let final_manifest = RunManifest {
        completed_at: Some(Utc::now().to_rfc3339()),
        status: RunStatus::Completed,
        ..manifest
    };
    save_manifest(&run_dir, &final_manifest)?;

    // Print summary
    tracing::info!("");
    tracing::info!("Pipeline Test Results");
    tracing::info!("=====================");
    tracing::info!("Run ID: {}", run_id);
    tracing::info!("Duration: {:.1}s", elapsed.as_secs_f64());
    tracing::info!("URLs Processed: {}", results.total_urls);
    tracing::info!("");
    tracing::info!("Phase Timing (wall clock):");
    tracing::info!(
        "  {:26} {:>7.1}s",
        "URL processing",
        processing_elapsed.as_secs_f64()
    );
    tracing::info!(
        "    (of which intermediate results.json writes held the mutex for {:.1}s)",
        intermediate_save_ms as f64 / 1000.0
    );
    tracing::info!(
        "  {:26} {:>7.1}s",
        "Final results.json write",
        final_save_elapsed.as_secs_f64()
    );
    tracing::info!(
        "  {:26} {:>7.1}s",
        "Snapshot write",
        snapshot_elapsed.as_secs_f64()
    );
    let accounted = processing_elapsed + final_save_elapsed + snapshot_elapsed;
    let unaccounted = elapsed.saturating_sub(accounted);
    tracing::info!(
        "  {:26} {:>7.1}s  (setup + everything not covered above)",
        "Unaccounted",
        unaccounted.as_secs_f64()
    );
    // Concurrency factor: if each URL ran serially, the sum of its step
    // durations is how long it would have taken. Dividing by wall-clock
    // processing time shows how much effective parallelism we got.
    let sum_step_ms: u64 = results
        .url_results
        .iter()
        .flat_map(|u| u.steps.iter().map(|s| s.duration_ms))
        .sum();
    let wall_ms = processing_elapsed.as_millis() as u64;
    if wall_ms > 0 {
        tracing::info!(
            "  {:26} {:>7.2}x  (sum of step times = {:.1}s; configured concurrency = {})",
            "Effective concurrency",
            sum_step_ms as f64 / wall_ms as f64,
            sum_step_ms as f64 / 1000.0,
            config.concurrency
        );
    }
    tracing::info!("");
    tracing::info!("Cache Stats:");
    tracing::info!(
        "  HTML cache hits: {} ({:.1}%)",
        results.cache_hits,
        if results.total_urls > 0 {
            results.cache_hits as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    tracing::info!("  HTML cache misses: {} (fetched)", results.cache_misses);
    let ai_total = results.ai_cache_hits + results.ai_cache_misses;
    if ai_total > 0 {
        tracing::info!(
            "  AI cache hits: {} ({:.1}%)",
            results.ai_cache_hits,
            results.ai_cache_hits as f64 / ai_total as f64 * 100.0
        );
        tracing::info!("  AI cache misses: {} (API calls)", results.ai_cache_misses);
    }
    tracing::info!("");
    tracing::info!("Overall Results:");
    tracing::info!(
        "  Completed: {} ({:.1}%)",
        results.completed,
        if results.total_urls > 0 {
            results.completed as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    tracing::info!(
        "  Failed at fetch_html: {} ({:.1}%)",
        results.failed_at_fetch,
        if results.total_urls > 0 {
            results.failed_at_fetch as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    tracing::info!(
        "  Failed at extract_recipe: {} ({:.1}%)",
        results.failed_at_extract,
        if results.total_urls > 0 {
            results.failed_at_extract as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    tracing::info!(
        "  Failed at save_recipe: {} ({:.1}%)",
        results.failed_at_save,
        if results.total_urls > 0 {
            results.failed_at_save as f64 / results.total_urls as f64 * 100.0
        } else {
            0.0
        }
    );
    tracing::info!("");
    tracing::info!("Results by Site:");

    // Sort sites by completion rate
    let mut sites: Vec<_> = results.by_site.values().collect();
    sites.sort_by(|a, b| {
        let rate_a = if a.total > 0 {
            a.completed as f64 / a.total as f64
        } else {
            0.0
        };
        let rate_b = if b.total > 0 {
            b.completed as f64 / b.total as f64
        } else {
            0.0
        };
        rate_b
            .partial_cmp(&rate_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for site in &sites {
        let rate = if site.total > 0 {
            site.completed as f64 / site.total as f64 * 100.0
        } else {
            0.0
        };
        tracing::info!(
            "  {}: {}/{} ({:.1}%)",
            site.domain,
            site.completed,
            site.total,
            rate
        );
    }

    tracing::info!("");
    tracing::info!("Extraction Method Stats:");
    let ems = &results.extraction_method_stats;
    if ems.urls_with_html > 0 {
        tracing::info!(
            "  JSON-LD: {}/{} ({:.1}%)",
            ems.jsonld_success,
            ems.urls_with_html,
            ems.jsonld_success as f64 / ems.urls_with_html as f64 * 100.0
        );
        tracing::info!(
            "  Microdata: {}/{} ({:.1}%)",
            ems.microdata_success,
            ems.urls_with_html,
            ems.microdata_success as f64 / ems.urls_with_html as f64 * 100.0
        );
        tracing::info!(
            "  Both: {}/{} ({:.1}%)",
            ems.both_success,
            ems.urls_with_html,
            ems.both_success as f64 / ems.urls_with_html as f64 * 100.0
        );
        tracing::info!(
            "  Neither: {}/{} ({:.1}%)",
            ems.neither_success,
            ems.urls_with_html,
            ems.neither_success as f64 / ems.urls_with_html as f64 * 100.0
        );
    } else {
        tracing::info!("  (no HTML fetched)");
    }

    // Print ingredient parsing stats
    let ips = &results.ingredient_stats;
    if ips.total_ingredients > 0 {
        tracing::info!("");
        tracing::info!("Ingredient Parsing Stats:");
        tracing::info!("  Total ingredients: {}", ips.total_ingredients);

        let volume_attempted =
            ips.volume_converted + ips.volume_unknown_ingredient + ips.volume_already_has_weight;
        if volume_attempted > 0 {
            tracing::info!(
                "  Volume→weight converted: {}/{} ({:.1}%)",
                ips.volume_converted,
                volume_attempted,
                ips.volume_converted as f64 / volume_attempted as f64 * 100.0
            );
            tracing::info!(
                "  Unknown ingredient (no density): {}",
                ips.volume_unknown_ingredient
            );
        }
        tracing::info!("  Already has weight: {}", ips.volume_already_has_weight);
        tracing::info!("  No volume unit: {}", ips.volume_no_volume);

        let metric_total = ips.metric_converted_oz + ips.metric_converted_lb;
        if metric_total > 0 {
            tracing::info!(
                "  Metric converted: {} oz→g, {} lb→g",
                ips.metric_converted_oz,
                ips.metric_converted_lb
            );
        }
    }

    // Print step timing summary
    print_timing_summary(&results);

    tracing::info!("");
    tracing::info!("Artifacts saved to: {}", run_dir.display());

    Ok(results)
}

// ============================================================================
// Helper functions
// ============================================================================

fn determine_final_status(steps: &[StepResult]) -> FinalStatus {
    for step in steps {
        if !step.success {
            match step.step {
                PipelineStep::FetchHtml => return FinalStatus::FailedAtFetch,
                PipelineStep::ExtractRecipe => return FinalStatus::FailedAtExtract,
                PipelineStep::SaveRecipe => return FinalStatus::FailedAtSave,
                PipelineStep::FetchImages | PipelineStep::ParseIngredients => {
                    // FetchImages is skipped in CLI, ParseIngredients runs before save
                    return FinalStatus::FailedAtSave;
                }
                PipelineStep::EnrichAutoTag | PipelineStep::ApplyAutoTags => {
                    // Enrichment failures are expected - don't fail the job
                    // Continue to check remaining steps
                }
            };
        }
    }
    FinalStatus::Completed
}

fn update_results(
    results: &mut PipelineResults,
    steps: &[StepResult],
    final_status: &FinalStatus,
    domain: &str,
    extraction_stats: Option<&ExtractionStats>,
    ingredient_stats: Option<&IngredientStats>,
    ai_cached: Option<bool>,
) {
    // Update HTML cache stats
    for step in steps {
        if step.step == PipelineStep::FetchHtml {
            if step.cached {
                results.cache_hits += 1;
            } else {
                results.cache_misses += 1;
            }
        }
    }

    // Update AI cache stats
    if let Some(cached) = ai_cached {
        if cached {
            results.ai_cache_hits += 1;
        } else {
            results.ai_cache_misses += 1;
        }
    }

    // Update overall stats
    match final_status {
        FinalStatus::Completed => results.completed += 1,
        FinalStatus::FailedAtFetch => results.failed_at_fetch += 1,
        FinalStatus::FailedAtExtract => results.failed_at_extract += 1,
        FinalStatus::FailedAtSave => results.failed_at_save += 1,
    }

    // Update extraction method stats
    // Count urls_with_html based on whether fetch succeeded (not just when extraction succeeds)
    let fetch_succeeded = steps
        .iter()
        .any(|s| s.step == PipelineStep::FetchHtml && s.success);

    if fetch_succeeded {
        results.extraction_method_stats.urls_with_html += 1;

        if let Some(stats) = extraction_stats {
            // We have extraction stats - count which methods worked
            if stats.jsonld_success {
                results.extraction_method_stats.jsonld_success += 1;
            }
            if stats.microdata_success {
                results.extraction_method_stats.microdata_success += 1;
            }
            if stats.jsonld_success && stats.microdata_success {
                results.extraction_method_stats.both_success += 1;
            }
            if !stats.jsonld_success && !stats.microdata_success {
                results.extraction_method_stats.neither_success += 1;
            }
        } else {
            // Fetch succeeded but no extraction stats - means extraction failed
            // This counts as "neither method succeeded"
            results.extraction_method_stats.neither_success += 1;
        }
    }

    // Update site stats
    let site_stats = results
        .by_site
        .entry(domain.to_string())
        .or_insert_with(|| SiteResults {
            domain: domain.to_string(),
            total: 0,
            completed: 0,
            failed: 0,
        });

    site_stats.total += 1;
    match final_status {
        FinalStatus::Completed => site_stats.completed += 1,
        _ => site_stats.failed += 1,
    }

    // Update ingredient parsing stats
    if let Some(stats) = ingredient_stats {
        results.ingredient_stats.total_ingredients += stats.total_ingredients;
        results.ingredient_stats.volume_converted += stats.volume_converted;
        results.ingredient_stats.volume_unknown_ingredient += stats.volume_unknown_ingredient;
        results.ingredient_stats.volume_no_volume += stats.volume_no_volume;
        results.ingredient_stats.volume_already_has_weight += stats.volume_already_has_weight;
        results.ingredient_stats.metric_converted_oz += stats.metric_converted_oz;
        results.ingredient_stats.metric_converted_lb += stats.metric_converted_lb;
        results
            .ingredient_stats
            .unknown_ingredients
            .extend(stats.unknown_ingredients.iter().cloned());
    }
}

fn save_manifest(run_dir: &Path, manifest: &RunManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(run_dir.join("manifest.json"), json)?;
    Ok(())
}

fn save_results(run_dir: &Path, results: &PipelineResults) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    fs::write(run_dir.join("results.json"), json)?;
    Ok(())
}

fn snapshot_allowlist_path(test_urls_path: &Path) -> PathBuf {
    test_urls_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("pipeline-snapshot-urls.json")
}

/// If `url` is a known non-recipe URL pattern that must not be added to the
/// snapshot allowlist, return a short reason explaining why. Today this only
/// catches Serious Eats "complete guide" articles (e.g. the food-lab guides),
/// which have no concrete `recipeIngredient` and break `make pipeline`.
fn unsupported_snapshot_url_reason(url: &str) -> Option<&'static str> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host == "seriouseats.com" && parsed.path().to_lowercase().contains("complete-guide-to-") {
        return Some(
            "Serious Eats \"complete guide\" articles are not concrete recipes; \
             use a specific recipe URL instead",
        );
    }
    None
}

fn load_pipeline_urls(test_urls_path: &Path) -> Result<TestUrlsOutput> {
    let test_urls_content = fs::read_to_string(test_urls_path)
        .with_context(|| format!("Failed to read test URLs from {}", test_urls_path.display()))?;
    let mut test_urls: TestUrlsOutput =
        serde_json::from_str(&test_urls_content).context("Failed to parse test URLs JSON")?;
    let mut existing_urls: HashSet<String> = test_urls
        .sites
        .iter()
        .flat_map(|site| site.urls.iter().cloned())
        .collect();

    let allowlist = snapshot_allowlist_path(test_urls_path);
    if !allowlist.exists() {
        return Ok(test_urls);
    }

    let allowlist_content = fs::read_to_string(&allowlist)
        .with_context(|| format!("Failed to read snapshot allowlist {}", allowlist.display()))?;
    let allowlisted_urls: Vec<String> = serde_json::from_str(&allowlist_content)
        .with_context(|| format!("Failed to parse snapshot allowlist {}", allowlist.display()))?;

    for url in allowlisted_urls {
        if let Some(reason) = unsupported_snapshot_url_reason(&url) {
            anyhow::bail!("Snapshot allowlist URL is not a recipe page: {url} ({reason})");
        }

        if !existing_urls.insert(url.clone()) {
            continue;
        }

        let domain = reqwest::Url::parse(&url)
            .with_context(|| format!("Invalid URL in snapshot allowlist: {url}"))?
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("Snapshot allowlist URL has no host: {url}"))?
            .to_string();

        if let Some(site) = test_urls
            .sites
            .iter_mut()
            .find(|site| site.domain == domain)
        {
            if !site.urls.iter().any(|existing| existing == &url) {
                site.urls.push(url);
            }
        } else {
            test_urls.sites.push(crate::generate_test_urls::SiteEntry {
                domain,
                rank: usize::MAX,
                urls: vec![url],
                error: None,
                source: crate::generate_test_urls::UrlSource::Merged,
            });
        }
    }

    for site in &mut test_urls.sites {
        site.urls.sort();
        site.urls.dedup();
    }
    test_urls
        .sites
        .sort_by_key(|site| (site.rank, site.domain.clone()));

    let mut seen_urls = HashSet::new();
    for site in &mut test_urls.sites {
        site.urls.retain(|url| seen_urls.insert(url.clone()));
    }
    test_urls.sites.retain(|site| !site.urls.is_empty());

    Ok(test_urls)
}

fn write_pipeline_snapshots(run_dir: &Path, allowlist: &Path) -> Result<()> {
    let snapshots_dir = PathBuf::from("data/pipeline-snapshots");

    if !allowlist.exists() {
        tracing::warn!(
            "Snapshot allowlist {} not found; skipping snapshot write",
            allowlist.display()
        );
        return Ok(());
    }

    crate::pipeline::snapshots::write_snapshots(run_dir, allowlist, &snapshots_dir)
}

fn truncate_url(url: &str, max_len: usize) -> String {
    if url.chars().count() <= max_len {
        url.to_string()
    } else {
        format!("{}...", url.chars().take(max_len - 3).collect::<String>())
    }
}

fn print_timing_summary(results: &PipelineResults) {
    // Aggregate step durations across all URLs
    let mut step_times: HashMap<String, Vec<u64>> = HashMap::new();

    for url_result in &results.url_results {
        for step in &url_result.steps {
            let step_name = format!("{:?}", step.step);
            step_times
                .entry(step_name)
                .or_default()
                .push(step.duration_ms);
        }
    }

    if step_times.is_empty() {
        return;
    }

    tracing::info!("");
    tracing::info!("Step Timing Summary:");

    // Order for consistent display
    let step_order = [
        "FetchHtml",
        "ExtractRecipe",
        "FetchImages",
        "ParseIngredients",
        "SaveRecipe",
        "EnrichAutoTag",
        "ApplyAutoTags",
    ];

    let mut grand_total_ms: u64 = 0;

    for step_name in step_order {
        if let Some(times) = step_times.get(step_name) {
            if times.is_empty() {
                continue;
            }
            let total: u64 = times.iter().sum();
            grand_total_ms += total;
            let avg = total as f64 / times.len() as f64;
            let max = *times.iter().max().unwrap_or(&0);
            let p50 = percentile(times, 50);
            let p95 = percentile(times, 95);

            tracing::info!(
                "  {:30} avg={:>6.0}ms  p50={:>6.0}ms  p95={:>6.0}ms  max={:>6}ms  total={:>7.1}s  (n={})",
                step_name,
                avg,
                p50,
                p95,
                max,
                total as f64 / 1000.0,
                times.len()
            );
        }
    }

    tracing::info!(
        "  {:30} {:>54.1}s",
        "TOTAL (sum of step times)",
        grand_total_ms as f64 / 1000.0
    );
}

fn percentile(times: &[u64], pct: usize) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    let mut sorted = times.to_vec();
    sorted.sort();
    let idx = (pct as f64 / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)] as f64
}

// ============================================================================
// Interactive cache prompting
// ============================================================================

/// Prompt user to manually save HTML for a URL, wait for file, and retry pipeline
async fn prompt_for_manual_cache(
    url: &str,
    client: Arc<CachingClient>,
    run_dir: &Path,
    registry: Arc<StepRegistry>,
) -> Result<Option<AllStepsResult>> {
    let staging = staging_dir();

    tracing::info!("");
    tracing::info!("  ┌─────────────────────────────────────────────────────────────┐");
    tracing::info!("  │ Fetch failed - manual cache needed                          │");
    tracing::info!("  └─────────────────────────────────────────────────────────────┘");
    tracing::info!("");
    tracing::info!("  URL: {}", url);
    tracing::info!("");
    tracing::info!("  To cache this page:");
    tracing::info!("  1. Open the URL above in your browser");
    tracing::info!("  2. Save the page (Cmd+S / Ctrl+S) to:");
    tracing::info!("     {}", staging.display());
    tracing::info!("");
    write!(
        io::stdout(),
        "  Waiting for .html file... (or type 'skip' + Enter): "
    )?;
    io::stdout().flush()?;

    // Clear any existing files in staging
    clear_staging()?;

    // Use a channel to communicate between stdin reader and file watcher
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);

    // Spawn a blocking task to read stdin
    let stdin_handle = tokio::task::spawn_blocking(move || {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() {
            let _ = tx.blocking_send(line);
        }
    });

    // Poll for file while waiting for stdin
    let poll_interval = Duration::from_millis(200);

    loop {
        // Check for file
        if let Some(staged_file) = find_staged_html() {
            // Wait a moment for write to complete
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Abort the stdin task
            stdin_handle.abort();

            // Import the file
            tracing::info!("");
            tracing::info!("  Found: {}", staged_file.display());

            // Read the HTML and inject into cache
            match fs::read_to_string(&staged_file) {
                Ok(html) => {
                    if let Err(e) = client.inject_html(url, &html) {
                        tracing::warn!(error = %e, "Failed to cache HTML");
                        tracing::info!("  Failed to cache: {}", e);
                        tracing::info!("  Continuing with failed status...");
                        tracing::info!("");
                        return Ok(None);
                    }

                    // Remove the staged file
                    let _ = fs::remove_file(&staged_file);

                    tracing::info!("  Cached successfully, retrying pipeline...");
                    tracing::info!("");

                    // Re-run all steps (should hit cache now)
                    let new_results =
                        run_all_steps(url, Arc::clone(&client), run_dir, false, registry).await;
                    return Ok(Some(new_results));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to read staged HTML file");
                    tracing::info!("  Failed to read file: {}", e);
                    tracing::info!("  Continuing with failed status...");
                    tracing::info!("");
                    return Ok(None);
                }
            }
        }

        // Check if user typed something
        match rx.try_recv() {
            Ok(line) => {
                if line.trim().eq_ignore_ascii_case("skip") || line.trim().is_empty() {
                    tracing::info!("");
                    tracing::info!("  Skipped by user");
                    tracing::info!("");
                    return Ok(None);
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No input yet, continue waiting
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                // Stdin closed, skip
                tracing::warn!("Stdin closed, skipping URL");
                return Ok(None);
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

// ============================================================================
// Cache stats command
// ============================================================================

pub fn print_cache_stats(cache_dir: &Path) {
    let cache = DiskCache::new(cache_dir.to_path_buf());
    let stats = cache.stats();

    tracing::info!("HTTP Cache Statistics");
    tracing::info!("=====================");
    tracing::info!("Cache directory: {}", cache_dir.display());
    tracing::info!("Cached responses (success): {}", stats.cached_success);
    tracing::info!("Cached errors: {}", stats.cached_errors);
    tracing::info!(
        "Total entries: {}",
        stats.cached_success + stats.cached_errors
    );
}

pub fn clear_cache(cache_dir: &Path) -> Result<()> {
    let cache = DiskCache::new(cache_dir.to_path_buf());
    cache.clear()?;
    tracing::info!("Cache cleared: {}", cache_dir.display());
    Ok(())
}

// ============================================================================
// Summary report generation
// ============================================================================

/// Get the path to the most recent pipeline run directory
pub fn get_latest_run_dir(output_dir: &Path) -> Result<(String, PathBuf)> {
    if !output_dir.exists() {
        anyhow::bail!(
            "Pipeline runs directory '{}' not found. Run `make pipeline` first.",
            output_dir.display()
        );
    }

    let mut runs: Vec<_> = fs::read_dir(output_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    runs.sort_by_key(|e| e.file_name());
    runs.reverse();

    let latest = runs
        .first()
        .ok_or_else(|| anyhow::anyhow!("No pipeline runs found in {}", output_dir.display()))?;

    let run_id = latest.file_name().to_string_lossy().to_string();
    Ok((run_id, latest.path()))
}

/// Output from the auto-tag step
#[derive(Debug, Deserialize)]
struct AutoTagOutput {
    suggested_tags: Vec<String>,
    cached: bool,
}

/// Generate a report of auto-tag suggestions from a pipeline run
pub fn generate_tag_report(run_dir: &Path) -> Result<String> {
    let mut report = String::new();
    report.push_str("# Auto-Tag Evaluation Report\n\n");

    let urls_dir = run_dir.join("urls");
    if !urls_dir.exists() {
        return Ok(report + "No URL results found.\n");
    }

    let mut entries: Vec<_> = fs::read_dir(&urls_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut total_with_tags = 0;
    let mut total_cached = 0;
    let mut tag_counts: HashMap<String, usize> = HashMap::new();

    report.push_str("## Per-Recipe Results\n\n");
    report.push_str("| Recipe | Tags | Cached |\n");
    report.push_str("|--------|------|--------|\n");

    for entry in &entries {
        let url_slug = entry.file_name().to_string_lossy().to_string();

        // Read extract_recipe output to get title
        let extract_path = entry.path().join("extract_recipe").join("output.json");
        let title = if extract_path.exists() {
            fs::read_to_string(&extract_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| {
                    v.get("raw_recipe")?
                        .get("title")?
                        .as_str()
                        .map(String::from)
                })
                .unwrap_or_else(|| url_slug.clone())
        } else {
            url_slug.clone()
        };

        // Read auto-tag output
        let tag_path = entry.path().join("enrich_auto_tag").join("output.json");
        if tag_path.exists() {
            if let Ok(content) = fs::read_to_string(&tag_path) {
                if let Ok(output) = serde_json::from_str::<AutoTagOutput>(&content) {
                    let tags_str = if output.suggested_tags.is_empty() {
                        "_none_".to_string()
                    } else {
                        output.suggested_tags.join(", ")
                    };

                    let cached_str = if output.cached { "yes" } else { "no" };

                    // Truncate title for table (character-safe)
                    let title_display = if title.chars().count() > 40 {
                        format!("{}...", title.chars().take(37).collect::<String>())
                    } else {
                        title.clone()
                    };

                    report.push_str(&format!(
                        "| {} | {} | {} |\n",
                        title_display, tags_str, cached_str
                    ));

                    if !output.suggested_tags.is_empty() {
                        total_with_tags += 1;
                    }
                    if output.cached {
                        total_cached += 1;
                    }

                    for tag in &output.suggested_tags {
                        *tag_counts.entry(tag.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Summary stats
    report.push_str("\n## Summary\n\n");
    report.push_str(&format!("- Total recipes processed: {}\n", entries.len()));
    report.push_str(&format!(
        "- Recipes with tag suggestions: {}\n",
        total_with_tags
    ));
    report.push_str(&format!("- Cached responses: {}\n", total_cached));

    // Tag frequency
    if !tag_counts.is_empty() {
        report.push_str("\n## Tag Frequency\n\n");
        report.push_str("| Tag | Count |\n");
        report.push_str("|-----|-------|\n");

        let mut sorted_tags: Vec<_> = tag_counts.iter().collect();
        sorted_tags.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

        for (tag, count) in sorted_tags {
            report.push_str(&format!("| {} | {} |\n", tag, count));
        }
    }

    Ok(report)
}

/// Generate a stable, diffable summary report from pipeline results
pub fn generate_summary_report(results: &PipelineResults) -> String {
    let mut report = String::new();

    // Overall stats
    report.push_str("# Pipeline Extraction Report\n\n");
    report.push_str("## Overall\n\n");
    report.push_str(&format!("- Total URLs: {}\n", results.total_urls));
    report.push_str(&format!(
        "- Completed: {} ({:.1}%)\n",
        results.completed,
        pct(results.completed, results.total_urls)
    ));
    report.push_str(&format!(
        "- Failed at fetch: {} ({:.1}%)\n",
        results.failed_at_fetch,
        pct(results.failed_at_fetch, results.total_urls)
    ));
    report.push_str(&format!(
        "- Failed at extract: {} ({:.1}%)\n",
        results.failed_at_extract,
        pct(results.failed_at_extract, results.total_urls)
    ));

    // Extraction method stats
    let ems = &results.extraction_method_stats;
    if ems.urls_with_html > 0 {
        report.push_str("\n## Extraction Methods\n\n");
        report.push_str(&format!(
            "- JSON-LD: {}/{} ({:.1}%)\n",
            ems.jsonld_success,
            ems.urls_with_html,
            pct(ems.jsonld_success, ems.urls_with_html)
        ));
        report.push_str(&format!(
            "- Microdata: {}/{} ({:.1}%)\n",
            ems.microdata_success,
            ems.urls_with_html,
            pct(ems.microdata_success, ems.urls_with_html)
        ));
        report.push_str(&format!(
            "- Both: {}/{} ({:.1}%)\n",
            ems.both_success,
            ems.urls_with_html,
            pct(ems.both_success, ems.urls_with_html)
        ));
        report.push_str(&format!(
            "- Neither: {}/{} ({:.1}%)\n",
            ems.neither_success,
            ems.urls_with_html,
            pct(ems.neither_success, ems.urls_with_html)
        ));
    }

    // Ingredient parsing stats
    let ips = &results.ingredient_stats;
    if ips.total_ingredients > 0 {
        report.push_str("\n## Ingredient Parsing\n\n");
        report.push_str(&format!("- Total ingredients: {}\n", ips.total_ingredients));

        // Volume-to-weight conversion stats
        let volume_attempted =
            ips.volume_converted + ips.volume_unknown_ingredient + ips.volume_already_has_weight;
        if volume_attempted > 0 {
            report.push_str(&format!(
                "- Volume-to-weight converted: {}/{} ({:.1}%)\n",
                ips.volume_converted,
                volume_attempted,
                pct(ips.volume_converted, volume_attempted)
            ));
            report.push_str(&format!(
                "- Unknown ingredient (no density data): {}\n",
                ips.volume_unknown_ingredient
            ));
        }
        report.push_str(&format!(
            "- Already has weight: {}\n",
            ips.volume_already_has_weight
        ));
        report.push_str(&format!(
            "- No volume unit (count-based): {}\n",
            ips.volume_no_volume
        ));

        // Metric conversion stats
        let metric_total = ips.metric_converted_oz + ips.metric_converted_lb;
        if metric_total > 0 {
            report.push_str(&format!(
                "- Metric converted (oz→g): {}\n",
                ips.metric_converted_oz
            ));
            report.push_str(&format!(
                "- Metric converted (lb→g): {}\n",
                ips.metric_converted_lb
            ));
        }
    }

    // AI cache stats
    let ai_total = results.ai_cache_hits + results.ai_cache_misses;
    if ai_total > 0 {
        report.push_str("\n## AI Cache\n\n");
        report.push_str(&format!(
            "- Cache hits: {}/{} ({:.1}%)\n",
            results.ai_cache_hits,
            ai_total,
            pct(results.ai_cache_hits, ai_total)
        ));
        report.push_str(&format!(
            "- API calls: {}/{} ({:.1}%)\n",
            results.ai_cache_misses,
            ai_total,
            pct(results.ai_cache_misses, ai_total)
        ));
    }

    // Per-site results (sorted alphabetically for stable diffs)
    report.push_str("\n## By Site\n\n");
    report.push_str("| Site | Completed | Total | Rate |\n");
    report.push_str("|------|-----------|-------|------|\n");

    let mut sites: Vec<_> = results.by_site.values().collect();
    sites.sort_by(|a, b| a.domain.cmp(&b.domain));

    for site in &sites {
        report.push_str(&format!(
            "| {} | {} | {} | {:.1}% |\n",
            site.domain,
            site.completed,
            site.total,
            pct(site.completed, site.total)
        ));
    }

    // Failed URLs grouped by error type (sorted for stability)
    let mut failures_by_error: std::collections::BTreeMap<String, Vec<&str>> =
        std::collections::BTreeMap::new();

    for url_result in &results.url_results {
        if !matches!(url_result.final_status, FinalStatus::Completed) {
            // Find the error message from the failed step
            let error = url_result
                .steps
                .iter()
                .find(|s| !s.success)
                .and_then(|s| s.error.as_ref())
                .map(|e| simplify_error(e))
                .unwrap_or_else(|| "Unknown error".to_string());

            failures_by_error
                .entry(error)
                .or_default()
                .push(&url_result.url);
        }
    }

    if !failures_by_error.is_empty() {
        report.push_str("\n## Failed URLs by Error\n");

        for (error, urls) in &failures_by_error {
            report.push_str(&format!("\n### {} ({} URLs)\n\n", error, urls.len()));
            let mut sorted_urls: Vec<_> = urls.iter().collect();
            sorted_urls.sort();
            for url in sorted_urls {
                report.push_str(&format!("- {}\n", url));
            }
        }
    }

    report
}

fn pct(num: usize, denom: usize) -> f64 {
    if denom == 0 {
        0.0
    } else {
        num as f64 / denom as f64 * 100.0
    }
}

/// Generate a density gap report showing the most common ingredients missing density data.
pub fn generate_density_gap_report(results: &PipelineResults) -> String {
    use std::collections::HashMap;

    let mut report = String::new();

    let unknown = &results.ingredient_stats.unknown_ingredients;
    if unknown.is_empty() {
        report.push_str("No unknown ingredients found.\n");
        return report;
    }

    // Count frequency of each ingredient name
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for name in unknown {
        *counts.entry(name.as_str()).or_default() += 1;
    }

    // Sort by frequency descending
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    report.push_str(&format!(
        "{} total occurrences across {} unique ingredients\n\n",
        unknown.len(),
        sorted.len()
    ));

    // Find max width needed for count column
    let max_count = sorted.first().map(|(_, c)| *c).unwrap_or(0);
    let count_width = max_count.to_string().len();

    for (name, count) in &sorted {
        report.push_str(&format!("{count:>count_width$}  {name}\n"));
    }

    report
}

/// Generate a sorted text file of unique ingredient names from a pipeline run
pub fn generate_unique_ingredients_file(run_dir: &Path) -> Result<String> {
    use std::collections::BTreeSet;

    let urls_dir = run_dir.join("urls");
    if !urls_dir.exists() {
        return Ok(String::new());
    }

    let mut unique_ingredients: BTreeSet<String> = BTreeSet::new();

    for entry in fs::read_dir(&urls_dir)?.filter_map(|e| e.ok()) {
        if !entry.path().is_dir() {
            continue;
        }

        let parse_path = entry.path().join("parse_ingredients").join("output.json");
        if !parse_path.exists() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&parse_path) {
            if let Ok(output) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(ingredients) = output.get("ingredients").and_then(|v| v.as_array()) {
                    for ingredient in ingredients {
                        if let Some(item) = ingredient.get("item").and_then(|v| v.as_str()) {
                            let item = item.trim();
                            if !item.is_empty() {
                                unique_ingredients.insert(item.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Return newline-separated sorted list
    Ok(unique_ingredients
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Simplify error messages for grouping (remove URL-specific parts)
fn simplify_error(error: &str) -> String {
    // Extract just the error type for grouping
    // Handle both new format ("No recipe found") and legacy ("No Recipe found in JSON-LD")
    if error.contains("No recipe found") || error.contains("No Recipe found") {
        "No recipe found".to_string()
    } else if error.contains("MissingField") {
        // Extract the field name using split_once for Unicode safety
        if let Some((_, after)) = error.split_once("MissingField(") {
            if let Some((field, _)) = after.split_once(')') {
                return format!("MissingField({})", field);
            }
        }
        "MissingField".to_string()
    } else if error.contains("Cached error") {
        "Cached fetch error".to_string()
    } else if error.contains("Fetch failed") {
        "Fetch failed".to_string()
    } else {
        // Truncate long errors
        let truncated: String = error.chars().take(50).collect();
        if truncated.len() < error.len() {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_test_urls() -> &'static str {
        r#"{
          "generated_at": "2026-04-22T00:00:00Z",
          "config": {
            "num_sites": 1,
            "urls_per_site": 2
          },
          "sites": [
            {
              "domain": "example.com",
              "rank": 1,
              "urls": [
                "https://example.com/already-present"
              ],
              "source": "sitemap"
            }
          ]
        }"#
    }

    #[test]
    fn load_pipeline_urls_unions_snapshot_allowlist() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let test_urls_path = data_dir.join("test-urls.json");
        std::fs::write(&test_urls_path, sample_test_urls()).unwrap();
        std::fs::write(
            data_dir.join("pipeline-snapshot-urls.json"),
            r#"[
              "https://example.com/already-present",
              "https://example.com/from-allowlist",
              "https://other.example/allowlisted-only"
            ]"#,
        )
        .unwrap();

        let loaded = load_pipeline_urls(&test_urls_path).unwrap();
        assert_eq!(loaded.sites.len(), 2);

        let example_site = loaded
            .sites
            .iter()
            .find(|site| site.domain == "example.com")
            .unwrap();
        assert_eq!(
            example_site.urls,
            vec![
                "https://example.com/already-present".to_string(),
                "https://example.com/from-allowlist".to_string(),
            ]
        );

        let allowlisted_only_site = loaded
            .sites
            .iter()
            .find(|site| site.domain == "other.example")
            .unwrap();
        assert_eq!(allowlisted_only_site.rank, usize::MAX);
        assert_eq!(
            allowlisted_only_site.urls,
            vec!["https://other.example/allowlisted-only".to_string()]
        );
        assert_eq!(
            allowlisted_only_site.source,
            crate::generate_test_urls::UrlSource::Merged
        );
    }

    #[test]
    fn load_pipeline_urls_without_allowlist_uses_test_urls_only() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let test_urls_path = data_dir.join("test-urls.json");
        std::fs::write(&test_urls_path, sample_test_urls()).unwrap();

        let loaded = load_pipeline_urls(&test_urls_path).unwrap();
        assert_eq!(loaded.sites.len(), 1);
        assert_eq!(loaded.sites[0].domain, "example.com");
        assert_eq!(
            loaded.sites[0].urls,
            vec!["https://example.com/already-present".to_string()]
        );
    }

    #[test]
    fn load_pipeline_urls_deduplicates_global_url_matches() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let test_urls_path = data_dir.join("test-urls.json");
        std::fs::write(
            &test_urls_path,
            r#"{
              "generated_at": "2026-04-22T00:00:00Z",
              "config": {
                "num_sites": 1,
                "urls_per_site": 1
              },
              "sites": [
                {
                  "domain": "seriouseats.com",
                  "rank": 1,
                  "urls": [
                    "https://www.seriouseats.com/already-present"
                  ],
                  "source": "sitemap"
                }
              ]
            }"#,
        )
        .unwrap();
        std::fs::write(
            data_dir.join("pipeline-snapshot-urls.json"),
            r#"[
              "https://www.seriouseats.com/already-present"
            ]"#,
        )
        .unwrap();

        let loaded = load_pipeline_urls(&test_urls_path).unwrap();
        assert_eq!(loaded.sites.len(), 1);
        assert_eq!(loaded.sites[0].domain, "seriouseats.com");
        assert_eq!(
            loaded.sites[0].urls,
            vec!["https://www.seriouseats.com/already-present".to_string()]
        );
    }

    #[test]
    fn load_pipeline_urls_rejects_seriouseats_complete_guide() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let test_urls_path = data_dir.join("test-urls.json");
        std::fs::write(&test_urls_path, sample_test_urls()).unwrap();
        std::fs::write(
            data_dir.join("pipeline-snapshot-urls.json"),
            r#"[
              "https://www.seriouseats.com/food-lab-complete-guide-to-sous-vide-steak"
            ]"#,
        )
        .unwrap();

        let err = load_pipeline_urls(&test_urls_path).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("food-lab-complete-guide-to-sous-vide-steak"),
            "error should name the offending URL: {msg}"
        );
        assert!(
            msg.contains("complete guide"),
            "error should explain the reason: {msg}"
        );
    }

    #[test]
    fn unsupported_snapshot_url_reason_allows_existing_recipe_urls() {
        // Recipes that include "food-lab" must not be flagged — only "complete-guide-to-"
        // guides on the Serious Eats host are rejected.
        assert!(unsupported_snapshot_url_reason(
            "https://www.seriouseats.com/chorizo-potato-tacos-how-to-food-lab-recipe"
        )
        .is_none());
        assert!(unsupported_snapshot_url_reason(
            "https://www.seriouseats.com/grilled-skirt-steak-fajitas-food-lab-recipe"
        )
        .is_none());
        assert!(unsupported_snapshot_url_reason(
            "https://www.seriouseats.com/marinara-sauce-recipe"
        )
        .is_none());
        assert!(unsupported_snapshot_url_reason(
            "https://www.seriouseats.com/food-lab-complete-guide-to-sous-vide-steak"
        )
        .is_some());
    }

    #[test]
    fn unsupported_snapshot_url_reason_only_matches_seriouseats_host() {
        // Other hosts whose name happens to embed "seriouseats.com" or whose path
        // contains "complete-guide-to-" must not be flagged.
        assert!(unsupported_snapshot_url_reason(
            "https://notseriouseats.com/food-lab-complete-guide-to-sous-vide-steak"
        )
        .is_none());
        assert!(
            unsupported_snapshot_url_reason("https://example.com/complete-guide-to-pizza")
                .is_none()
        );
        // The host check accepts both bare and www. variants.
        assert!(unsupported_snapshot_url_reason(
            "https://seriouseats.com/food-lab-complete-guide-to-sous-vide-steak"
        )
        .is_some());
        // Query strings or fragments that mention the pattern but aren't in the path
        // must not trigger.
        assert!(unsupported_snapshot_url_reason(
            "https://www.seriouseats.com/marinara-sauce-recipe?ref=complete-guide-to-pizza"
        )
        .is_none());
    }

    #[test]
    fn snapshot_allowlist_path_uses_test_urls_sibling_directory() {
        let custom_test_urls = Path::new("/tmp/custom-inputs/test-urls.json");
        assert_eq!(
            snapshot_allowlist_path(custom_test_urls),
            PathBuf::from("/tmp/custom-inputs/pipeline-snapshot-urls.json")
        );
    }
}
