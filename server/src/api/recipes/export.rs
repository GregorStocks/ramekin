use super::paprika::export_recipe_to_paprikarecipe;
use super::read::{fetch_current_recipe_with_version, fetch_current_recipes_with_versions};
use super::zip_stream::write_zip_stream;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use chrono::Utc;
use std::io;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/api/recipes/{id}/export",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 200, description = "Paprika recipe file (.paprikarecipe)", content_type = "application/gzip"),
        (status = 404, description = "Recipe not found", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn export_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    let exported = run_db(&pool, move |conn| {
        let recipe = match fetch_current_recipe_with_version(conn, user_id, id) {
            Ok(r) => r,
            Err(diesel::NotFound) => return Err(ApiError::not_found("Recipe not found")),
            Err(_) => return Err(ApiError::internal("Failed to fetch recipe")),
        };

        export_recipe_to_paprikarecipe(conn, user_id, &recipe).map_err(|e| {
            tracing::error!("Failed to export recipe: {}", e);
            ApiError::internal("Failed to export recipe")
        })
    })
    .await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", exported.filename),
        )
        .body(Body::from(exported.data))
        .unwrap())
}

#[utoipa::path(
    get,
    path = "/api/recipes/export",
    tag = "recipes",
    responses(
        (status = 200, description = "Paprika recipes archive (.paprikarecipes)", content_type = "application/zip"),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn export_all_recipes(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    // Fetch recipe metadata (no photo bytes) on a blocking thread so we can
    // still return a clean 500 if the DB is unhappy. Once we commit to the
    // streaming body below, errors can only truncate the response.
    let all_recipes = run_db(&pool, move |conn| {
        fetch_current_recipes_with_versions(conn, user_id).map_err(|e| {
            tracing::error!(error = %e, "failed to fetch recipes for export");
            ApiError::internal("Failed to fetch recipes")
        })
    })
    .await?;

    let recipe_count = all_recipes.len();
    let start = Instant::now();
    tracing::info!(
        recipe_count,
        %user_id,
        "starting paprikarecipes export stream"
    );

    // Small buffer: the ZipWriter writes in modest-sized chunks, so backpressure
    // keeps peak in-flight bytes bounded while still allowing overlap between
    // encoding and network send.
    let (tx, rx) = mpsc::channel::<Result<Bytes, io::Error>>(8);

    let pool_for_write = Arc::clone(&pool);
    tokio::task::spawn_blocking(move || {
        match write_zip_stream(&pool_for_write, user_id, &all_recipes, tx.clone()) {
            Ok(bytes_written) => {
                tracing::info!(
                    recipe_count,
                    bytes_written,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "paprikarecipes export stream complete"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "paprikarecipes export stream aborted"
                );
                let _ = tx.blocking_send(Err(e));
            }
        }
    });

    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let filename = format!("recipes-{}.paprikarecipes", timestamp);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap())
}
