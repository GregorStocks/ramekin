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
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::types::RawRecipe;

/// Write snapshots for every URL in `allowlist_path` by reading step outputs
/// under `run_dir` and writing JSON files under `snapshots_dir`.
#[allow(dead_code)] // Wired into pipeline_orchestrator in a follow-up batch.
pub fn write_snapshots(run_dir: &Path, allowlist_path: &Path, snapshots_dir: &Path) -> Result<()> {
    let _ = (run_dir, allowlist_path, snapshots_dir);
    Ok(())
}

#[allow(dead_code)] // Used via write_snapshots once wired into the orchestrator.
fn read_allowlist(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read allowlist: {}", path.display()))?;
    let urls: Vec<String> = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse allowlist JSON: {}", path.display()))?;
    Ok(urls)
}

#[allow(dead_code)] // Used via write_snapshots once wired into the orchestrator.
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

#[allow(dead_code)] // Used via write_snapshots once wired into the orchestrator.
fn assemble_snapshot(run_dir: &Path, url_slug: &str) -> Result<FinalRecipe> {
    let extract = read_step_output(run_dir, url_slug, "extract_recipe")?.ok_or_else(|| {
        anyhow!(
            "No extract_recipe output for URL slug {url_slug} in {}. \
             Either the URL wasn't in the pipeline run or the pipeline didn't reach extract_recipe.",
            run_dir.display()
        )
    })?;

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

    let applied_tags: Option<Vec<String>> = read_step_output(run_dir, url_slug, "apply_auto_tags")?
        .and_then(|v| v.get("tags_applied").cloned())
        .map(serde_json::from_value)
        .transpose()
        .context("Failed to deserialize apply_auto_tags tags_applied")?;

    Ok(build_final_recipe(
        &raw_recipe,
        parsed_ingredients.as_deref(),
        suggested_tags.as_deref(),
        applied_tags.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn stub_is_noop() {
        let p = Path::new("/nonexistent");
        write_snapshots(p, p, p).unwrap();
    }

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

        let fr = assemble_snapshot(dir.path(), "example-com_r").unwrap();
        assert_eq!(fr.title, "Test");
        assert_eq!(fr.ingredients.len(), 2); // line-split fallback
        assert!(fr.applied_tags.is_none());
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
        write_step_output(
            dir.path(),
            slug,
            "apply_auto_tags",
            r#"{"tags_applied": ["dinner"]}"#,
        );

        let fr = assemble_snapshot(dir.path(), slug).unwrap();
        assert_eq!(fr.ingredients.len(), 1);
        assert_eq!(fr.ingredients[0].item, "flour");
        assert_eq!(
            fr.suggested_tags.as_deref(),
            Some(&["dinner".to_string(), "breakfast".to_string()][..]),
        );
        assert_eq!(
            fr.applied_tags.as_deref(),
            Some(&["dinner".to_string()][..]),
        );
    }

    #[test]
    fn errors_when_extract_recipe_output_missing() {
        let dir = TempDir::new().unwrap();
        let err = assemble_snapshot(dir.path(), "missing-slug").unwrap_err();
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
}
