pub mod create;
pub mod get;
pub mod retry;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/scrape endpoints (mounted at /api/scrape)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create::create_scrape))
        .route("/{id}", get(get::get_scrape))
        .route("/{id}/retry", post(retry::retry_scrape))
}

#[derive(OpenApi)]
#[openapi(
    paths(create::create_scrape, get::get_scrape, retry::retry_scrape,),
    components(schemas(
        create::CreateScrapeRequest,
        create::CreateScrapeResponse,
        get::ScrapeJobResponse,
        retry::RetryScrapeResponse,
    ))
)]
pub struct ApiDoc;
