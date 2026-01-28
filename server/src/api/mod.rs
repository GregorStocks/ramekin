pub mod enrich;
pub mod import;
pub mod photos;
pub mod public;
pub mod recipes;
pub mod scrape;
pub mod tags;
pub mod testing;

use serde::Serialize;
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{OpenApi, ToSchema};

use crate::models::Ingredient;

/// Shared error response used by all endpoints
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

/// Generate the complete OpenAPI spec by merging all module specs
pub fn openapi() -> utoipa::openapi::OpenApi {
    // Base spec with shared components and security
    #[derive(OpenApi)]
    #[openapi(components(schemas(ErrorResponse, Ingredient)))]
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
        enrich::ApiDoc::openapi(),
        tags::ApiDoc::openapi(),
        import::ApiDoc::openapi(),
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
