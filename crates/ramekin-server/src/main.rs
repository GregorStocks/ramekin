mod api;
mod db;
mod models;
mod schema;

use api::{paths, GarbagesResponse};
use axum::{extract::State, routing::get, Json};
use diesel::prelude::*;
use models::Garbage;
use schema::garbage;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(paths(get_garbages), components(schemas(GarbagesResponse)))]
struct ApiDoc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = db::create_pool(&database_url);

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .route(paths::GARBAGES, get(get_garbages))
        .split_for_parts();

    // Serve OpenAPI spec as JSON and Swagger UI
    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api);

    let app = router.merge(swagger_ui).with_state(Arc::new(pool));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    tracing::info!("Swagger UI available at http://localhost:3000/swagger-ui/");
    tracing::info!("OpenAPI spec available at http://localhost:3000/api-docs/openapi.json");

    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/api/garbages",
    responses(
        (status = 200, description = "List of all garbages", body = GarbagesResponse)
    )
)]
async fn get_garbages(State(pool): State<Arc<db::DbPool>>) -> Json<GarbagesResponse> {
    let mut conn = pool.get().expect("Failed to get DB connection");

    let results = garbage::table
        .select(Garbage::as_select())
        .load(&mut conn)
        .expect("Error loading garbages");

    let garbages: Vec<String> = results.into_iter().map(|g| g.garbage_name).collect();

    Json(GarbagesResponse { garbages })
}
