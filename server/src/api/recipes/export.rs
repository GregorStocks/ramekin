use super::paprika::export_recipe_to_paprikarecipe;
use super::read::{fetch_current_recipe_with_version, fetch_current_recipes_with_versions};
use super::zip_stream::write_zip_stream;
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let recipe = match fetch_current_recipe_with_version(&mut conn, user.id, id) {
        Ok(r) => r,
        Err(diesel::NotFound) => return ApiError::not_found("Recipe not found").into_response(),
        Err(_) => return ApiError::internal("Failed to fetch recipe").into_response(),
    };

    let exported = match export_recipe_to_paprikarecipe(&mut conn, user.id, &recipe) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to export recipe: {}", e);
            return ApiError::internal("Failed to export recipe").into_response();
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", exported.filename),
        )
        .body(Body::from(exported.data))
        .unwrap()
        .into_response()
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
) -> impl IntoResponse {
    let user_id = user.id;

    // Fetch recipe metadata (no photo bytes) on a blocking thread so we can
    // still return a clean 500 if the DB is unhappy. Once we commit to the
    // streaming body below, errors can only truncate the response.
    let pool_for_list = Arc::clone(&pool);
    let fetched = tokio::task::spawn_blocking(move || {
        let mut conn = pool_for_list.get().map_err(|e| format!("db pool: {}", e))?;
        fetch_current_recipes_with_versions(&mut conn, user_id).map_err(|e| e.to_string())
    })
    .await;

    let all_recipes = match fetched {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            tracing::error!(error = %e, "failed to fetch recipes for export");
            return ApiError::internal("Failed to fetch recipes").into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "export fetch task panicked");
            return ApiError::internal("Failed to fetch recipes").into_response();
        }
    };

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

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
        .into_response()
}
