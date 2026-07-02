pub mod client_logs;
pub mod enrich;
pub mod error;
pub mod import;
pub mod meal_plans;
pub mod photos;
pub mod public;
pub mod recipes;
pub mod scrape;
pub mod shopping_list;
pub mod tags;
pub mod testing;
pub mod users;

use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::OpenApi;

use crate::models::Ingredient;

pub use error::{ApiError, ErrorCode, ErrorResponse};

/// Generate the complete OpenAPI spec by merging all module specs
pub fn openapi() -> utoipa::openapi::OpenApi {
    // Base spec with shared components and security
    #[derive(OpenApi)]
    #[openapi(components(schemas(ErrorResponse, ErrorCode, Ingredient)))]
    struct BaseApi;

    let mut spec = BaseApi::openapi();

    // Add security scheme
    if let Some(components) = spec.components.as_mut() {
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
    }

    // Merge in each module's spec
    let modules: Vec<utoipa::openapi::OpenApi> = vec![
        public::ApiDoc::openapi(),
        testing::ApiDoc::openapi(),
        photos::ApiDoc::openapi(),
        recipes::ApiDoc::openapi(),
        scrape::ApiDoc::openapi(),
        client_logs::ApiDoc::openapi(),
        enrich::ApiDoc::openapi(),
        tags::ApiDoc::openapi(),
        import::ApiDoc::openapi(),
        meal_plans::ApiDoc::openapi(),
        shopping_list::ApiDoc::openapi(),
        users::ApiDoc::openapi(),
    ];

    for module_spec in modules {
        // Merge paths
        spec.paths.paths.extend(module_spec.paths.paths);

        // Merge components (schemas)
        if let Some(module_components) = module_spec.components {
            if let Some(spec_components) = spec.components.as_mut() {
                spec_components.schemas.extend(module_components.schemas);
            }
        }
    }

    spec
}
