use super::read::RecipeWithVersion;
use crate::db::DbConn;
use crate::models::Ingredient;
use crate::photos::processing::{generate_thumbnail, resize_for_export, EXPORT_PHOTO_DATA_SIZE};
use crate::schema::{photos, recipe_version_tags, user_tags};
use base64::Engine;
use diesel::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use uuid::Uuid;

/// Paprika recipe format for export.
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

/// Convert a Ramekin recipe to Paprika format.
fn convert_to_paprika(
    recipe: &RecipeWithVersion,
    photos_data: Vec<(Uuid, Vec<u8>)>,
    tags: Vec<String>,
) -> Result<PaprikaRecipe, String> {
    let version = &recipe.version;

    // Parse ingredients back to newline-separated format.
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

    let created = recipe.created_at.format("%Y-%m-%d %H:%M:%S").to_string();

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

/// Compress a recipe to gzip format (for .paprikarecipe files).
fn gzip_recipe(recipe: &PaprikaRecipe) -> Result<Vec<u8>, String> {
    let json = serde_json::to_string(recipe).map_err(|e| e.to_string())?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(json.as_bytes())
        .map_err(|e: std::io::Error| e.to_string())?;
    encoder.finish().map_err(|e: std::io::Error| e.to_string())
}

/// Fetch all photo data for a recipe.
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

/// Exported single recipe data (gzipped .paprikarecipe content).
pub(super) struct ExportedRecipe {
    pub filename: String,
    pub data: Vec<u8>,
}

/// Export a single recipe to .paprikarecipe format (gzipped JSON).
/// This is the core export function used by both single-recipe and bulk export.
pub(super) fn export_recipe_to_paprikarecipe(
    conn: &mut DbConn,
    user_id: Uuid,
    recipe: &RecipeWithVersion,
) -> Result<ExportedRecipe, String> {
    let photos_data = fetch_recipe_photos(conn, user_id, &recipe.version.photo_ids)?;
    let photo_count = photos_data.len();
    let photo_bytes: usize = photos_data.iter().map(|(_, d)| d.len()).sum();

    let tags: Vec<String> = recipe_version_tags::table
        .inner_join(user_tags::table)
        .filter(recipe_version_tags::recipe_version_id.eq(recipe.version.id))
        .filter(user_tags::deleted_at.is_null())
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(conn)
        .map_err(|e| format!("failed to fetch tags: {}", e))?;

    let paprika_recipe = convert_to_paprika(recipe, photos_data, tags)?;
    let data = gzip_recipe(&paprika_recipe)?;

    tracing::debug!(
        recipe_id = %recipe.id,
        photo_count,
        photo_bytes_raw = photo_bytes,
        gzipped_bytes = data.len(),
        "encoded .paprikarecipe"
    );

    let filename = paprika_recipe
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>();
    let filename = format!("{}.paprikarecipe", filename);

    Ok(ExportedRecipe { filename, data })
}
