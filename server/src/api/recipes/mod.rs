pub mod create;
pub mod delete;
pub mod estimate_calories;
pub mod export;
pub mod generate_photo;
pub mod get;
pub mod list;
pub mod normalize;
pub(crate) mod paprika;
pub(crate) mod read;
pub mod rescrape_photo;
pub mod update;
pub mod versions;
pub(crate) mod zip_stream;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/recipes endpoints (mounted at /api/recipes)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_recipes).post(create::create_recipe))
        .route("/export", get(export::export_all_recipes))
        .route(
            "/estimate-calories",
            post(estimate_calories::estimate_calories),
        )
        .route("/normalize-ops", get(normalize::list_normalize_ops))
        .route("/normalize", post(normalize::normalize_recipes))
        .route(
            "/{id}",
            get(get::get_recipe)
                .put(update::update_recipe)
                .delete(delete::delete_recipe),
        )
        .route("/{id}/export", get(export::export_recipe))
        .route("/{id}/versions", get(versions::list_versions))
        .route("/{id}/rescrape-photo", post(rescrape_photo::rescrape_photo))
        .route("/{id}/generate-photo", post(generate_photo::generate_photo))
        .route("/{id}/normalize", post(normalize::normalize_recipe))
        .route(
            "/{id}/normalize/{job_id}",
            get(normalize::get_normalize_proposal),
        )
        .route(
            "/{id}/normalize/{job_id}/accept",
            post(normalize::accept_normalize_proposal),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(
        create::create_recipe,
        list::list_recipes,
        get::get_recipe,
        estimate_calories::estimate_calories,
        update::update_recipe,
        delete::delete_recipe,
        export::export_recipe,
        export::export_all_recipes,
        versions::list_versions,
        rescrape_photo::rescrape_photo,
        generate_photo::generate_photo,
        normalize::list_normalize_ops,
        normalize::normalize_recipe,
        normalize::normalize_recipes,
        normalize::get_normalize_proposal,
        normalize::accept_normalize_proposal,
    ),
    components(schemas(
        create::CreateRecipeRequest,
        create::CreateRecipeResponse,
        list::ListRecipesResponse,
        list::RecipeSummary,
        list::SortBy,
        list::Direction,
        get::RecipeResponse,
        estimate_calories::EstimateCaloriesRequest,
        estimate_calories::CalorieEstimateResponse,
        estimate_calories::CalorieRange,
        estimate_calories::CalorieStatus,
        estimate_calories::CalorieLine,
        update::UpdateRecipeRequest,
        versions::VersionListResponse,
        versions::VersionSummary,
        rescrape_photo::RescrapeResponse,
        generate_photo::GeneratePhotoResponse,
        normalize::NormalizeOpGroupResponse,
        normalize::NormalizeOpInfo,
        normalize::NormalizeOpsResponse,
        normalize::NormalizeRequest,
        normalize::BulkNormalizeRequest,
        normalize::NormalizeJobResponse,
        normalize::BulkNormalizeResult,
        normalize::BulkNormalizeResponse,
        normalize::NormalizeProposalResponse,
        normalize::AcceptNormalizeResponse,
        crate::normalize::NormalizedRecipe,
    ))
)]
pub struct ApiDoc;
