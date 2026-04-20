//! Write end-of-pipeline recipe snapshots for an allowlisted set of URLs.
//!
//! After a pipeline run completes, this module reads the relevant per-step
//! outputs from `run_dir/urls/<slug>/` for each allowlisted URL, assembles a
//! `FinalRecipe` via `ramekin_core::final_recipe::build_final_recipe`, and
//! writes the JSON to `snapshots_dir/<slug>.json`. If an allowlisted URL
//! isn't present in the run directory, the function returns an error so the
//! pipeline run fails fast.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ramekin_core::final_recipe::{build_final_recipe, FinalRecipe};
use ramekin_core::http::slugify_url;
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::types::RawRecipe;

use crate::pipeline_orchestrator::PipelineResults;

/// Write snapshots for every URL in `allowlist_path` by reading step outputs
/// under `run_dir` and writing JSON files under `snapshots_dir`.
pub fn write_snapshots(run_dir: &Path, allowlist_path: &Path, snapshots_dir: &Path) -> Result<()> {
    let urls = read_allowlist(allowlist_path)?;
    if urls.is_empty() {
        return Ok(());
    }
    let run_results = read_run_results(run_dir)?;

    std::fs::create_dir_all(snapshots_dir)
        .with_context(|| format!("Failed to create {}", snapshots_dir.display()))?;

    for url in &urls {
        let slug = slugify_url(url);
        let final_recipe = assemble_snapshot(run_dir, run_results.as_ref(), url, &slug)
            .with_context(|| format!("Failed to assemble snapshot for allowlisted URL {url}"))?;

        let mut json = serde_json::to_string_pretty(&final_recipe)
            .context("Failed to serialize FinalRecipe")?;
        json.push('\n');

        let path = snapshots_dir.join(format!("{slug}.json"));
        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write snapshot: {}", path.display()))?;

        tracing::info!("Wrote pipeline snapshot: {}", path.display());
    }

    Ok(())
}

fn read_allowlist(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read allowlist: {}", path.display()))?;
    let urls: Vec<String> = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse allowlist JSON: {}", path.display()))?;
    Ok(urls)
}

fn read_run_results(run_dir: &Path) -> Result<Option<PipelineResults>> {
    let path = run_dir.join("results.json");
    if !path.exists() {
        return Ok(None);
    }

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read pipeline results: {}", path.display()))?;
    let results: PipelineResults = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse pipeline results JSON: {}", path.display()))?;
    Ok(Some(results))
}

fn read_step_output(
    run_dir: &Path,
    url_slug: &str,
    step_name: &str,
) -> Result<Option<serde_json::Value>> {
    let path = run_dir
        .join("urls")
        .join(url_slug)
        .join(step_name)
        .join("output.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read step output: {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse step output JSON: {}", path.display()))?;
    Ok(Some(value))
}

fn missing_extract_recipe_error(
    run_dir: &Path,
    run_results: Option<&PipelineResults>,
    url: &str,
    url_slug: &str,
) -> anyhow::Error {
    if let Some(url_result) =
        run_results.and_then(|results| results.url_results.iter().find(|result| result.url == url))
    {
        if let Some(failed_step) = url_result.steps.iter().find(|step| !step.success) {
            let step_error = failed_step
                .error
                .as_deref()
                .unwrap_or("No step error message recorded");
            return anyhow!(
                "No extract_recipe output for URL slug {url_slug} in {}. \
                 The pipeline processed this URL but it ended with final_status={:?} \
                 after step {:?} failed: {}",
                run_dir.display(),
                url_result.final_status,
                failed_step.step,
                step_error
            );
        }

        return anyhow!(
            "No extract_recipe output for URL slug {url_slug} in {}. \
             The pipeline recorded this URL with final_status={:?}, but no failed \
             step was recorded.",
            run_dir.display(),
            url_result.final_status
        );
    }

    anyhow!(
        "No extract_recipe output for URL slug {url_slug} in {}. \
         Either the URL wasn't in the pipeline run or the pipeline didn't reach extract_recipe.",
        run_dir.display()
    )
}

fn assemble_snapshot(
    run_dir: &Path,
    run_results: Option<&PipelineResults>,
    url: &str,
    url_slug: &str,
) -> Result<FinalRecipe> {
    let extract = read_step_output(run_dir, url_slug, "extract_recipe")?
        .ok_or_else(|| missing_extract_recipe_error(run_dir, run_results, url, url_slug))?;

    let raw_recipe: RawRecipe =
        serde_json::from_value(extract.get("raw_recipe").cloned().ok_or_else(|| {
            anyhow!("extract_recipe output missing raw_recipe field for {url_slug}")
        })?)
        .context("Failed to deserialize raw_recipe from extract_recipe output")?;

    let parsed_ingredients: Option<Vec<ParsedIngredient>> =
        read_step_output(run_dir, url_slug, "parse_ingredients")?
            .and_then(|v| v.get("ingredients").cloned())
            .map(serde_json::from_value)
            .transpose()
            .context("Failed to deserialize parse_ingredients output")?;

    let suggested_tags: Option<Vec<String>> =
        read_step_output(run_dir, url_slug, "enrich_auto_tag")?
            .and_then(|v| v.get("suggested_tags").cloned())
            .map(serde_json::from_value)
            .transpose()
            .context("Failed to deserialize enrich_auto_tag suggested_tags")?;

    Ok(build_final_recipe(
        &raw_recipe,
        parsed_ingredients.as_deref(),
        suggested_tags.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reads_valid_allowlist() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allowlist.json");
        std::fs::write(&path, r#"["https://a.example/", "https://b.example/"]"#).unwrap();
        let urls = read_allowlist(&path).unwrap();
        assert_eq!(
            urls,
            vec![
                "https://a.example/".to_string(),
                "https://b.example/".to_string()
            ],
        );
    }

    #[test]
    fn reads_empty_allowlist() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allowlist.json");
        std::fs::write(&path, "[]").unwrap();
        let urls = read_allowlist(&path).unwrap();
        assert!(urls.is_empty());
    }

    #[test]
    fn errors_on_missing_file() {
        let err = read_allowlist(Path::new("/nonexistent/allowlist.json")).unwrap_err();
        assert!(err.to_string().contains("Failed to read allowlist"));
    }

    #[test]
    fn errors_on_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allowlist.json");
        std::fs::write(&path, "not json").unwrap();
        let err = read_allowlist(&path).unwrap_err();
        assert!(err.to_string().contains("Failed to parse allowlist JSON"));
    }

    #[test]
    fn reads_step_output_when_present() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path();
        let step_dir = run_dir
            .join("urls")
            .join("example-com_recipe")
            .join("extract_recipe");
        std::fs::create_dir_all(&step_dir).unwrap();
        std::fs::write(step_dir.join("output.json"), r#"{"foo":"bar"}"#).unwrap();

        let out = read_step_output(run_dir, "example-com_recipe", "extract_recipe").unwrap();
        assert_eq!(out, Some(serde_json::json!({"foo": "bar"})));
    }

    #[test]
    fn returns_none_when_step_output_missing() {
        let dir = TempDir::new().unwrap();
        let out = read_step_output(dir.path(), "example-com_recipe", "extract_recipe").unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn errors_when_step_output_is_malformed_json() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path();
        let step_dir = run_dir
            .join("urls")
            .join("example-com_recipe")
            .join("extract_recipe");
        std::fs::create_dir_all(&step_dir).unwrap();
        std::fs::write(step_dir.join("output.json"), "not json").unwrap();

        let err = read_step_output(run_dir, "example-com_recipe", "extract_recipe").unwrap_err();
        assert!(err.to_string().contains("Failed to parse"));
    }

    fn write_step_output(run_dir: &Path, slug: &str, step: &str, body: &str) {
        let dir = run_dir.join("urls").join(slug).join(step);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("output.json"), body).unwrap();
    }

    fn extract_output_body() -> &'static str {
        r#"{
          "raw_recipe": {
            "title": "Test",
            "ingredients": "1 cup flour\n2 eggs",
            "instructions": "Mix.",
            "image_urls": []
          },
          "method_used": "json_ld"
        }"#
    }

    #[test]
    fn assembles_with_only_extract_recipe() {
        let dir = TempDir::new().unwrap();
        write_step_output(
            dir.path(),
            "example-com_r",
            "extract_recipe",
            extract_output_body(),
        );

        let fr =
            assemble_snapshot(dir.path(), None, "https://example.com/r", "example-com_r").unwrap();
        assert_eq!(fr.title, "Test");
        assert_eq!(fr.ingredients.len(), 2); // line-split fallback
        assert!(fr.suggested_tags.is_none());
    }

    #[test]
    fn assembles_with_parse_ingredients_and_tags() {
        let dir = TempDir::new().unwrap();
        let slug = "example-com_r";
        write_step_output(dir.path(), slug, "extract_recipe", extract_output_body());
        write_step_output(
            dir.path(),
            slug,
            "parse_ingredients",
            r#"{
              "ingredients": [
                {
                  "item": "flour",
                  "measurements": [{"amount": "1", "unit": "cup"}],
                  "note": null,
                  "raw": "1 cup flour",
                  "section": null
                }
              ]
            }"#,
        );
        write_step_output(
            dir.path(),
            slug,
            "enrich_auto_tag",
            r#"{"suggested_tags": ["dinner", "breakfast"]}"#,
        );

        let fr = assemble_snapshot(dir.path(), None, "https://example.com/r", slug).unwrap();
        assert_eq!(fr.ingredients.len(), 1);
        assert_eq!(fr.ingredients[0].item, "flour");
        assert_eq!(
            fr.suggested_tags.as_deref(),
            Some(&["dinner".to_string(), "breakfast".to_string()][..]),
        );
    }

    #[test]
    fn errors_when_extract_recipe_output_missing() {
        let dir = TempDir::new().unwrap();
        let err = assemble_snapshot(dir.path(), None, "https://missing.example/", "missing-slug")
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("missing-slug"),
            "error should name the slug: {msg}"
        );
        assert!(
            msg.contains("extract_recipe"),
            "error should mention extract_recipe: {msg}"
        );
    }

    #[test]
    fn reports_pipeline_failure_details_when_extract_recipe_output_missing() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path();
        let results = serde_json::json!({
            "total_urls": 1,
            "completed": 0,
            "failed_at_fetch": 1,
            "failed_at_extract": 0,
            "failed_at_save": 0,
            "cache_hits": 0,
            "cache_misses": 1,
            "ai_cache_hits": 0,
            "ai_cache_misses": 0,
            "by_site": {},
            "url_results": [
                {
                    "url": "https://example.com/offline-miss",
                    "site": "example.com",
                    "steps": [
                        {
                            "step": "fetch_html",
                            "success": false,
                            "duration_ms": 0,
                            "error": "URL not cached and RAMEKIN_OFFLINE is set",
                            "cached": false
                        }
                    ],
                    "final_status": "failed_at_fetch"
                }
            ],
            "extraction_method_stats": {
                "urls_with_html": 0,
                "jsonld_success": 0,
                "microdata_success": 0,
                "both_success": 0,
                "neither_success": 0
            },
            "ingredient_stats": {
                "total_ingredients": 0,
                "volume_converted": 0,
                "volume_unknown_ingredient": 0,
                "volume_no_volume": 0,
                "volume_already_has_weight": 0,
                "metric_converted_oz": 0,
                "metric_converted_lb": 0,
                "unknown_ingredients": []
            }
        });
        std::fs::write(
            run_dir.join("results.json"),
            serde_json::to_string_pretty(&results).unwrap(),
        )
        .unwrap();

        let run_results = read_run_results(run_dir).unwrap();
        let err = assemble_snapshot(
            run_dir,
            run_results.as_ref(),
            "https://example.com/offline-miss",
            "example-com_offline-miss",
        )
        .unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("final_status=FailedAtFetch"));
        assert!(msg.contains("step FetchHtml failed"));
        assert!(msg.contains("RAMEKIN_OFFLINE"));
    }

    #[test]
    fn writes_snapshot_files_for_allowlisted_urls() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        let snapshots_dir = dir.path().join("snapshots");
        let url = "https://example.com/r";
        let slug = slugify_url(url);
        write_step_output(&run_dir, &slug, "extract_recipe", extract_output_body());

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, format!("[{url:?}]")).unwrap();

        write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap();

        let snapshot_path = snapshots_dir.join(format!("{slug}.json"));
        assert!(
            snapshot_path.exists(),
            "snapshot not written at {}",
            snapshot_path.display()
        );
        let content = std::fs::read_to_string(&snapshot_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("title").and_then(|v| v.as_str()), Some("Test"));
    }

    #[test]
    fn errors_when_allowlisted_url_missing_from_run() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let snapshots_dir = dir.path().join("snapshots");
        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, r#"["https://missing.example/"]"#).unwrap();

        let err = write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("https://missing.example/"),
            "error should name the URL: {msg}"
        );
    }

    #[test]
    fn snapshot_output_is_pretty_printed() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        let snapshots_dir = dir.path().join("snapshots");
        let url = "https://example.com/r";
        let slug = slugify_url(url);
        write_step_output(&run_dir, &slug, "extract_recipe", extract_output_body());

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, format!("[{url:?}]")).unwrap();
        write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap();

        let snapshot = std::fs::read_to_string(snapshots_dir.join(format!("{slug}.json"))).unwrap();
        assert!(
            snapshot.contains('\n'),
            "expected pretty-printed multi-line JSON"
        );
        assert!(snapshot.ends_with('\n'), "expected trailing newline");
    }
}
