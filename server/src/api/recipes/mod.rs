pub mod create;
pub mod delete;
pub mod get;
pub mod list;
pub mod tags;
pub mod update;

use crate::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Returns the router for /api/recipes endpoints (mounted at /api/recipes)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list::list_recipes).post(create::create_recipe))
        .route("/tags", get(tags::list_tags))
        .route(
            "/{id}",
            get(get::get_recipe)
                .put(update::update_recipe)
                .delete(delete::delete_recipe),
        )
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
    ),
    components(schemas(
        create::CreateRecipeRequest,
        create::CreateRecipeResponse,
        list::ListRecipesResponse,
        list::RecipeSummary,
        list::OrderBy,
        list::Direction,
        get::RecipeResponse,
        update::UpdateRecipeRequest,
        tags::TagsResponse,
    ))
)]
pub struct ApiDoc;
