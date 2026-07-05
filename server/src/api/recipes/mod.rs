pub mod create;
pub mod delete;
pub mod export;
pub mod generate_description;
pub mod generate_photo;
pub mod get;
pub mod list;
pub mod normalize_title;
pub(crate) mod read;
pub mod rescrape;
pub mod rescrape_photo;
pub mod sync;
pub mod update;
pub mod versions;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/recipes endpoints (mounted at /api/recipes)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_recipes).post(create::create_recipe))
        .route("/export", get(export::export_all_recipes))
        .route("/sync", get(sync::sync_recipes))
        .route(
            "/{id}",
            get(get::get_recipe)
                .put(update::update_recipe)
                .delete(delete::delete_recipe),
        )
        .route("/{id}/export", get(export::export_recipe))
        .route("/{id}/versions", get(versions::list_versions))
        .route("/{id}/rescrape", post(rescrape::rescrape))
        .route("/{id}/rescrape-photo", post(rescrape_photo::rescrape_photo))
        .route("/{id}/generate-photo", post(generate_photo::generate_photo))
        .route(
            "/{id}/normalize-title",
            post(normalize_title::normalize_title),
        )
        .route(
            "/{id}/generate-description",
            post(generate_description::generate_description),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(
        create::create_recipe,
        list::list_recipes,
        sync::sync_recipes,
        get::get_recipe,
        update::update_recipe,
        delete::delete_recipe,
        export::export_recipe,
        export::export_all_recipes,
        versions::list_versions,
        rescrape::rescrape,
        rescrape_photo::rescrape_photo,
        generate_photo::generate_photo,
        normalize_title::normalize_title,
        generate_description::generate_description,
    ),
    components(schemas(
        create::CreateRecipeRequest,
        create::CreateRecipeResponse,
        list::ListRecipesResponse,
        list::RecipeSummary,
        sync::SyncRecipesResponse,
        list::SortBy,
        list::Direction,
        get::RecipeResponse,
        update::UpdateRecipeRequest,
        versions::VersionListResponse,
        versions::VersionSummary,
        rescrape::RescrapeResponse,
        generate_photo::GeneratePhotoResponse,
        normalize_title::NormalizeTitleResponse,
        generate_description::GenerateDescriptionResponse,
    ))
)]
pub struct ApiDoc;
