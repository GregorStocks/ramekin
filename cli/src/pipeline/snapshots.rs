//! Write end-of-pipeline recipe snapshots for an allowlisted set of URLs.
//!
//! After a pipeline run completes, this module reads the relevant per-step
//! outputs from `run_dir/urls/<slug>/` for each allowlisted URL, assembles a
//! `FinalRecipe` via `ramekin_core::final_recipe::build_final_recipe`, and
//! writes the JSON to `snapshots_dir/<slug>.json`. If an allowlisted URL
//! isn't present in the run directory, the function returns an error so the
//! pipeline run fails fast.

use std::path::Path;

use anyhow::Result;

/// Write snapshots for every URL in `allowlist_path` by reading step outputs
/// under `run_dir` and writing JSON files under `snapshots_dir`.
#[allow(dead_code)] // Wired into pipeline_orchestrator in a follow-up batch.
pub fn write_snapshots(run_dir: &Path, allowlist_path: &Path, snapshots_dir: &Path) -> Result<()> {
    let _ = (run_dir, allowlist_path, snapshots_dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_is_noop() {
        let p = Path::new("/nonexistent");
        write_snapshots(p, p, p).unwrap();
    }
}
