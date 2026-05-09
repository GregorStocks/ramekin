//! Write end-of-pipeline recipe snapshots for an allowlisted set of URLs.
//!
//! After a pipeline run completes, this module reads the relevant per-step
//! outputs from `run_dir/urls/<slug>/` for each allowlisted URL, assembles a
//! `FinalRecipe` via `ramekin_core::final_recipe::build_final_recipe`, and
//! writes the JSON to `snapshots_dir/<slug>.json`. Every allowlisted URL is
//! expected to have reached `extract_recipe`; if one did not, that is a real
//! pipeline coverage regression and the phase fails.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ramekin_core::final_recipe::{build_final_recipe, FinalRecipe};
use ramekin_core::http::slugify_url;
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::types::RawRecipe;

use crate::pipeline_orchestrator::PipelineResults;

/// Write snapshots for every URL in `allowlist_path` by reading step outputs
/// under `run_dir` and writing JSON files under `snapshots_dir`.
///
/// Every allowlisted URL must have `extract_recipe` output in this run.
/// Missing output is treated as a hard failure with a specific reason so the
/// pipeline cannot silently drift away from the committed allowlist. Other
/// errors — unreadable allowlist, mkdir failure, corrupt step output JSON,
/// serialization failure, write failure — also propagate.
pub fn write_snapshots(run_dir: &Path, allowlist_path: &Path, snapshots_dir: &Path) -> Result<()> {
    let urls = read_allowlist(allowlist_path)?;
    if urls.is_empty() {
        return Ok(());
    }
    let run_results = read_run_results(run_dir)?;

    std::fs::create_dir_all(snapshots_dir)
        .with_context(|| format!("Failed to create {}", snapshots_dir.display()))?;

    let mut written = 0usize;
    let mut missing: Vec<(String, String)> = Vec::new();
    for url in &urls {
        let slug = slugify_url(url);
        let path = snapshots_dir.join(format!("{slug}.json"));
        let final_recipe = match assemble_snapshot(run_dir, run_results.as_ref(), url, &slug)? {
            Some(recipe) => recipe,
            None => {
                let reason = describe_missing_extract(run_dir, run_results.as_ref(), url, &slug);
                if path.exists() {
                    std::fs::remove_file(&path).with_context(|| {
                        format!("Failed to remove stale snapshot {}", path.display())
                    })?;
                    tracing::error!(
                        url = %url,
                        slug = %slug,
                        reason = %reason,
                        path = %path.display(),
                        "removed stale pipeline snapshot for missing allowlisted extract output"
                    );
                }
                missing.push((url.clone(), reason));
                continue;
            }
        };

        let mut json = serde_json::to_string_pretty(&final_recipe)
            .context("Failed to serialize FinalRecipe")?;
        json.push('\n');

        std::fs::write(&path, json)
            .with_context(|| format!("Failed to write snapshot: {}", path.display()))?;

        tracing::info!("Wrote pipeline snapshot: {}", path.display());
        written += 1;
    }

    tracing::info!(
        allowlisted = urls.len(),
        written,
        missing = missing.len(),
        "pipeline snapshot phase complete"
    );

    if !missing.is_empty() {
        let mut message = format!(
            "Allowlisted URLs missing extract_recipe output ({}):",
            missing.len()
        );
        for (url, reason) in &missing {
            message.push_str(&format!("\n  - {url}: {reason}"));
        }
        anyhow::bail!(message);
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

/// Human-readable reason explaining why `extract_recipe` output was absent
/// for `url`. Pulled from the pipeline `results.json` when available so the
/// warn log in `write_snapshots` says *why* the URL was skipped (failed
/// fetch, failed extract, never processed) rather than just "not present".
fn describe_missing_extract(
    run_dir: &Path,
    run_results: Option<&PipelineResults>,
    url: &str,
    url_slug: &str,
) -> String {
    if let Some(url_result) =
        run_results.and_then(|results| results.url_results.iter().find(|result| result.url == url))
    {
        if let Some(failed_step) = url_result.steps.iter().find(|step| !step.success) {
            let step_error = failed_step
                .error
                .as_deref()
                .unwrap_or("No step error message recorded");
            return format!(
                "No extract_recipe output for URL slug {url_slug} in {}. \
                 The pipeline processed this URL but it ended with final_status={:?} \
                 after step {:?} failed: {}",
                run_dir.display(),
                url_result.final_status,
                failed_step.step,
                step_error
            );
        }

        return format!(
            "No extract_recipe output for URL slug {url_slug} in {}. \
             The pipeline recorded this URL with final_status={:?}, but no failed \
             step was recorded.",
            run_dir.display(),
            url_result.final_status
        );
    }

    format!(
        "No extract_recipe output for URL slug {url_slug} in {}. \
         Either the URL wasn't in the pipeline run or the pipeline didn't reach extract_recipe.",
        run_dir.display()
    )
}

/// Build a `FinalRecipe` from this URL's step outputs.
///
/// Returns `Ok(None)` when `extract_recipe` produced no output for the slug —
/// callers treat `Ok(None)` as a hard failure because every allowlisted URL
/// must have reached `extract_recipe` in the current run. Any other problem
/// (corrupt step-output JSON, missing `raw_recipe` field, deserialization
/// mismatch) also surfaces as `Err`.
fn assemble_snapshot(
    run_dir: &Path,
    _run_results: Option<&PipelineResults>,
    _url: &str,
    url_slug: &str,
) -> Result<Option<FinalRecipe>> {
    let Some(extract) = read_step_output(run_dir, url_slug, "extract_recipe")? else {
        return Ok(None);
    };

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

    let normalized_title: Option<String> =
        read_step_output(run_dir, url_slug, "enrich_normalize_title")?.and_then(|v| {
            let changed = v.get("changed").and_then(|v| v.as_bool()).unwrap_or(false);
            if changed {
                v.get("normalized_title")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            } else {
                None
            }
        });

    let generated_description: Option<String> =
        read_step_output(run_dir, url_slug, "enrich_generate_description")?.and_then(|v| {
            let changed = v.get("changed").and_then(|v| v.as_bool()).unwrap_or(false);
            if changed {
                v.get("generated_description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            } else {
                None
            }
        });

    Ok(Some(build_final_recipe(
        &raw_recipe,
        parsed_ingredients.as_deref(),
        normalized_title.as_deref(),
        generated_description.as_deref(),
        suggested_tags.as_deref(),
    )))
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

        let fr = assemble_snapshot(dir.path(), None, "https://example.com/r", "example-com_r")
            .unwrap()
            .expect("extract_recipe output present");
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

        let fr = assemble_snapshot(dir.path(), None, "https://example.com/r", slug)
            .unwrap()
            .expect("extract_recipe output present");
        assert_eq!(fr.ingredients.len(), 1);
        assert_eq!(fr.ingredients[0].item, "flour");
        assert_eq!(
            fr.suggested_tags.as_deref(),
            Some(&["dinner".to_string(), "breakfast".to_string()][..]),
        );
    }

    #[test]
    fn returns_none_when_extract_recipe_output_missing() {
        let dir = TempDir::new().unwrap();
        let got = assemble_snapshot(dir.path(), None, "https://missing.example/", "missing-slug")
            .unwrap();
        assert!(
            got.is_none(),
            "missing extract_recipe output should map to Ok(None), got {got:?}"
        );
    }

    #[test]
    fn errors_when_extract_recipe_output_is_corrupt() {
        let dir = TempDir::new().unwrap();
        write_step_output(
            dir.path(),
            "example-com_r",
            "extract_recipe",
            "not valid json",
        );
        let err = assemble_snapshot(dir.path(), None, "https://example.com/r", "example-com_r")
            .unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse"),
            "corrupt extract_recipe output should bubble up, got {err}"
        );
    }

    #[test]
    fn errors_when_extract_recipe_output_missing_raw_recipe_field() {
        let dir = TempDir::new().unwrap();
        write_step_output(
            dir.path(),
            "example-com_r",
            "extract_recipe",
            r#"{"method_used":"json_ld"}"#,
        );
        let err = assemble_snapshot(dir.path(), None, "https://example.com/r", "example-com_r")
            .unwrap_err();
        assert!(
            err.to_string().contains("missing raw_recipe"),
            "missing raw_recipe field should bubble up, got {err}"
        );
    }

    #[test]
    fn describes_missing_extract_with_pipeline_failure_details() {
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
        let msg = describe_missing_extract(
            run_dir,
            run_results.as_ref(),
            "https://example.com/offline-miss",
            "example-com_offline-miss",
        );

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

        let slug = slugify_url("https://missing.example/");
        assert!(
            !snapshots_dir.join(format!("{slug}.json")).exists(),
            "no snapshot should be written for a URL that didn't reach extract_recipe"
        );
        let msg = err.to_string();
        assert!(msg.contains("Allowlisted URLs missing extract_recipe output"));
        assert!(msg.contains("https://missing.example/"));
    }

    #[test]
    fn errors_even_if_other_allowlisted_urls_succeeded() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        let snapshots_dir = dir.path().join("snapshots");
        let ok_url = "https://example.com/ok";
        let ok_slug = slugify_url(ok_url);
        write_step_output(&run_dir, &ok_slug, "extract_recipe", extract_output_body());

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(
            &allowlist,
            format!(r#"[{ok_url:?}, "https://missing.example/"]"#),
        )
        .unwrap();

        let err = write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap_err();

        assert!(
            snapshots_dir.join(format!("{ok_slug}.json")).exists(),
            "successful snapshots should be written even when other allowlisted URLs fail"
        );
        let missing_slug = slugify_url("https://missing.example/");
        assert!(
            !snapshots_dir.join(format!("{missing_slug}.json")).exists(),
            "no snapshot for the URL that had no extract_recipe output"
        );
        assert!(err.to_string().contains("https://missing.example/"));
    }

    #[test]
    fn lists_all_missing_urls_in_error() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let snapshots_dir = dir.path().join("snapshots");

        // One URL that did extract successfully, sandwiched between three that
        // didn't, to verify we keep iterating past every kind of position.
        let ok_url = "https://example.com/ok";
        let ok_slug = slugify_url(ok_url);
        write_step_output(&run_dir, &ok_slug, "extract_recipe", extract_output_body());

        let missing_a = "https://missing.example/a";
        let missing_b = "https://missing.example/b";
        let missing_c = "https://missing.example/c";

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(
            &allowlist,
            format!(r#"[{missing_a:?}, {ok_url:?}, {missing_b:?}, {missing_c:?}]"#),
        )
        .unwrap();

        let err = write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap_err();
        let msg = err.to_string();

        for url in [missing_a, missing_b, missing_c] {
            assert!(
                msg.contains(url),
                "missing URL not reported in error: {url}\nerror was: {msg}"
            );
        }
        assert!(
            msg.contains("(3)"),
            "error should report a count of missing URLs; got: {msg}"
        );
        assert!(
            snapshots_dir.join(format!("{ok_slug}.json")).exists(),
            "the one successful URL should still get a snapshot"
        );
    }

    #[test]
    fn removes_stale_snapshot_when_url_no_longer_extracts() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let snapshots_dir = dir.path().join("snapshots");
        std::fs::create_dir_all(&snapshots_dir).unwrap();

        // A snapshot from a previous run exists on disk for a URL that the
        // current run can't produce extract_recipe output for.
        let url = "https://example.com/now-broken";
        let slug = slugify_url(url);
        let stale_path = snapshots_dir.join(format!("{slug}.json"));
        std::fs::write(&stale_path, r#"{"title":"Stale"}"#).unwrap();

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, format!("[{url:?}]")).unwrap();

        let err = write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap_err();

        assert!(
            !stale_path.exists(),
            "stale snapshot should be removed when the current run can't refresh it"
        );
        assert!(err
            .to_string()
            .contains("Allowlisted URLs missing extract_recipe output"));
    }

    #[test]
    fn write_snapshots_propagates_corrupt_extract_output() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        let snapshots_dir = dir.path().join("snapshots");
        let url = "https://example.com/corrupt";
        let slug = slugify_url(url);
        write_step_output(&run_dir, &slug, "extract_recipe", "definitely not json");

        let allowlist = dir.path().join("allowlist.json");
        std::fs::write(&allowlist, format!("[{url:?}]")).unwrap();

        let err = write_snapshots(&run_dir, &allowlist, &snapshots_dir).unwrap_err();
        assert!(
            err.to_string().contains("Failed to parse")
                || format!("{err:?}").contains("Failed to parse"),
            "corrupt extract_recipe output should fail the phase, got {err}"
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
