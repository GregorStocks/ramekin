use crate::api::ErrorResponse;
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::{Ingredient, NewPhoto, NewRecipeVersion};
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::schema::{photos, recipe_version_tags, recipe_versions, recipes};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::Engine;
use diesel::prelude::*;
use ramekin_core::ai::{generate_recipe_photo as ai_generate_recipe_photo, AiConfig};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GeneratePhotoResponse {
    pub photo_id: Uuid,
    pub version_id: Uuid,
}

#[derive(Debug)]
enum GeneratePhotoWriteError {
    Db(diesel::result::Error),
    StaleVersion,
}

impl From<diesel::result::Error> for GeneratePhotoWriteError {
    fn from(value: diesel::result::Error) -> Self {
        Self::Db(value)
    }
}

#[allow(clippy::type_complexity)]
type CurrentVersionRow = (
    Uuid,              // recipes.id
    Option<Uuid>,      // recipes.current_version_id
    String,            // title
    Option<String>,    // description
    serde_json::Value, // ingredients
    String,            // instructions
    Option<String>,    // source_url
    Option<String>,    // source_name
    Vec<Option<Uuid>>, // photo_ids
    Option<String>,    // servings
    Option<String>,    // prep_time
    Option<String>,    // cook_time
    Option<String>,    // total_time
    Option<i32>,       // rating
    Option<String>,    // difficulty
    Option<String>,    // nutritional_info
    Option<String>,    // notes
);

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
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);

    let current: CurrentVersionRow = match recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::id.eq(recipe_id))
        .filter(recipes::user_id.eq(user.id))
        .filter(recipes::deleted_at.is_null())
        .select((
            recipes::id,
            recipes::current_version_id,
            recipe_versions::title,
            recipe_versions::description,
            recipe_versions::ingredients,
            recipe_versions::instructions,
            recipe_versions::source_url,
            recipe_versions::source_name,
            recipe_versions::photo_ids,
            recipe_versions::servings,
            recipe_versions::prep_time,
            recipe_versions::cook_time,
            recipe_versions::total_time,
            recipe_versions::rating,
            recipe_versions::difficulty,
            recipe_versions::nutritional_info,
            recipe_versions::notes,
        ))
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
        Err(e) => {
            tracing::error!("Failed to fetch recipe for photo generation: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response();
        }
    };

    let (
        recipe_id,
        source_version_id,
        title,
        description,
        ingredients,
        instructions,
        _source_url,
        _source_name,
        _current_photo_ids,
        _servings,
        _prep_time,
        _cook_time,
        _total_time,
        _rating,
        _difficulty,
        _nutritional_info,
        _notes,
    ) = current;

    let source_version_id = match source_version_id {
        Some(version_id) => version_id,
        None => {
            tracing::error!(
                "Recipe {} had no current version during photo generation",
                recipe_id
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch recipe".to_string(),
                }),
            )
                .into_response();
        }
    };

    let config = match AiConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("AI config unavailable for recipe photo generation: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "AI service unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    let ingredients_str = format_ingredients_for_prompt(&ingredients);
    let generated = match ai_generate_recipe_photo(
        &config,
        &title,
        description.as_deref(),
        &ingredients_str,
        &instructions,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("Recipe photo generation failed: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("Photo generation failed: {}", e),
                }),
            )
                .into_response();
        }
    };

    let raw_image = match decode_data_url_image(&generated.image_data_url) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Generated photo payload was invalid: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "AI returned an invalid image".to_string(),
                }),
            )
                .into_response();
        }
    };

    if raw_image.len() > MAX_FILE_SIZE {
        tracing::warn!(
            "Generated photo exceeded max size: {} bytes (max {})",
            raw_image.len(),
            MAX_FILE_SIZE
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "AI returned an invalid image".to_string(),
            }),
        )
            .into_response();
    }

    let processed = match process_image(&raw_image) {
        Ok(processed) => processed,
        Err(e) => {
            tracing::warn!("Generated photo failed validation: {}", e);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "AI returned an invalid image".to_string(),
                }),
            )
                .into_response();
        }
    };

    let write_result: Result<(Uuid, Uuid), GeneratePhotoWriteError> = conn.transaction(|conn| {
        let current: CurrentVersionRow = recipes::table
            .inner_join(
                recipe_versions::table.on(recipe_versions::id
                    .nullable()
                    .eq(recipes::current_version_id)),
            )
            .filter(recipes::id.eq(recipe_id))
            .filter(recipes::user_id.eq(user.id))
            .filter(recipes::deleted_at.is_null())
            .select((
                recipes::id,
                recipes::current_version_id,
                recipe_versions::title,
                recipe_versions::description,
                recipe_versions::ingredients,
                recipe_versions::instructions,
                recipe_versions::source_url,
                recipe_versions::source_name,
                recipe_versions::photo_ids,
                recipe_versions::servings,
                recipe_versions::prep_time,
                recipe_versions::cook_time,
                recipe_versions::total_time,
                recipe_versions::rating,
                recipe_versions::difficulty,
                recipe_versions::nutritional_info,
                recipe_versions::notes,
            ))
            .first(conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => GeneratePhotoWriteError::StaleVersion,
                other => GeneratePhotoWriteError::Db(other),
            })?;

        let (
            _recipe_id,
            current_version_id,
            current_title,
            current_description,
            current_ingredients,
            current_instructions,
            current_source_url,
            current_source_name,
            current_photo_ids,
            current_servings,
            current_prep_time,
            current_cook_time,
            current_total_time,
            current_rating,
            current_difficulty,
            current_nutritional_info,
            current_notes,
        ) = current;

        if current_version_id != Some(source_version_id) {
            return Err(GeneratePhotoWriteError::StaleVersion);
        }

        let new_photo = NewPhoto {
            user_id: user.id,
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
            .map_err(GeneratePhotoWriteError::Db)?;

        let mut new_photo_ids = Vec::with_capacity(current_photo_ids.len() + 1);
        new_photo_ids.push(Some(photo_id));
        new_photo_ids.extend(current_photo_ids.iter().copied());

        let new_version = NewRecipeVersion {
            recipe_id,
            title: &current_title,
            description: current_description.as_deref(),
            ingredients: current_ingredients,
            instructions: &current_instructions,
            source_url: current_source_url.as_deref(),
            source_name: current_source_name.as_deref(),
            photo_ids: &new_photo_ids,
            servings: current_servings.as_deref(),
            prep_time: current_prep_time.as_deref(),
            cook_time: current_cook_time.as_deref(),
            total_time: current_total_time.as_deref(),
            rating: current_rating,
            difficulty: current_difficulty.as_deref(),
            nutritional_info: current_nutritional_info.as_deref(),
            notes: current_notes.as_deref(),
            version_source: "ai_photo",
        };

        let new_version_id: Uuid = diesel::insert_into(recipe_versions::table)
            .values(&new_version)
            .returning(recipe_versions::id)
            .get_result(conn)
            .map_err(GeneratePhotoWriteError::Db)?;

        let updated_rows = diesel::update(
            recipes::table
                .filter(recipes::id.eq(recipe_id))
                .filter(recipes::user_id.eq(user.id))
                .filter(recipes::deleted_at.is_null())
                .filter(recipes::current_version_id.eq(source_version_id)),
        )
        .set(recipes::current_version_id.eq(new_version_id))
        .execute(conn)
        .map_err(GeneratePhotoWriteError::Db)?;

        if updated_rows == 0 {
            return Err(GeneratePhotoWriteError::StaleVersion);
        }

        let old_tag_ids: Vec<Uuid> = recipe_version_tags::table
            .filter(recipe_version_tags::recipe_version_id.eq(source_version_id))
            .select(recipe_version_tags::tag_id)
            .load(conn)
            .map_err(GeneratePhotoWriteError::Db)?;

        for tag_id in old_tag_ids {
            diesel::insert_into(recipe_version_tags::table)
                .values(crate::models::RecipeVersionTag {
                    recipe_version_id: new_version_id,
                    tag_id,
                })
                .on_conflict_do_nothing()
                .execute(conn)
                .map_err(GeneratePhotoWriteError::Db)?;
        }

        Ok((photo_id, new_version_id))
    });

    match write_result {
        Ok((photo_id, version_id)) => (
            StatusCode::OK,
            Json(GeneratePhotoResponse {
                photo_id,
                version_id,
            }),
        )
            .into_response(),
        Err(GeneratePhotoWriteError::StaleVersion) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Recipe changed while generating photo; try again".to_string(),
            }),
        )
            .into_response(),
        Err(GeneratePhotoWriteError::Db(e)) => {
            tracing::error!("Failed to persist generated recipe photo: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to save generated photo".to_string(),
                }),
            )
                .into_response()
        }
    }
}
