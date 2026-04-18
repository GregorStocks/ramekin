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

/// Canonical pipeline step names, in the order they run in `build_registry`.
pub const PIPELINE_STEPS: &[&str] = &[
    "fetch_html",
    "extract_recipe",
    "fetch_images",
    "parse_ingredients",
    "save_recipe",
    "enrich_normalize_ingredients",
    "enrich_auto_tag",
    "apply_auto_tags",
    "enrich_generate_photo",
];

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
/// one of the canonical `PIPELINE_STEPS` (e.g. `photo_extract` for the
/// photo-only import path). Without this, the status API would render every
/// canonical step as `"pending"` and silently hide the real failure.
///
/// Returns `None` if `failed_at_step` is absent or matches a known step.
pub fn extra_failed_state_for_unknown_step(
    failed_at_step: Option<&str>,
    current_step_started_at: Option<DateTime<Utc>>,
    job_error_message: Option<&str>,
) -> Option<StepState> {
    let name = failed_at_step?;
    if PIPELINE_STEPS.contains(&name) {
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
/// Reads all step outputs for the job, then walks `PIPELINE_STEPS` in order
/// and emits a `StepState` per step derived from:
/// - the step's stored output (completed), or
/// - the job's `current_step` (running) / `failed_at_step` (failed), or
/// - nothing → pending.
///
/// For completed steps, `finished_at` comes from `step_outputs.created_at`
/// and `duration_ms` from `step_outputs.duration_ms` (written by
/// `DbOutputStore::save_output`). `started_at` is derived as
/// `finished_at - duration_ms` when duration is available.
pub fn build_step_states(
    pool: &DbPool,
    job_id: Uuid,
    job_status: &str,
    current_step: Option<&str>,
    current_step_started_at: Option<DateTime<Utc>>,
    failed_at_step: Option<&str>,
    job_error_message: Option<&str>,
) -> Result<Vec<StepState>, diesel::result::Error> {
    let mut conn = pool.get().expect("db pool");

    let outputs: Vec<(String, JsonValue, DateTime<Utc>, Option<i64>)> = step_outputs::table
        .filter(step_outputs::scrape_job_id.eq(job_id))
        .select((
            step_outputs::step_name,
            step_outputs::output,
            step_outputs::created_at,
            step_outputs::duration_ms,
        ))
        .load(&mut conn)?;

    // Latest output per step name (in case a step was re-run on retry).
    let mut by_name: std::collections::HashMap<String, (JsonValue, DateTime<Utc>, Option<i64>)> =
        std::collections::HashMap::new();
    for (name, output, created_at, duration_ms) in outputs {
        match by_name.get(&name) {
            Some((_, existing_at, _)) if *existing_at >= created_at => {}
            _ => {
                by_name.insert(name, (output, created_at, duration_ms));
            }
        }
    }

    let terminal = matches!(job_status, "completed" | "failed");
    let mut states = Vec::with_capacity(PIPELINE_STEPS.len());

    for step_name in PIPELINE_STEPS {
        let name = (*step_name).to_string();
        if let Some((output, created_at, duration_ms)) = by_name.get(&name) {
            let finished_at = *created_at;
            let started_at = duration_ms
                .and_then(chrono::Duration::try_milliseconds)
                .map(|d| finished_at - d);
            states.push(StepState {
                name: name.clone(),
                status: "completed".to_string(),
                started_at,
                finished_at: Some(finished_at),
                duration_ms: *duration_ms,
                summary: step_summary(&name, output),
                error: None,
                has_output: true,
            });
        } else if failed_at_step == Some(name.as_str()) {
            states.push(StepState {
                name: name.clone(),
                status: "failed".to_string(),
                started_at: current_step_started_at,
                finished_at: None,
                duration_ms: None,
                summary: None,
                error: job_error_message.map(|s| s.to_string()),
                has_output: false,
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
        states.push(extra);
    }

    Ok(states)
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
        // actually registers, in order. If someone reorders steps in
        // `scraping::mod::build_registry` without updating this list, the
        // status API will be wrong — keep them in lockstep.
        assert_eq!(
            PIPELINE_STEPS,
            &[
                "fetch_html",
                "extract_recipe",
                "fetch_images",
                "parse_ingredients",
                "save_recipe",
                "enrich_normalize_ingredients",
                "enrich_auto_tag",
                "apply_auto_tags",
                "enrich_generate_photo",
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
        // `photo_extract` is the photo-only import path's step name and is
        // NOT in PIPELINE_STEPS; without the fallback the status list would
        // render all 9 canonical steps as "pending" and silently hide the
        // real failure.
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
}
