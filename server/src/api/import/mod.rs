mod photos;
mod recipe;
mod text;

pub use photos::import_from_photos;
pub use recipe::{import_recipe, lookup_import_jobs};
pub use text::import_from_text;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        recipe::import_recipe,
        recipe::lookup_import_jobs,
        photos::import_from_photos,
        text::import_from_text
    ),
    components(schemas(
        recipe::ImportRecipeRequest,
        recipe::ImportRecipeResponse,
        recipe::LookupImportJobsRequest,
        recipe::ImportJobKey,
        recipe::LookupImportJobsResponse,
        photos::ImportFromPhotosRequest,
        photos::ImportFromPhotosResponse,
        text::ImportFromTextRequest,
        text::ImportFromTextResponse,
    ))
)]
pub struct ApiDoc;
