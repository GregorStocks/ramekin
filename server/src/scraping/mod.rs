mod allowlist;
mod jobs;
mod output_store;
mod photo_import;
mod runner;
pub mod status;
pub mod steps;

pub use allowlist::is_host_allowed;
pub use jobs::{
    create_import_job, create_job, create_job_with_html, create_photo_rescrape_job,
    create_rescrape_job, get_job,
};
pub use photo_import::{create_pending_photo_job, spawn_photo_import_job};
pub use runner::{retry_job, spawn_import_job, spawn_scrape_job};

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

    #[error("AI configuration error: {0}")]
    AiConfig(#[from] ramekin_core::ai::AiError),
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

/// Job statuses
pub const STATUS_SCRAPING: &str = "scraping";
pub const STATUS_PARSING: &str = "parsing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";
