mod api;
mod auth;
mod db;
mod models;
mod schema;

use api::photos::get as photos_get;
use api::photos::list as photos_list;
use api::photos::upload;
use api::public::auth::{login, signup};
use api::public::testing::unauthed_ping;
use api::recipes::create as recipes_create;
use api::recipes::delete as recipes_delete;
use api::recipes::get as recipes_get;
use api::recipes::list as recipes_list;
use api::recipes::update as recipes_update;
use api::testing::ping;
use api::ErrorResponse;
use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use models::Ingredient;
use std::env;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::Span;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        unauthed_ping::unauthed_ping,
        signup::signup,
        login::login,
        ping::ping,
        upload::upload,
        photos_get::get_photo,
        photos_list::list_photos,
        recipes_create::create_recipe,
        recipes_list::list_recipes,
        recipes_get::get_recipe,
        recipes_update::update_recipe,
        recipes_delete::delete_recipe
    ),
    components(schemas(
        unauthed_ping::UnauthedPingResponse,
        signup::SignupRequest,
        signup::SignupResponse,
        login::LoginRequest,
        login::LoginResponse,
        ping::PingResponse,
        upload::UploadPhotoRequest,
        upload::UploadPhotoResponse,
        photos_list::ListPhotosResponse,
        photos_list::PhotoSummary,
        recipes_create::CreateRecipeRequest,
        recipes_create::CreateRecipeResponse,
        recipes_list::ListRecipesResponse,
        recipes_list::RecipeSummary,
        recipes_get::RecipeResponse,
        recipes_update::UpdateRecipeRequest,
        Ingredient,
        ErrorResponse
    )),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

#[tokio::main]
async fn main() {
    // Check for --openapi flag to dump spec and exit
    if env::args().any(|arg| arg == "--openapi") {
        let spec = ApiDoc::openapi().to_pretty_json().unwrap();
        println!("{}", spec);
        return;
    }

    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = Arc::new(db::create_pool(&database_url));

    // Public routes - no authentication required
    // Add new public routes here (health checks, login, signup, etc.)
    let (public_router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .route(unauthed_ping::PATH, get(unauthed_ping::unauthed_ping))
        .route(signup::PATH, post(signup::signup))
        .route(login::PATH, post(login::login))
        .split_for_parts();

    // Protected routes - authentication required by default
    // ADD NEW ROUTES HERE - they will automatically require auth
    let protected_router = Router::new()
        .route(ping::PATH, get(ping::ping))
        .route(
            photos_list::PATH,
            get(photos_list::list_photos).post(upload::upload),
        )
        .route(photos_get::PATH, get(photos_get::get_photo))
        .route(
            recipes_list::PATH,
            get(recipes_list::list_recipes).post(recipes_create::create_recipe),
        )
        .route(
            recipes_get::PATH,
            get(recipes_get::get_recipe)
                .put(recipes_update::update_recipe)
                .delete(recipes_delete::delete_recipe),
        )
        .layer(middleware::from_fn_with_state(
            pool.clone(),
            auth::require_auth,
        ));

    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi);

    let app = public_router
        .merge(protected_router)
        .merge(swagger_ui)
        .with_state(pool)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str)
                        .unwrap_or(request.uri().path());
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        path = %matched_path,
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {})
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &Span| {
                        tracing::info!(
                            status = %response.status().as_u16(),
                            latency_ms = %latency.as_millis(),
                            "request completed"
                        );
                    },
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: std::time::Duration,
                     _span: &Span| {
                        tracing::error!(
                            error = %error,
                            latency_ms = %latency.as_millis(),
                            "request failed"
                        );
                    },
                ),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    tracing::info!("Swagger UI available at http://localhost:3000/swagger-ui/");
    tracing::info!("OpenAPI spec available at http://localhost:3000/api-docs/openapi.json");
    tracing::info!("Hot reload is enabled!");

    axum::serve(listener, app).await.unwrap();
}
