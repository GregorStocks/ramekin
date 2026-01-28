mod recipe;

pub use recipe::import_recipe;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(recipe::import_recipe),
    components(schemas(recipe::ImportRecipeRequest, recipe::ImportRecipeResponse))
)]
pub struct ApiDoc;
