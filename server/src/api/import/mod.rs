mod photos;
mod recipe;
pub(crate) mod text;

pub use photos::import_from_photos;
pub use recipe::import_recipe;
pub use text::prepare_text_recipe;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        recipe::import_recipe,
        photos::import_from_photos,
        text::prepare_text_recipe
    ),
    components(schemas(
        recipe::ImportRecipeRequest,
        recipe::ImportRecipeResponse,
        photos::ImportFromPhotosRequest,
        photos::ImportFromPhotosResponse,
        text::PrepareTextRecipeRequest,
        text::PrepareTextRecipeResponse,
    ))
)]
pub struct ApiDoc;
