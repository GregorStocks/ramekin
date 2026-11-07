mod allowlist;
mod job_kind;
mod job_status;
mod jobs;
pub mod output_store;
mod photo_import;
pub mod plan;
mod runner;
pub mod status;
pub mod steps;

pub use allowlist::is_host_allowed;
pub use job_kind::JobKind;
pub use job_status::JobStatus;
pub use jobs::{
    create_edit_job, create_import_job, create_job, create_job_with_html, create_normalize_job,
    create_photo_rescrape_job, create_text_import_job, find_import_jobs_by_key, get_job,
    stored_source, NormalizeStart, StoredSource,
};
pub use photo_import::{create_pending_photo_job, spawn_photo_import_job};
pub use runner::{
    resume_interrupted_jobs, retry_job, retry_refusal, spawn_import_job, spawn_scrape_job,
};
pub use steps::EditRequest;

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
