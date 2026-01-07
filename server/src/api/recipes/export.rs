use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::{DbConn, DbPool};
use crate::get_conn;
use crate::models::{Ingredient, Recipe};
use crate::schema::{photos, recipes};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use chrono::Utc;
use diesel::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Arc;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Paprika recipe format for export
#[derive(Debug, Serialize)]
struct PaprikaRecipe {
    uid: String,
    name: String,
    ingredients: String,
    directions: String,
    description: String,
    notes: String,
    source: String,
    source_url: String,
    categories: Vec<String>,
    servings: String,
    prep_time: String,
    cook_time: String,
    total_time: String,
    rating: i32,
    difficulty: String,
    nutritional_info: String,
    created: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    photos: Vec<PaprikaPhoto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    photo_data: Option<String>,
    hash: String,
}

#[derive(Debug, Serialize)]
struct PaprikaPhoto {
    filename: String,
    hash: String,
    data: String,
}

/// Convert a Ramekin recipe to Paprika format
fn convert_to_paprika(recipe: &Recipe, photos_data: Vec<(Uuid, Vec<u8>)>) -> PaprikaRecipe {
    // Parse ingredients back to newline-separated format
    let ingredients: Vec<Ingredient> =
        serde_json::from_value(recipe.ingredients.clone()).unwrap_or_default();
    let ingredients_str = ingredients
        .iter()
        .map(|i| i.item.clone())
        .collect::<Vec<_>>()
        .join("\n");

    // Convert photos to Paprika format
    let paprika_photos: Vec<PaprikaPhoto> = photos_data
        .iter()
        .map(|(id, data)| {
            let filename = format!("{}.jpg", id);
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash = format!("{:X}", hasher.finalize());
            let data_b64 = base64::engine::general_purpose::STANDARD.encode(data);
            PaprikaPhoto {
                filename,
                hash,
                data: data_b64,
            }
        })
        .collect();

    // Use first photo's base64 as photo_data fallback
    let photo_data = paprika_photos.first().map(|p| p.data.clone());

    // Format created timestamp in Paprika format
    let created = recipe.created_at.format("%Y-%m-%d %H:%M:%S").to_string();

    // Build the recipe JSON for hashing
    let recipe_content = format!(
        "{}{}{}{}",
        recipe.title,
        ingredients_str,
        recipe.instructions,
        recipe.description.as_deref().unwrap_or("")
    );
    let mut hasher = Sha256::new();
    hasher.update(recipe_content.as_bytes());
    let hash = format!("{:X}", hasher.finalize());

    PaprikaRecipe {
        uid: Uuid::new_v4().to_string().to_uppercase(),
        name: recipe.title.clone(),
        ingredients: ingredients_str,
        directions: recipe.instructions.clone(),
        description: recipe.description.clone().unwrap_or_default(),
        notes: recipe.notes.clone().unwrap_or_default(),
        source: recipe.source_name.clone().unwrap_or_default(),
        source_url: recipe.source_url.clone().unwrap_or_default(),
        categories: recipe.tags.iter().filter_map(|t| t.clone()).collect(),
        servings: recipe.servings.clone().unwrap_or_default(),
        prep_time: recipe.prep_time.clone().unwrap_or_default(),
        cook_time: recipe.cook_time.clone().unwrap_or_default(),
        total_time: recipe.total_time.clone().unwrap_or_default(),
        rating: recipe.rating.unwrap_or(0),
        difficulty: recipe.difficulty.clone().unwrap_or_default(),
        nutritional_info: recipe.nutritional_info.clone().unwrap_or_default(),
        created,
        photos: paprika_photos,
        photo_data,
        hash,
    }
}

/// Compress a recipe to gzip format (for .paprikarecipe files)
fn gzip_recipe(recipe: &PaprikaRecipe) -> Result<Vec<u8>, String> {
    let json = serde_json::to_string(recipe).map_err(|e| e.to_string())?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(json.as_bytes())
        .map_err(|e: std::io::Error| e.to_string())?;
    encoder.finish().map_err(|e: std::io::Error| e.to_string())
}

/// Fetch all photo data for a recipe
fn fetch_recipe_photos(
    conn: &mut diesel::PgConnection,
    user_id: Uuid,
    photo_ids: &[Option<Uuid>],
) -> Vec<(Uuid, Vec<u8>)> {
    let ids: Vec<Uuid> = photo_ids.iter().filter_map(|id| *id).collect();
    if ids.is_empty() {
        return Vec::new();
    }

    photos::table
        .filter(photos::id.eq_any(&ids))
        .filter(photos::user_id.eq(user_id))
        .filter(photos::deleted_at.is_null())
        .select((photos::id, photos::data))
        .load::<(Uuid, Vec<u8>)>(conn)
        .unwrap_or_default()
}

/// Exported single recipe data (gzipped .paprikarecipe content)
pub struct ExportedRecipe {
    pub filename: String,
    pub data: Vec<u8>,
}

/// Export a single recipe to .paprikarecipe format (gzipped JSON)
/// This is the core export function used by both single-recipe and bulk export.
pub fn export_recipe_to_paprikarecipe(
    conn: &mut DbConn,
    user_id: Uuid,
    recipe: &Recipe,
) -> Result<ExportedRecipe, String> {
    // Fetch photos for this recipe
    let photos_data = fetch_recipe_photos(conn, user_id, &recipe.photo_ids);

    // Convert to Paprika format
    let paprika_recipe = convert_to_paprika(recipe, photos_data);

    // Gzip compress
    let data = gzip_recipe(&paprika_recipe)?;

    // Sanitize filename
    let filename = paprika_recipe
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>();
    let filename = format!("{}.paprikarecipe", filename);

    Ok(ExportedRecipe { filename, data })
}

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

    // Fetch the recipe
    let recipe: Recipe = match recipes::table
        .filter(recipes::id.eq(id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select(Recipe::as_select())
        .first(&mut conn)
    {
        Ok(r) => r,
        Err(diesel::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Recipe not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Export to .paprikarecipe format (gzipped JSON)
    let exported = match export_recipe_to_paprikarecipe(&mut conn, user.id, &recipe) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to export recipe: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to export recipe".to_string(),
                }),
            )
                .into_response();
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
    let mut conn = get_conn!(pool);

    // Fetch all user's recipes
    let all_recipes: Vec<Recipe> = match recipes::table
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select(Recipe::as_select())
        .load(&mut conn)
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipes".to_string(),
                }),
            )
                .into_response()
        }
    };

    // Create ZIP archive in memory
    // A .paprikarecipes file is a ZIP containing .paprikarecipe files (each is gzipped JSON)
    let mut zip_buffer = Vec::new();
    {
        let mut zip = ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
        // Store without additional compression since each .paprikarecipe is already gzipped
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        for recipe in &all_recipes {
            // Export each recipe to .paprikarecipe format
            let exported = match export_recipe_to_paprikarecipe(&mut conn, user.id, recipe) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to export recipe {}: {}", recipe.title, e);
                    continue;
                }
            };

            // Add gzipped .paprikarecipe to ZIP
            if let Err(e) = zip.start_file(&exported.filename, options) {
                tracing::warn!("Failed to start ZIP entry for {}: {}", recipe.title, e);
                continue;
            }
            if let Err(e) = zip.write_all(&exported.data) {
                tracing::warn!("Failed to write ZIP entry for {}: {}", recipe.title, e);
                continue;
            }
        }

        if let Err(e) = zip.finish() {
            tracing::error!("Failed to finalize ZIP: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to create export archive".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Generate filename with timestamp
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let filename = format!("recipes-{}.paprikarecipes", timestamp);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(zip_buffer))
        .unwrap()
        .into_response()
}
