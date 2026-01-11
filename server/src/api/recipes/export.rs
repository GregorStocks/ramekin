use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::{DbConn, DbPool};
use crate::get_conn;
use crate::models::{Ingredient, RecipeVersion};
use crate::schema::{photos, recipe_versions, recipes};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use chrono::{DateTime, Utc};
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

/// Recipe with version info needed for export
pub struct RecipeWithVersion {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub version: RecipeVersion,
}

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
fn convert_to_paprika(
    recipe: &RecipeWithVersion,
    photos_data: Vec<(Uuid, Vec<u8>)>,
) -> PaprikaRecipe {
    let version = &recipe.version;

    // Parse ingredients back to newline-separated format
    let ingredients: Vec<Ingredient> =
        serde_json::from_value(version.ingredients.clone()).unwrap_or_default();
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
        version.title,
        ingredients_str,
        version.instructions,
        version.description.as_deref().unwrap_or("")
    );
    let mut hasher = Sha256::new();
    hasher.update(recipe_content.as_bytes());
    let hash = format!("{:X}", hasher.finalize());

    PaprikaRecipe {
        uid: recipe.id.to_string().to_uppercase(),
        name: version.title.clone(),
        ingredients: ingredients_str,
        directions: version.instructions.clone(),
        description: version.description.clone().unwrap_or_default(),
        notes: version.notes.clone().unwrap_or_default(),
        source: version.source_name.clone().unwrap_or_default(),
        source_url: version.source_url.clone().unwrap_or_default(),
        categories: version.tags.iter().filter_map(|t| t.clone()).collect(),
        servings: version.servings.clone().unwrap_or_default(),
        prep_time: version.prep_time.clone().unwrap_or_default(),
        cook_time: version.cook_time.clone().unwrap_or_default(),
        total_time: version.total_time.clone().unwrap_or_default(),
        rating: version.rating.unwrap_or(0),
        difficulty: version.difficulty.clone().unwrap_or_default(),
        nutritional_info: version.nutritional_info.clone().unwrap_or_default(),
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
    recipe: &RecipeWithVersion,
) -> Result<ExportedRecipe, String> {
    // Fetch photos for this recipe
    let photos_data = fetch_recipe_photos(conn, user_id, &recipe.version.photo_ids);

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

/// Fetch a recipe with its current version
fn fetch_recipe_with_version(
    conn: &mut DbConn,
    user_id: Uuid,
    recipe_id: Uuid,
) -> Result<RecipeWithVersion, diesel::result::Error> {
    let (id, created_at, current_version_id): (Uuid, DateTime<Utc>, Option<Uuid>) = recipes::table
        .filter(recipes::id.eq(recipe_id))
        .filter(recipes::user_id.eq(user_id))
        .filter(recipes::deleted_at.is_null())
        .select((
            recipes::id,
            recipes::created_at,
            recipes::current_version_id,
        ))
        .first(conn)?;

    let version_id = current_version_id.ok_or(diesel::result::Error::NotFound)?;

    let version: RecipeVersion = recipe_versions::table
        .filter(recipe_versions::id.eq(version_id))
        .first(conn)?;

    Ok(RecipeWithVersion {
        id,
        created_at,
        version,
    })
}

/// Fetch all recipes with their current versions for a user
fn fetch_all_recipes_with_versions(
    conn: &mut DbConn,
    user_id: Uuid,
) -> Result<Vec<RecipeWithVersion>, diesel::result::Error> {
    // Single query with JOIN
    let rows: Vec<(Uuid, DateTime<Utc>, RecipeVersion)> = recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::user_id.eq(user_id))
        .filter(recipes::deleted_at.is_null())
        .select((recipes::id, recipes::created_at, RecipeVersion::as_select()))
        .load(conn)?;

    Ok(rows
        .into_iter()
        .map(|(id, created_at, version)| RecipeWithVersion {
            id,
            created_at,
            version,
        })
        .collect())
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

    // Fetch the recipe with its current version
    let recipe = match fetch_recipe_with_version(&mut conn, user.id, id) {
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

    // Fetch all user's recipes with their current versions
    let all_recipes = match fetch_all_recipes_with_versions(&mut conn, user.id) {
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
                    tracing::warn!("Failed to export recipe {}: {}", recipe.version.title, e);
                    continue;
                }
            };

            // Add gzipped .paprikarecipe to ZIP
            if let Err(e) = zip.start_file(&exported.filename, options) {
                tracing::warn!(
                    "Failed to start ZIP entry for {}: {}",
                    recipe.version.title,
                    e
                );
                continue;
            }
            if let Err(e) = zip.write_all(&exported.data) {
                tracing::warn!(
                    "Failed to write ZIP entry for {}: {}",
                    recipe.version.title,
                    e
                );
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
