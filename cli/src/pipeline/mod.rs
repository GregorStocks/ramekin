//! CLI pipeline module.
//!
//! This module provides:
//! - Step runners for the orchestrator (run_fetch_html, run_all_steps, etc.)
//! - Staging utilities for manual HTML caching

mod runners;
mod staging;

// Re-export commonly used items from runners
pub use runners::{
    parse_pipeline_step, run_all_steps, run_enrich, run_extract_recipe, run_fetch_html,
    run_save_recipe, AllStepsResult, ExtractionStats, PipelineStep, StepResult,
};

// Re-export staging utilities
pub use staging::{clear_staging, ensure_staging_dir, find_staged_html, staging_dir};
