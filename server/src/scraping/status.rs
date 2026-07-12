//! Helpers for building the per-step status view used by the scrape status API.
//!
//! `build_step_states` reads from `scrape_jobs` and `step_outputs` and produces
//! an ordered list of `StepState` entries — one per pipeline step — combining
//! whatever outputs have been written with the job's live state (current step,
//! failure, etc.). `step_summary` produces a short human-readable summary line
//! for a completed step's stored output.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use serde_json::Value as JsonValue;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::DbPool;
use crate::schema::step_outputs;
use crate::scraping::{run_scrape_db, ScrapeError};
use ramekin_core::pipeline::scrape_pipeline_step_names;

/// Row shape read by `build_step_states_from_outputs`:
/// `(step_name, created_at, duration_ms, summary, success, error)`. The full
/// `output` JSON is intentionally excluded — it can be multi-MB for
/// `fetch_html` / `extract_recipe` and is loaded on demand by the expand-step
/// endpoint only.
type StepOutputRow = (
    String,
    DateTime<Utc>,
    Option<i64>,
    Option<String>,
    bool,
    Option<String>,
);

/// A single pipeline step's state for the status API response.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StepState {
    pub name: String,
    /// One of "pending", "running", "completed", "failed", "skipped".
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub has_output: bool,
}

/// Produce a short human-readable summary line for a completed step.
///
/// Returns None when the step's output has no interesting fields to surface.
/// Each arm mirrors the actual output shape written by the corresponding
/// `PipelineStep` implementation — keep these in sync.
pub fn step_summary(step_name: &str, output: &JsonValue) -> Option<String> {
    match step_name {
        // FetchHtmlStep stores `{ "html": "<...>" }`; derive byte length from it.
        "fetch_html" => {
            let bytes = output
                .get("html")
                .and_then(|v| v.as_str())
                .map(|s| s.len())?;
            Some(format!("{} bytes fetched", bytes))
        }
        // ExtractRecipeStep stores `ExtractRecipeOutput` with `raw_recipe.title`.
        "extract_recipe" => {
            let title = output
                .get("raw_recipe")
                .and_then(|r| r.get("title"))
                .and_then(|t| t.as_str())?;
            Some(format!("extracted \"{}\"", title))
        }
        // FetchImagesStep stores `FetchImagesOutput { photo_ids, failed_urls }`.
        // "Requested" = photo_ids + failed_urls; "succeeded" = photo_ids.
        "fetch_images" => {
            let succeeded = output
                .get("photo_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let failed = output
                .get("failed_urls")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let requested = succeeded + failed;
            Some(format!("{}/{} images", succeeded, requested))
        }
        // ParseIngredientsStep stores `ParseIngredientsOutput { ingredients, ... }`.
        "parse_ingredients" => {
            let count = output
                .get("ingredients")
                .and_then(|v| v.as_array())
                .map(|a| a.len())?;
            Some(format!("{} ingredients parsed", count))
        }
        // SaveRecipeStep stores `{ "recipe_id": "<uuid>" }`.
        "save_recipe" => {
            let id = output.get("recipe_id").and_then(|v| v.as_str())?;
            Some(format!("saved recipe {}", id))
        }
        // EnrichNormalizeTitleStep stores `{ changed, normalized_title, ... }`.
        "enrich_normalize_title" => {
            let changed = output
                .get("changed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(if changed {
                "title normalized".to_string()
            } else {
                "title unchanged".to_string()
            })
        }
        // ApplyNormalizedTitleStep stores `{ changed, new_version_id? }`.
        "apply_normalized_title" => {
            let changed = output
                .get("changed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(if changed {
                "normalized title applied".to_string()
            } else {
                "no title change applied".to_string()
            })
        }
        // EnrichGenerateDescriptionStep stores `{ changed, generated_description, ... }`.
        "enrich_generate_description" => {
            let changed = output
                .get("changed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(if changed {
                "description generated".to_string()
            } else {
                "description unchanged".to_string()
            })
        }
        // ApplyGeneratedDescriptionStep stores `{ changed, new_version_id? }`.
        "apply_generated_description" => {
            let changed = output
                .get("changed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(if changed {
                "generated description applied".to_string()
            } else {
                "no description change applied".to_string()
            })
        }
        // EnrichAutoTagStep stores `{ suggested_tags, cached, usage }`.
        "enrich_auto_tag" => {
            let count = output
                .get("suggested_tags")
                .and_then(|v| v.as_array())
                .map(|a| a.len())?;
            Some(format!("{} tags suggested", count))
        }
        // ApplyAutoTagsStep stores `{ tags_applied: [...], new_version_id? }`
        // (or `{ message, tags_applied: [] }` when there's nothing to apply).
        "apply_auto_tags" => {
            let count = output
                .get("tags_applied")
                .and_then(|v| v.as_array())
                .map(|a| a.len())?;
            Some(format!("{} tags applied", count))
        }
        _ => None,
    }
}

/// Produce a synthetic `StepState` for a `failed_at_step` value that isn't
/// one of the canonical scrape pipeline steps (e.g. `photo_extract` for the
/// photo-only import path). Without this, the status API would render every
/// canonical step as `"pending"` and silently hide the real failure.
///
/// `build_step_states` prepends this entry to the returned list so that the
/// non-canonical failed step renders above the canonical pipeline steps — it
/// represents a step that ran outside the canonical pipeline and the real
/// cause of the failure should be the first thing the user sees.
///
/// Returns `None` if `failed_at_step` is absent or matches a known step.
pub fn extra_failed_state_for_unknown_step(
    failed_at_step: Option<&str>,
    current_step_started_at: Option<DateTime<Utc>>,
    job_error_message: Option<&str>,
) -> Option<StepState> {
    let name = failed_at_step?;
    if scrape_pipeline_step_names().contains(&name) {
        return None;
    }
    Some(StepState {
        name: name.to_string(),
        status: "failed".to_string(),
        started_at: current_step_started_at,
        finished_at: None,
        duration_ms: None,
        summary: None,
        error: job_error_message.map(|s| s.to_string()),
        has_output: false,
    })
}

/// Build the ordered list of step states for a job.
///
/// Reads all step outputs for the job, then walks canonical scrape steps in order
/// and emits a `StepState` per step derived from:
/// - the step's stored output (completed), or
/// - the job's `current_step` (running) / `failed_at_step` (failed), or
/// - nothing → pending.
///
/// For completed steps, `finished_at` comes from `step_outputs.created_at`
/// and `duration_ms` from `step_outputs.duration_ms` (written by
/// `DbOutputStore::save_output`). `started_at` is derived as
/// `finished_at - duration_ms` when duration is available.
///
/// If `failed_at_step` is not one of the canonical scrape steps, a synthetic failed
/// entry is prepended — these represent steps that run outside the canonical
/// scrape pipeline (e.g. `photo_extract` in photo-only imports).
pub async fn build_step_states(
    pool: &DbPool,
    job_id: Uuid,
    job_status: &str,
    current_step: Option<&str>,
    current_step_started_at: Option<DateTime<Utc>>,
    failed_at_step: Option<&str>,
    job_error_message: Option<&str>,
) -> Result<Vec<StepState>, ScrapeError> {
    // Only select the pre-computed `summary` column — NOT the full `output`
    // JSON. Loading `output` would pull megabytes per poll for `fetch_html` /
    // `extract_recipe` rows, and the status API only ever needed the short
    // summary. The expand-step endpoint still reads `output` on demand.
    let outputs: Vec<StepOutputRow> = run_scrape_db(pool, move |conn| {
        step_outputs::table
            .filter(step_outputs::scrape_job_id.eq(job_id))
            .order(step_outputs::created_at.asc())
            .select((
                step_outputs::step_name,
                step_outputs::created_at,
                step_outputs::duration_ms,
                step_outputs::summary,
                step_outputs::success,
                step_outputs::error,
            ))
            .load(conn)
            .map_err(|e| ScrapeError::Database(e.to_string()))
    })
    .await?;

    Ok(build_step_states_from_outputs(
        outputs,
        job_status,
        current_step,
        current_step_started_at,
        failed_at_step,
        job_error_message,
    ))
}

/// Pure helper: walk canonical scrape steps and emit a `StepState` per step from the
/// given `step_outputs` rows plus the job's live state. Separated from
/// `build_step_states` so it can be unit-tested without a DB.
fn build_step_states_from_outputs(
    outputs: Vec<StepOutputRow>,
    job_status: &str,
    current_step: Option<&str>,
    current_step_started_at: Option<DateTime<Utc>>,
    failed_at_step: Option<&str>,
    job_error_message: Option<&str>,
) -> Vec<StepState> {
    // Latest row per step name (in case a step was re-run on retry).
    // Value layout matches `StepOutputRow` minus the leading name.
    type StoredRow = (
        DateTime<Utc>,
        Option<i64>,
        Option<String>,
        bool,
        Option<String>,
    );
    let mut by_name: std::collections::HashMap<String, StoredRow> =
        std::collections::HashMap::new();
    for (name, created_at, duration_ms, summary, success, error) in outputs {
        match by_name.get(&name) {
            Some((existing_at, _, _, _, _)) if *existing_at >= created_at => {}
            _ => {
                by_name.insert(name, (created_at, duration_ms, summary, success, error));
            }
        }
    }

    let terminal = matches!(job_status, "completed" | "failed");
    let pipeline_steps = scrape_pipeline_step_names();
    let mut states = Vec::with_capacity(pipeline_steps.len());

    for step_name in pipeline_steps {
        let name = step_name.to_string();
        // Branch order matters:
        //   1. failed_at_step — a failed step must render as "failed" even if
        //      execute_step_with_tracing persisted a partial output row for
        //      debugging.
        //   2. current_step (when not terminal) — on a retry, a previously
        //      failed step has a stale output row from the prior attempt; the
        //      live current_step must win so the retry renders as "running"
        //      instead of the stale "completed".
        //   3. Output row present with `success = false` → "failed" (this is
        //      the continues-on-failure enrichment case: the overall job
        //      completed, but the step itself failed and the per-step error
        //      should be surfaced).
        //   4. Output row present → "completed".
        //   5. Otherwise → "pending".
        if failed_at_step == Some(name.as_str()) {
            let stored = by_name.get(&name);
            let finished_at = stored.map(|(created_at, _, _, _, _)| *created_at);
            let duration_ms = stored.and_then(|(_, d, _, _, _)| *d);
            let started_at = match (finished_at, duration_ms) {
                (Some(finished), Some(d)) => chrono::Duration::try_milliseconds(d)
                    .map(|dur| finished - dur)
                    .or(current_step_started_at),
                _ => current_step_started_at,
            };
            states.push(StepState {
                name: name.clone(),
                status: "failed".to_string(),
                started_at,
                finished_at,
                duration_ms,
                summary: None,
                error: job_error_message.map(|s| s.to_string()),
                has_output: stored.is_some(),
            });
        } else if !terminal && current_step == Some(name.as_str()) {
            states.push(StepState {
                name: name.clone(),
                status: "running".to_string(),
                started_at: current_step_started_at,
                finished_at: None,
                duration_ms: None,
                summary: None,
                error: None,
                has_output: false,
            });
        } else if let Some((created_at, duration_ms, summary, success, step_error)) =
            by_name.get(&name)
        {
            let finished_at = *created_at;
            let started_at = duration_ms
                .and_then(chrono::Duration::try_milliseconds)
                .map(|d| finished_at - d);
            let (status, summary_val, error_val) = if *success {
                ("completed".to_string(), summary.clone(), None)
            } else {
                // continues_on_failure enrichment step failed: surface the
                // per-step error even though the overall job completed.
                ("failed".to_string(), None, step_error.clone())
            };
            states.push(StepState {
                name: name.clone(),
                status,
                started_at,
                finished_at: Some(finished_at),
                duration_ms: *duration_ms,
                summary: summary_val,
                error: error_val,
                has_output: true,
            });
        } else {
            states.push(StepState {
                name: name.clone(),
                status: "pending".to_string(),
                started_at: None,
                finished_at: None,
                duration_ms: None,
                summary: None,
                error: None,
                has_output: false,
            });
        }
    }

    if let Some(extra) = extra_failed_state_for_unknown_step(
        failed_at_step,
        current_step_started_at,
        job_error_message,
    ) {
        states.insert(0, extra);
    }

    states
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn step_summary_fetch_html() {
        // FetchHtmlStep writes `{ "html": "<...>" }`; bytes come from the string length.
        let output = json!({ "html": "hello world" });
        assert_eq!(
            step_summary("fetch_html", &output).as_deref(),
            Some("11 bytes fetched")
        );
    }

    #[test]
    fn step_summary_extract_recipe() {
        let output = json!({ "raw_recipe": { "title": "Chocolate Cake" } });
        assert_eq!(
            step_summary("extract_recipe", &output).as_deref(),
            Some("extracted \"Chocolate Cake\"")
        );
    }

    #[test]
    fn step_summary_fetch_images() {
        // 3 succeeded + 1 failed = 4 requested
        let output = json!({
            "photo_ids": ["11111111-1111-1111-1111-111111111111", "22222222-2222-2222-2222-222222222222", "33333333-3333-3333-3333-333333333333"],
            "failed_urls": [{ "url": "https://example.com/x.jpg", "error": "404" }]
        });
        assert_eq!(
            step_summary("fetch_images", &output).as_deref(),
            Some("3/4 images")
        );
    }

    #[test]
    fn step_summary_parse_ingredients() {
        let output = json!({
            "ingredients": [
                { "item": "flour", "measurements": [], "note": null, "raw": null, "section": null },
                { "item": "sugar", "measurements": [], "note": null, "raw": null, "section": null }
            ]
        });
        assert_eq!(
            step_summary("parse_ingredients", &output).as_deref(),
            Some("2 ingredients parsed")
        );
    }

    #[test]
    fn step_summary_save_recipe() {
        let output = json!({ "recipe_id": "abc-123" });
        assert_eq!(
            step_summary("save_recipe", &output).as_deref(),
            Some("saved recipe abc-123")
        );
    }

    #[test]
    fn step_summary_enrich_auto_tag() {
        let output = json!({
            "suggested_tags": ["dessert", "quick"],
            "cached": false
        });
        assert_eq!(
            step_summary("enrich_auto_tag", &output).as_deref(),
            Some("2 tags suggested")
        );
    }

    #[test]
    fn step_summary_apply_auto_tags() {
        let output = json!({ "tags_applied": ["dessert"], "new_version_id": "v1" });
        assert_eq!(
            step_summary("apply_auto_tags", &output).as_deref(),
            Some("1 tags applied")
        );
    }

    #[test]
    fn step_summary_unknown_step_is_none() {
        assert_eq!(step_summary("not_a_step", &json!({})), None);
    }

    #[test]
    fn step_summary_missing_field_is_none() {
        // fetch_html needs the "html" field to compute bytes; without it, no summary.
        assert_eq!(step_summary("fetch_html", &json!({})), None);
    }

    #[test]
    fn pipeline_steps_matches_build_registry_order() {
        // Sanity check: the canonical list must match what `build_registry`
        // actually registers, in order.
        assert_eq!(
            scrape_pipeline_step_names(),
            vec![
                "fetch_html",
                "extract_recipe",
                "fetch_images",
                "parse_ingredients",
                "save_recipe",
                "enrich_normalize_title",
                "apply_normalized_title",
                "enrich_generate_description",
                "apply_generated_description",
                "enrich_auto_tag",
                "apply_auto_tags",
            ]
        );
    }

    #[test]
    fn extra_failed_state_for_unknown_step_returns_none_when_missing() {
        // No failed step → no synthetic state.
        assert!(extra_failed_state_for_unknown_step(None, None, None).is_none());
    }

    #[test]
    fn extra_failed_state_for_unknown_step_returns_none_for_canonical_step() {
        // A canonical step is already handled by the main loop; don't
        // duplicate it with a synthetic entry.
        let out =
            extra_failed_state_for_unknown_step(Some("extract_recipe"), None, Some("bad recipe"));
        assert!(out.is_none());
    }

    #[test]
    fn build_step_states_synthetic_entry_for_non_canonical_failed_step() {
        // `photo_extract` is the photo-only import path's step name; without
        // the fallback the status list would render canonical steps as
        // "pending" and silently hide the real failure.
        let started = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let state = extra_failed_state_for_unknown_step(
            Some("photo_extract"),
            Some(started),
            Some("photo extraction failed"),
        )
        .expect("expected synthetic state for non-canonical step");
        assert_eq!(state.name, "photo_extract");
        assert_eq!(state.status, "failed");
        assert_eq!(state.started_at, Some(started));
        assert_eq!(state.finished_at, None);
        assert_eq!(state.duration_ms, None);
        assert_eq!(state.summary, None);
        assert_eq!(state.error.as_deref(), Some("photo extraction failed"));
        assert!(!state.has_output);
    }

    #[test]
    fn step_state_serializes_compactly() {
        // Smoke test that StepState serializes with the expected field names
        // and skips None optionals — this is the shape the API will expose.
        let state = StepState {
            name: "fetch_html".to_string(),
            status: "pending".to_string(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            summary: None,
            error: None,
            has_output: false,
        };
        let v = serde_json::to_value(&state).expect("serialize");
        assert_eq!(v["name"], "fetch_html");
        assert_eq!(v["status"], "pending");
        assert_eq!(v["has_output"], false);
        assert!(v.get("started_at").is_none());
        assert!(v.get("summary").is_none());
    }

    #[test]
    fn failed_step_with_output_row_is_rendered_as_failed() {
        // `execute_step_with_tracing` persists a step_outputs row even when a
        // step fails (for debugging). `build_step_states` must render that
        // step as "failed" — not "completed" — when `failed_at_step` names
        // it. `has_output` and `finished_at` still come from the stored row
        // so the user can expand the row to see the partial output.
        let finished = DateTime::parse_from_rfc3339("2025-01-01T00:00:05Z")
            .unwrap()
            .with_timezone(&Utc);
        let outputs = vec![(
            "save_recipe".to_string(),
            finished,
            Some(1000),
            None,
            true,
            None,
        )];
        let states = build_step_states_from_outputs(
            outputs,
            "failed",
            None,
            None,
            Some("save_recipe"),
            Some("save failed: no photo ids"),
        );
        let save = states
            .iter()
            .find(|s| s.name == "save_recipe")
            .expect("save_recipe state present");
        assert_eq!(save.status, "failed");
        assert!(save.has_output);
        assert_eq!(save.finished_at, Some(finished));
        assert_eq!(save.duration_ms, Some(1000));
        assert_eq!(save.error.as_deref(), Some("save failed: no photo ids"));
    }

    #[test]
    fn continues_on_failure_enrichment_renders_as_failed() {
        // Auto-applied enrichment steps can fail after save_recipe; the status
        // page must still render that step as "failed" with the stored error.
        let finished = DateTime::parse_from_rfc3339("2025-01-01T00:00:05Z")
            .unwrap()
            .with_timezone(&Utc);
        let outputs = vec![(
            "apply_auto_tags".to_string(),
            finished,
            Some(500),
            None,
            false,
            Some("boom".to_string()),
        )];
        let states = build_step_states_from_outputs(outputs, "completed", None, None, None, None);
        let enrich = states
            .iter()
            .find(|s| s.name == "apply_auto_tags")
            .expect("apply_auto_tags state present");
        assert_eq!(enrich.status, "failed");
        assert_eq!(enrich.error.as_deref(), Some("boom"));
        assert_eq!(enrich.finished_at, Some(finished));
        assert_eq!(enrich.duration_ms, Some(500));
        assert!(enrich.has_output);
    }

    #[test]
    fn current_running_step_overrides_stale_output_row() {
        // Scenario: a previous attempt for "fetch_html" left an output row
        // behind, and the retry is now re-running that step. The status
        // should be "running", not "completed" — the live current_step must
        // win over the stale output row.
        let stale_finished = DateTime::parse_from_rfc3339("2025-01-01T00:00:05Z")
            .unwrap()
            .with_timezone(&Utc);
        let retry_started = DateTime::parse_from_rfc3339("2025-01-01T00:01:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let outputs = vec![(
            "fetch_html".to_string(),
            stale_finished,
            Some(123),
            Some("3 bytes fetched".to_string()),
            true,
            None,
        )];

        let states = build_step_states_from_outputs(
            outputs,
            "scraping",
            Some("fetch_html"),
            Some(retry_started),
            None,
            None,
        );

        let fetch_html = states
            .iter()
            .find(|s| s.name == "fetch_html")
            .expect("fetch_html state present");
        assert_eq!(
            fetch_html.status, "running",
            "current_step should override stale output row on retry"
        );
        assert_eq!(fetch_html.started_at, Some(retry_started));
        assert_eq!(fetch_html.finished_at, None);
        assert_eq!(fetch_html.duration_ms, None);
        assert!(!fetch_html.has_output);
    }
}
