pub mod create;
pub mod delete;
pub mod export;
pub mod get;
pub mod list;
pub mod tags;
pub mod update;
pub mod versions;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/recipes endpoints (mounted at /api/recipes)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_recipes).post(create::create_recipe))
        .route("/tags", get(tags::list_tags))
        .route("/export", get(export::export_all_recipes))
        .route(
            "/{id}",
            get(get::get_recipe)
                .put(update::update_recipe)
                .delete(delete::delete_recipe),
        )
        .route("/{id}/export", get(export::export_recipe))
        .route("/{id}/versions", get(versions::list_versions))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        create::create_recipe,
        list::list_recipes,
        get::get_recipe,
        update::update_recipe,
        delete::delete_recipe,
        tags::list_tags,
        export::export_recipe,
        export::export_all_recipes,
        versions::list_versions,
    ),
    components(schemas(
        create::CreateRecipeRequest,
        create::CreateRecipeResponse,
        list::ListRecipesResponse,
        list::RecipeSummary,
        list::SortBy,
        list::Direction,
        get::RecipeResponse,
        update::UpdateRecipeRequest,
        tags::TagsResponse,
        versions::VersionListResponse,
        versions::VersionSummary,
    ))
)]
pub struct ApiDoc;
