use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::{DbConn, DbPool};
use crate::get_conn;
use crate::models::{Ingredient, RecipeVersion};
use crate::photos::processing::{generate_thumbnail, resize_for_export, EXPORT_PHOTO_DATA_SIZE};
use crate::schema::{photos, recipe_version_tags, recipe_versions, recipes, user_tags};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
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
    tags: Vec<String>,
) -> Result<PaprikaRecipe, String> {
    let version = &recipe.version;

    // Parse ingredients back to newline-separated format
    let ingredients: Vec<Ingredient> = serde_json::from_value(version.ingredients.clone())
        .map_err(|e| format!("stored ingredients JSON failed to deserialize: {}", e))?;
    let ingredients_str = ingredients
        .iter()
        .map(|i| i.item.clone())
        .collect::<Vec<_>>()
        .join("\n");

    let fallback_photo_data = photos_data.first().and_then(|(id, raw)| {
        match generate_thumbnail(raw, EXPORT_PHOTO_DATA_SIZE) {
            Ok(thumbnail) => Some(base64::engine::general_purpose::STANDARD.encode(thumbnail)),
            Err(e) => {
                tracing::warn!(
                    photo_id = %id,
                    bytes = raw.len(),
                    error = %e,
                    "skipping export photo_data fallback; thumbnail generation failed"
                );
                None
            }
        }
    });

    // Each photo is downscaled before base64 encoding. Originals can be up to
    // MAX_FILE_SIZE (10MB) each and the export would otherwise hold every
    // photo's raw bytes plus a 1.33x base64 copy plus the JSON + gzip buffer
    // all live at once. Photos that fail to decode/resize are skipped with a
    // warning rather than failing the whole recipe export.
    let paprika_photos: Vec<PaprikaPhoto> = photos_data
        .into_iter()
        .filter_map(|(id, raw)| {
            let raw_len = raw.len();
            let resized = match resize_for_export(&raw) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        photo_id = %id,
                        bytes = raw_len,
                        error = %e,
                        "skipping photo during export; resize failed"
                    );
                    return None;
                }
            };
            drop(raw);
            let filename = format!("{}.jpg", id);
            let mut hasher = Sha256::new();
            hasher.update(&resized);
            let hash = hex::encode_upper(hasher.finalize());
            let data_b64 = base64::engine::general_purpose::STANDARD.encode(&resized);
            Some(PaprikaPhoto {
                filename,
                hash,
                data: data_b64,
            })
        })
        .collect();

    // `photo_data` is only a fallback thumbnail when the structured `photos`
    // array is empty. Duplicating the first full photo here roughly doubles
    // the photo payload for no importer benefit.
    let photo_data = if paprika_photos.is_empty() {
        fallback_photo_data
    } else {
        None
    };

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
    let hash = hex::encode_upper(hasher.finalize());

    Ok(PaprikaRecipe {
        uid: recipe.id.to_string().to_uppercase(),
        name: version.title.clone(),
        ingredients: ingredients_str,
        directions: version.instructions.clone(),
        description: version.description.clone().unwrap_or_default(),
        notes: version.notes.clone().unwrap_or_default(),
        source: version.source_name.clone().unwrap_or_default(),
        source_url: version.source_url.clone().unwrap_or_default(),
        categories: tags,
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
    })
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
) -> Result<Vec<(Uuid, Vec<u8>)>, String> {
    let ids: Vec<Uuid> = photo_ids.iter().filter_map(|id| *id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    photos::table
        .filter(photos::id.eq_any(&ids))
        .filter(photos::user_id.eq(user_id))
        .filter(photos::deleted_at.is_null())
        .select((photos::id, photos::data))
        .load::<(Uuid, Vec<u8>)>(conn)
        .map_err(|e| format!("failed to fetch photos: {}", e))
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
    let photos_data = fetch_recipe_photos(conn, user_id, &recipe.version.photo_ids)?;
    let photo_count = photos_data.len();
    let photo_bytes: usize = photos_data.iter().map(|(_, d)| d.len()).sum();

    // Fetch tags for this recipe version from junction table
    let tags: Vec<String> = recipe_version_tags::table
        .inner_join(user_tags::table)
        .filter(recipe_version_tags::recipe_version_id.eq(recipe.version.id))
        .filter(user_tags::deleted_at.is_null())
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(conn)
        .map_err(|e| format!("failed to fetch tags: {}", e))?;

    // Convert to Paprika format
    let paprika_recipe = convert_to_paprika(recipe, photos_data, tags)?;

    // Gzip compress
    let data = gzip_recipe(&paprika_recipe)?;

    tracing::debug!(
        recipe_id = %recipe.id,
        photo_count,
        photo_bytes_raw = photo_bytes,
        gzipped_bytes = data.len(),
        "encoded .paprikarecipe"
    );

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
        Err(diesel::NotFound) => return ApiError::not_found("Recipe not found").into_response(),
        Err(_) => return ApiError::internal("Failed to fetch recipe").into_response(),
    };

    // Export to .paprikarecipe format (gzipped JSON)
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

/// `std::io::Write` that forwards writes as `Bytes` chunks to a tokio mpsc
/// channel. Lets a blocking ZIP writer stream output to an axum body without
/// buffering the whole archive in memory.
struct ChannelWriter {
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.tx
            .blocking_send(Ok(Bytes::copy_from_slice(buf)))
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Write every recipe in `recipes` as a .paprikarecipe entry to a streaming
/// ZIP. Per-recipe content-level failures (e.g. DB error fetching photos,
/// zip metadata rejection) are logged and skipped; writer IO errors (client
/// disconnect surfacing as a broken pipe from ChannelWriter) abort the
/// stream so we don't keep doing expensive DB/CPU work with nowhere to send
/// the bytes.
///
/// A fresh DB connection is checked out per recipe and dropped before the
/// (potentially slow, backpressured) zip write, so a slow client can't pin
/// a pool connection for the whole download.
fn write_zip_stream(
    pool: &DbPool,
    user_id: Uuid,
    recipes: &[RecipeWithVersion],
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
) -> io::Result<u64> {
    let writer = ChannelWriter { tx };
    // new_stream uses data descriptors and avoids seek operations, so the
    // zip bytes can be produced and forwarded to the network without ever
    // materializing the whole archive in memory.
    let mut zip = ZipWriter::new_stream(writer);
    // Store without additional compression since each .paprikarecipe is already gzipped
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut total_entry_bytes: u64 = 0;
    for recipe in recipes {
        // Short-lived per-recipe connection: held during the DB fetch and
        // in-memory encoding, released before the slow zip write.
        //
        // Pool checkout failures abort the stream rather than silently
        // skipping the recipe — an export is meant as a backup, so silently
        // omitting recipes under pool pressure is partial data loss the
        // client would have no way to notice.
        let exported = {
            let mut conn = pool.get().map_err(|e| {
                tracing::error!(
                    recipe_id = %recipe.id,
                    title = %recipe.version.title,
                    error = %e,
                    "failed to acquire db connection during export; aborting stream"
                );
                io::Error::other(format!("db pool: {}", e))
            })?;
            match export_recipe_to_paprikarecipe(&mut conn, user_id, recipe) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(
                        recipe_id = %recipe.id,
                        title = %recipe.version.title,
                        error = %e,
                        "failed to export recipe; skipping"
                    );
                    continue;
                }
            }
        };

        // start_file writes the entry header via ChannelWriter, so an IO
        // error here almost always means the client has gone away. Propagate
        // it so we abort rather than spinning on the rest of the library.
        match zip.start_file(&exported.filename, options) {
            Ok(()) => {}
            Err(zip::result::ZipError::Io(e)) => return Err(e),
            Err(e) => {
                tracing::warn!(
                    recipe_id = %recipe.id,
                    title = %recipe.version.title,
                    error = %e,
                    "failed to start zip entry; skipping"
                );
                continue;
            }
        }
        let entry_bytes = exported.data.len() as u64;
        zip.write_all(&exported.data)?;
        total_entry_bytes += entry_bytes;
    }

    zip.finish()
        .map_err(|e| io::Error::other(format!("finalize zip: {}", e)))?;
    Ok(total_entry_bytes)
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
        fetch_all_recipes_with_versions(&mut conn, user_id).map_err(|e| e.to_string())
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
