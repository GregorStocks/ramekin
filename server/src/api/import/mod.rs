mod photos;
mod recipe;
pub(crate) mod text;

pub use photos::import_from_photos;
pub use recipe::{import_recipe, lookup_import_jobs};
pub use text::prepare_text_recipe;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        recipe::import_recipe,
        recipe::lookup_import_jobs,
        photos::import_from_photos,
        text::prepare_text_recipe
    ),
    components(schemas(
        recipe::ImportRecipeRequest,
        recipe::ImportRecipeResponse,
        recipe::LookupImportJobsRequest,
        recipe::ImportJobKey,
        recipe::LookupImportJobsResponse,
        photos::ImportFromPhotosRequest,
        photos::ImportFromPhotosResponse,
        text::PrepareTextRecipeRequest,
        text::PrepareTextRecipeResponse,
    ))
)]
pub struct ApiDoc;
