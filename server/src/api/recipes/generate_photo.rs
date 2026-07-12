use crate::api::ai::ai_config_from_env;
use crate::api::{run_db, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Ingredient, NewPhoto, NewRecipeVersion, RecipeVersion};
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::recipes::{create_new_version_cas, VersionWriteError};
use crate::schema::{photos, recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::Engine;
use diesel::prelude::*;
use ramekin_core::ai::generate_recipe_photo as ai_generate_recipe_photo;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GeneratePhotoResponse {
    pub photo_id: Uuid,
    pub version_id: Uuid,
}

fn format_ingredients_for_prompt(ingredients: &serde_json::Value) -> String {
    let Ok(items) = serde_json::from_value::<Vec<Ingredient>>(ingredients.clone()) else {
        return ingredients.to_string();
    };

    items
        .iter()
        .map(|i| {
            let measurement = i
                .measurements
                .first()
                .map(|m| {
                    format!(
                        "{} {}",
                        m.amount.as_deref().unwrap_or(""),
                        m.unit.as_deref().unwrap_or("")
                    )
                })
                .unwrap_or_default();
            format!("{} {}", measurement, i.item).trim().to_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn decode_data_url_image(data_url: &str) -> Result<Vec<u8>, String> {
    let Some((metadata, payload)) = data_url.split_once(',') else {
        return Err("Generated image was not a valid data URL".to_string());
    };

    if !metadata.starts_with("data:image/") || !metadata.ends_with(";base64") {
        return Err("Generated image was not returned as a base64 image data URL".to_string());
    }

    let estimated_decoded_len = payload.len().saturating_mul(3) / 4;
    if estimated_decoded_len > MAX_FILE_SIZE {
        return Err(format!(
            "Generated image payload too large: estimated {} bytes (max {})",
            estimated_decoded_len, MAX_FILE_SIZE
        ));
    }

    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| format!("Failed to decode generated image: {}", e))
}

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/generate-photo",
    tag = "recipes",
    params(
        ("id" = Uuid, Path, description = "Recipe ID")
    ),
    responses(
        (status = 200, description = "Recipe photo generated and applied", body = GeneratePhotoResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 409, description = "Recipe changed during generation", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse),
        (status = 503, description = "AI service unavailable", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn generate_photo(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    // Read recipe snapshot in its own run_db block so no DB connection is held
    // across the (potentially slow) AI call.
    let (source_version_id, current_version): (Option<Uuid>, RecipeVersion) =
        run_db(&pool, move |conn| {
            recipes::table
                .inner_join(
                    recipe_versions::table.on(recipe_versions::id
                        .nullable()
                        .eq(recipes::current_version_id)),
                )
                .filter(recipes::id.eq(recipe_id))
                .filter(recipes::user_id.eq(user_id))
                .filter(recipes::deleted_at.is_null())
                .select((recipes::current_version_id, RecipeVersion::as_select()))
                .first(conn)
                .map_err(|e| match e {
                    diesel::NotFound => ApiError::not_found("Recipe not found"),
                    e => {
                        tracing::error!("Failed to fetch recipe for photo generation: {}", e);
                        ApiError::internal("Failed to fetch recipe")
                    }
                })
        })
        .await?;

    let source_version_id = match source_version_id {
        Some(version_id) => version_id,
        None => {
            tracing::error!(
                "Recipe {} had no current version during photo generation",
                recipe_id
            );
            return Err(ApiError::internal("Failed to fetch recipe"));
        }
    };

    let config = ai_config_from_env()?;

    let ingredients_str = format_ingredients_for_prompt(&current_version.ingredients);
    let generated = match ai_generate_recipe_photo(
        &config,
        &current_version.title,
        current_version.description.as_deref(),
        &ingredients_str,
        &current_version.instructions,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("Recipe photo generation failed: {}", e);
            return Err(ApiError::service_unavailable(format!(
                "Photo generation failed: {}",
                e
            )));
        }
    };

    let raw_image = match decode_data_url_image(&generated.image_data_url) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Generated photo payload was invalid: {}", e);
            return Err(ApiError::service_unavailable(
                "AI returned an invalid image",
            ));
        }
    };

    if raw_image.len() > MAX_FILE_SIZE {
        tracing::warn!(
            "Generated photo exceeded max size: {} bytes (max {})",
            raw_image.len(),
            MAX_FILE_SIZE
        );
        return Err(ApiError::service_unavailable(
            "AI returned an invalid image",
        ));
    }

    let processed = match process_image(&raw_image) {
        Ok(processed) => processed,
        Err(e) => {
            tracing::warn!("Generated photo failed validation: {}", e);
            return Err(ApiError::service_unavailable(
                "AI returned an invalid image",
            ));
        }
    };

    let (photo_id, version_id) = run_db(&pool, move |conn| {
        let write_result: Result<(Uuid, Uuid), VersionWriteError> = conn.transaction(|conn| {
            let (current_version_id, current_version): (Option<Uuid>, RecipeVersion) =
                recipes::table
                    .inner_join(
                        recipe_versions::table.on(recipe_versions::id
                            .nullable()
                            .eq(recipes::current_version_id)),
                    )
                    .filter(recipes::id.eq(recipe_id))
                    .filter(recipes::user_id.eq(user_id))
                    .filter(recipes::deleted_at.is_null())
                    .select((recipes::current_version_id, RecipeVersion::as_select()))
                    .first(conn)
                    .map_err(|e| match e {
                        diesel::result::Error::NotFound => VersionWriteError::Stale,
                        other => VersionWriteError::Db(other),
                    })?;

            if current_version_id != Some(source_version_id) {
                return Err(VersionWriteError::Stale);
            }

            let new_photo = NewPhoto {
                user_id,
                content_type: &processed.content_type,
                data: &raw_image,
                thumbnail: &processed.thumbnail,
                width: Some(processed.width as i32),
                height: Some(processed.height as i32),
                file_size: Some(raw_image.len() as i32),
            };

            let photo_id: Uuid = diesel::insert_into(photos::table)
                .values(&new_photo)
                .returning(photos::id)
                .get_result(conn)
                .map_err(VersionWriteError::Db)?;

            let mut new_photo_ids = Vec::with_capacity(current_version.photo_ids.len() + 1);
            new_photo_ids.push(Some(photo_id));
            new_photo_ids.extend(current_version.photo_ids.iter().copied());

            let new_version = NewRecipeVersion {
                photo_ids: &new_photo_ids,
                ..NewRecipeVersion::copy_of(&current_version, "ai_photo")
            };

            // Compare-and-swap: only repoint if current_version_id still matches
            // the version we generated the photo for.
            let new_version_id = create_new_version_cas(
                conn,
                &new_version,
                Some(source_version_id),
                crate::recipes::TagSource::CopyFrom(source_version_id),
            )?;

            Ok((photo_id, new_version_id))
        });

        match write_result {
            Ok(ids) => Ok(ids),
            Err(VersionWriteError::Stale) => Err(ApiError::conflict(
                "Recipe changed while generating photo; try again",
            )),
            Err(VersionWriteError::Db(e)) => {
                tracing::error!("Failed to persist generated recipe photo: {}", e);
                Err(ApiError::internal("Failed to save generated photo"))
            }
        }
    })
    .await?;

    Ok((
        StatusCode::OK,
        Json(GeneratePhotoResponse {
            photo_id,
            version_id,
        }),
    ))
}
