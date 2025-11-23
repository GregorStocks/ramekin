mod db;
mod models;
mod schema;

use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use diesel::prelude::*;
use models::Garbage;
use schema::garbage;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let pool = db::create_pool(&database_url);

    let app = Router::new()
        .route("/api/garbages", get(get_garbages))
        .with_state(Arc::new(pool));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    tracing::info!("Server listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn get_garbages(
    State(pool): State<Arc<db::DbPool>>,
) -> Json<Vec<String>> {
    let mut conn = pool.get().expect("Failed to get DB connection");

    let results = garbage::table
        .select(Garbage::as_select())
        .load(&mut conn)
        .expect("Error loading garbages");

    let names: Vec<String> = results.into_iter().map(|g| g.garbage_name).collect();

    Json(names)
}
