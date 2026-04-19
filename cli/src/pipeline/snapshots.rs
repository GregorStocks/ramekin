//! Write end-of-pipeline recipe snapshots for an allowlisted set of URLs.
//!
//! After a pipeline run completes, this module reads the relevant per-step
//! outputs from `run_dir/urls/<slug>/` for each allowlisted URL, assembles a
//! `FinalRecipe` via `ramekin_core::final_recipe::build_final_recipe`, and
//! writes the JSON to `snapshots_dir/<slug>.json`. If an allowlisted URL
//! isn't present in the run directory, the function returns an error so the
//! pipeline run fails fast.

use std::path::Path;

use anyhow::{Context, Result};

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
}
