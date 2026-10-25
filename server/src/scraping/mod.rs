mod allowlist;
mod jobs;
pub mod output_store;
mod photo_import;
pub mod plan;
mod runner;
pub mod status;
pub mod steps;

pub use allowlist::is_host_allowed;
pub use jobs::{
    create_import_job, create_job, create_job_with_html, create_normalize_job,
    create_photo_rescrape_job, find_import_jobs_by_key, get_job, saved_recipe_id, stored_source,
    NormalizeStart, StoredSource,
};
pub use photo_import::{create_pending_photo_job, spawn_photo_import_job};
pub use runner::{resume_interrupted_jobs, retry_job, spawn_import_job, spawn_scrape_job};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScrapeError {
    #[error("Fetch error: {0}")]
    Fetch(#[from] ramekin_core::FetchError),

    #[error("Parse error: {0}")]
    Parse(#[from] ramekin_core::ExtractError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Job not found")]
    JobNotFound,

    #[error("Invalid job state: {0}")]
    InvalidState(String),

    #[error("URL host not allowed: {0}")]
    HostNotAllowed(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,

    #[error("Idempotency key already used")]
    DuplicateIdempotencyKey,

    #[error("AI configuration error: {0}")]
    AiConfig(#[from] ramekin_core::ai::AiError),
}

impl From<diesel::result::Error> for ScrapeError {
    fn from(e: diesel::result::Error) -> Self {
        ScrapeError::Database(e.to_string())
    }
}

/// Run scraping-side blocking Diesel work off the runtime threads.
///
/// Wraps [`crate::db::run_blocking`], turning a pool checkout failure into
/// [`ScrapeError::Database`] so callers can use `?` on the result.
pub(crate) async fn run_scrape_db<T, F>(pool: &crate::db::DbPool, f: F) -> Result<T, ScrapeError>
where
    F: FnOnce(&mut crate::db::DbConn) -> Result<T, ScrapeError> + Send + 'static,
    T: Send + 'static,
{
    crate::db::run_blocking(pool, f)
        .await
        .map_err(|e| ScrapeError::Database(e.to_string()))?
}

/// [`run_scrape_db`] for a closure that writes: it runs inside `db::write`.
pub(crate) async fn run_scrape_db_write<T, F>(
    pool: &crate::db::DbPool,
    f: F,
) -> Result<T, ScrapeError>
where
    F: FnOnce(&mut diesel::SqliteConnection) -> Result<T, ScrapeError> + Send + 'static,
    T: Send + 'static,
{
    crate::db::run_blocking_write(pool, f)
        .await
        .map_err(|e| ScrapeError::Database(e.to_string()))?
}

/// Job statuses
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_SCRAPING: &str = "scraping";
pub const STATUS_PARSING: &str = "parsing";
/// Normalize ops are running. For an import or scrape the recipe is already
/// saved and visible, and they may still update it; a manual normalize job
/// then proposes a version.
pub const STATUS_NORMALIZING: &str = "normalizing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";
