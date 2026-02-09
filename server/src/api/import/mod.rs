mod photos;
mod recipe;

pub use photos::import_from_photos;
pub use recipe::import_recipe;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(recipe::import_recipe, photos::import_from_photos),
    components(schemas(
        recipe::ImportRecipeRequest,
        recipe::ImportRecipeResponse,
        photos::ImportFromPhotosRequest,
        photos::ImportFromPhotosResponse,
    ))
)]
pub struct ApiDoc;
